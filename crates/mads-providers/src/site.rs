use std::{
    collections::{HashMap, HashSet},
    io::Read,
    net::IpAddr,
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use flate2::read::GzDecoder;
use mads_core::init::{FetchedPage, SiteFetch};
use quick_xml::{Reader, events::Event};
use reqwest::{Client, redirect::Policy};
use scraper::{ElementRef, Html, Selector};
use texting_robots::Robot;
use tokio::sync::Mutex;
use url::{Host, Url};

const TEXT_LIMIT: usize = 8000;
const LINK_LIMIT: usize = 200;
const BODY_LIMIT: u64 = 5 * 1024 * 1024;
const REDIRECT_LIMIT: usize = 5;
const SITEMAP_DEPTH: usize = 2;
const SITEMAP_CHILDREN: usize = 50;
const SITEMAP_URL_CAP: usize = 200_000;
const ROBOTS_AGENT: &str = "mads";
const SKIPPED_TAGS: [&str; 6] = ["script", "style", "noscript", "template", "svg", "head"];

/// Special-purpose addresses a crawler must never touch: loopback, private ranges, link-local
/// (cloud metadata lives at 169.254.169.254), CGNAT, multicast and the unspecified address.
pub(crate) fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || (o[0] == 100 && (o[1] & 0xC0) == 64)
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_private_ip(IpAddr::V4(mapped));
            }
            let s = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xffc0) == 0xfe80
        }
    }
}

/// The start host and its `www.` or apex sibling. Nothing else counts as the same site.
pub(crate) fn site_hosts(start_url: &str) -> Result<Vec<String>, String> {
    let url = Url::parse(start_url).map_err(|e| format!("not a URL: {start_url} ({e})"))?;
    let host = url
        .host_str()
        .ok_or_else(|| format!("no host in {start_url}"))?
        .to_lowercase();
    let apex = host.strip_prefix("www.").unwrap_or(&host).to_string();
    Ok(vec![apex.clone(), format!("www.{apex}")])
}

pub(crate) fn host_allowed(hosts: &[String], url: &str) -> bool {
    Url::parse(url)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .and_then(|u| u.host_str().map(str::to_lowercase))
        .is_some_and(|h| hosts.contains(&h))
}

fn selector(css: &str) -> Selector {
    // Every selector in this file is a constant that parses; a failure is a programming error.
    Selector::parse(css).unwrap_or_else(|e| unreachable!("bad selector {css}: {e}"))
}

fn meta(html: &Html, css: &str) -> Option<String> {
    html.select(&selector(css))
        .find_map(|m| m.value().attr("content"))
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
}

/// Title, description, visible text and same-site links. Scripts, styles and the head are not text.
pub(crate) fn extract_page(page_url: &str, body: &str, hosts: &[String]) -> FetchedPage {
    let html = Html::parse_document(body);
    let title = html
        .select(&selector("title"))
        .next()
        .map(|t| {
            t.text()
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let description = meta(&html, r#"meta[name="description"]"#)
        .or_else(|| meta(&html, r#"meta[property="og:description"]"#))
        .unwrap_or_default();

    let root = html
        .select(&selector("body"))
        .next()
        .unwrap_or_else(|| html.root_element());
    let mut pieces: Vec<&str> = Vec::new();
    for node in root.descendants() {
        let Some(text) = node.value().as_text() else {
            continue;
        };
        let hidden = node
            .ancestors()
            .filter_map(ElementRef::wrap)
            .any(|e| SKIPPED_TAGS.contains(&e.value().name()));
        if !hidden {
            pieces.push(text);
        }
    }
    let flat = pieces
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let text: String = flat.chars().take(TEXT_LIMIT).collect();

    let base = Url::parse(page_url).ok();
    let mut seen = HashSet::new();
    let mut links = Vec::new();
    for a in html.select(&selector("a[href]")) {
        let href = a.value().attr("href").unwrap_or("").trim();
        let Some(mut url) = base.as_ref().and_then(|b| b.join(href).ok()) else {
            continue;
        };
        url.set_fragment(None);
        let link = url.to_string();
        if !href.is_empty() && host_allowed(hosts, &link) && seen.insert(link.clone()) {
            links.push(link);
        }
        if links.len() == LINK_LIMIT {
            break;
        }
    }
    FetchedPage {
        url: page_url.to_string(),
        status: 200,
        title,
        description,
        text,
        links,
    }
}

/// Read-only HTTP access to one website for the `init` mission.
pub struct SiteClient {
    client: Client,
    hosts: Arc<Vec<String>>,
    start: Url,
    allow_private: bool,
    sitemap_cap: usize,
    robots: Mutex<HashMap<String, Option<Arc<Robot>>>>,
}

impl SiteClient {
    /// `allow_private` lifts the block on loopback and private addresses. Only tests and local
    /// development need it.
    pub fn new(start_url: &str, allow_private: bool) -> Result<Self, String> {
        let hosts = Arc::new(site_hosts(start_url)?);
        let start = Url::parse(start_url).map_err(|e| e.to_string())?;
        let policy_hosts = hosts.clone();
        let policy = Policy::custom(move |attempt| {
            if attempt.previous().len() >= REDIRECT_LIMIT
                || !host_allowed(&policy_hosts, attempt.url().as_str())
            {
                attempt.stop()
            } else {
                attempt.follow()
            }
        });
        let client = Client::builder()
            .user_agent(concat!(
                "mads/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/avelino/mads)"
            ))
            .timeout(Duration::from_secs(20))
            .redirect(policy)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            hosts,
            start,
            allow_private,
            sitemap_cap: SITEMAP_URL_CAP,
            robots: Mutex::new(HashMap::new()),
        })
    }

    pub fn with_sitemap_cap(mut self, cap: usize) -> Self {
        self.sitemap_cap = cap;
        self
    }

    /// Resolves the host and refuses private answers. A name that resolves later to a private
    /// address (DNS rebinding between this check and the connection) is not covered.
    async fn ensure_public(&self, url: &Url) -> Result<(), String> {
        if self.allow_private {
            return Ok(());
        }
        match url.host() {
            None => Err(format!("no host in {url}")),
            Some(Host::Ipv4(_) | Host::Ipv6(_)) => Err("IP address hosts are not allowed".into()),
            Some(Host::Domain(name)) => {
                let port = url.port_or_known_default().unwrap_or(443);
                let addrs = tokio::net::lookup_host((name, port))
                    .await
                    .map_err(|e| format!("cannot resolve {name}: {e}"))?;
                if addrs.into_iter().any(|a| is_private_ip(a.ip())) {
                    return Err(format!("{name} resolves to a private address"));
                }
                Ok(())
            }
        }
    }

    async fn robots_for(&self, url: &Url) -> Option<Arc<Robot>> {
        let key = format!("{}://{}", url.scheme(), url.host_str()?);
        if let Some(cached) = self.robots.lock().await.get(&key) {
            return cached.clone();
        }
        let robots_url = url.join("/robots.txt").ok()?;
        let robot = match self.client.get(robots_url).send().await {
            Ok(r) if r.status().is_success() => r
                .bytes()
                .await
                .ok()
                .and_then(|b| Robot::new(ROBOTS_AGENT, &b).ok())
                .map(Arc::new),
            _ => None,
        };
        self.robots.lock().await.insert(key, robot.clone());
        robot
    }

    async fn get_bytes(&self, url: &Url) -> Result<Vec<u8>, String> {
        let resp = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status().as_u16()));
        }
        if resp.content_length().is_some_and(|n| n > BODY_LIMIT) {
            return Err("response is too large".into());
        }
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        if bytes.len() as u64 > BODY_LIMIT {
            return Err("response is too large".into());
        }
        Ok(bytes.to_vec())
    }

    fn check_site(&self, url: &str) -> Result<Url, String> {
        if !host_allowed(&self.hosts, url) {
            return Err(format!("{url} is not on the business website"));
        }
        Url::parse(url).map_err(|e| e.to_string())
    }

    /// One sitemap file: the child sitemaps it lists and the page URLs it holds.
    async fn read_sitemap(&self, url: &Url) -> Result<(Vec<String>, Vec<String>), String> {
        self.ensure_public(url).await?;
        let mut bytes = self.get_bytes(url).await?;
        if bytes.starts_with(&[0x1f, 0x8b]) {
            let mut text = Vec::new();
            GzDecoder::new(bytes.as_slice())
                .take(BODY_LIMIT * 4)
                .read_to_end(&mut text)
                .map_err(|e| format!("bad gzip: {e}"))?;
            bytes = text;
        }
        Ok(parse_sitemap(&bytes))
    }

    async fn candidate_sitemaps(&self, requested: Option<&str>) -> Result<Vec<Url>, String> {
        if let Some(u) = requested {
            return Ok(vec![self.check_site(u)?]);
        }
        let mut out: Vec<Url> = Vec::new();
        if let Some(robot) = self.robots_for(&self.start).await {
            out.extend(robot.sitemaps.iter().filter_map(|s| Url::parse(s).ok()));
        }
        out.extend(self.start.join("/sitemap.xml").ok());
        Ok(out)
    }
}

fn resolve_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16)
            .ok()
            .and_then(char::from_u32),
        n if n.starts_with('#') => n[1..].parse::<u32>().ok().and_then(char::from_u32),
        _ => None,
    }
}

/// `(child sitemaps, page URLs)` from a sitemap or a sitemap index. Entities such as `&amp;` are
/// decoded: the parser delivers them as separate events, so a `<loc>` is assembled piece by piece.
fn parse_sitemap(xml: &[u8]) -> (Vec<String>, Vec<String>) {
    let mut reader = Reader::from_reader(xml);
    let (mut children, mut pages) = (Vec::new(), Vec::new());
    let (mut in_sitemap, mut in_url, mut in_loc) = (false, false, false);
    let (mut loc, mut buf) = (String::new(), Vec::new());
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                "sitemap" => in_sitemap = true,
                "url" => in_url = true,
                "loc" => {
                    in_loc = true;
                    loc.clear();
                }
                _ => {}
            },
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                "sitemap" => in_sitemap = false,
                "url" => in_url = false,
                "loc" => {
                    in_loc = false;
                    let text = loc.trim().to_string();
                    if in_sitemap {
                        children.push(text);
                    } else if in_url {
                        pages.push(text);
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) if in_loc => loc.push_str(&t.xml10_content()),
            Ok(Event::GeneralRef(r)) if in_loc => loc.extend(resolve_entity(&r)),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    (children, pages)
}

#[async_trait]
impl SiteFetch for SiteClient {
    fn is_same_site(&self, url: &str) -> bool {
        host_allowed(&self.hosts, url)
    }

    async fn fetch_page(&self, url: &str) -> Result<FetchedPage, String> {
        let parsed = self.check_site(url)?;
        self.ensure_public(&parsed).await?;
        if let Some(robot) = self.robots_for(&parsed).await
            && !robot.allowed(url)
        {
            return Err(format!("robots.txt disallows {url}"));
        }
        let resp = self
            .client
            .get(parsed)
            .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("HTTP {}", status.as_u16()));
        }
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if !content_type.to_lowercase().contains("html") {
            return Err(format!("not HTML (content-type {content_type})"));
        }
        if resp.content_length().is_some_and(|n| n > BODY_LIMIT) {
            return Err("response is too large".into());
        }
        let final_url = resp.url().to_string();
        let body = resp.bytes().await.map_err(|e| e.to_string())?;
        let mut page = extract_page(&final_url, &String::from_utf8_lossy(&body), &self.hosts);
        page.status = status.as_u16();
        Ok(page)
    }

    async fn fetch_sitemap(&self, url: Option<&str>) -> Result<Vec<String>, String> {
        let candidates = self.candidate_sitemaps(url).await?;
        let tried = candidates
            .iter()
            .map(Url::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let mut urls: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue: Vec<(Url, usize)> = candidates.into_iter().map(|u| (u, 0)).collect();
        queue.reverse();
        let mut read_any = false;
        while let Some((sitemap, depth)) = queue.pop() {
            if !seen.insert(sitemap.to_string()) {
                continue;
            }
            let Ok((children, pages)) = self.read_sitemap(&sitemap).await else {
                continue;
            };
            read_any = true;
            if depth < SITEMAP_DEPTH {
                let mut next: Vec<Url> = children
                    .iter()
                    .take(SITEMAP_CHILDREN)
                    .filter(|c| host_allowed(&self.hosts, c))
                    .filter_map(|c| Url::parse(c).ok())
                    .collect();
                next.reverse();
                queue.extend(next.into_iter().map(|u| (u, depth + 1)));
            }
            for page in pages.into_iter().filter(|p| host_allowed(&self.hosts, p)) {
                if urls.len() >= self.sitemap_cap {
                    return Ok(urls);
                }
                urls.push(page);
            }
        }
        if read_any {
            Ok(urls)
        } else {
            Err(format!("no sitemap found (tried {tried})"))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::Write,
        net::{IpAddr, Ipv4Addr},
    };

    use flate2::{Compression, write::GzEncoder};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn private_and_special_addresses_are_recognised() {
        for a in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "255.255.255.255",
            "224.0.0.1",
            "::1",
            "::",
            "fe80::1",
            "fc00::1",
            "fd12::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
        ] {
            assert!(is_private_ip(ip(a)), "{a} should be private");
        }
        for a in [
            "8.8.8.8",
            "1.1.1.1",
            "172.32.0.1",
            "172.15.255.255",
            "100.128.0.1",
            "2606:4700::1111",
            "::ffff:8.8.8.8",
        ] {
            assert!(!is_private_ip(ip(a)), "{a} should be public");
        }
    }

    #[test]
    fn the_site_is_the_start_host_and_its_www_sibling() {
        let hosts = site_hosts("https://www.vinellu.com/pt").unwrap();
        for ok in [
            "https://vinellu.com/a",
            "https://www.vinellu.com/b",
            "HTTPS://VINELLU.COM:443/c",
        ] {
            assert!(host_allowed(&hosts, ok), "{ok}");
        }
        for bad in [
            "https://blog.vinellu.com",
            "https://evil.com",
            "https://vinellu.com.evil.com",
            "ftp://vinellu.com",
            "garbage",
        ] {
            assert!(!host_allowed(&hosts, bad), "{bad}");
        }
        assert_eq!(
            site_hosts("https://vinellu.com").unwrap(),
            site_hosts("https://www.vinellu.com").unwrap()
        );
        assert!(site_hosts("not a url").is_err());
    }

    const HTML: &str = r#"<html><head><title> Vinellu | Vinhos </title>
        <meta name="description" content="App social de vinhos">
        <style>.x{color:red}</style><script>var secret = 1;</script></head>
        <body><nav><a href="/app">App</a> <a href="https://vinellu.com/app#top">App again</a>
        <a href="https://www.vinellu.com/sobre">Sobre</a> <a href="https://other.com/x">Out</a>
        <a href="mailto:a@b.c">mail</a> <a href="tel:123">tel</a> <a href="javascript:void(0)">js</a> <a href="">empty</a></nav>
        <h1>Descubra  vinhos</h1><p>Foto do rótulo,<br>nota na hora.</p><noscript>no script</noscript>
        <script>alert("x")</script></body></html>"#;

    #[test]
    fn extraction_reads_title_description_text_and_same_site_links() {
        let hosts = site_hosts("https://vinellu.com").unwrap();
        let p = extract_page("https://vinellu.com/", HTML, &hosts);
        assert_eq!(p.title, "Vinellu | Vinhos");
        assert_eq!(p.description, "App social de vinhos");
        assert!(
            p.text.contains("Descubra vinhos") && p.text.contains("Foto do rótulo, nota na hora."),
            "{}",
            p.text
        );
        for hidden in ["secret", "color:red", "alert", "no script"] {
            assert!(!p.text.contains(hidden), "{hidden} leaked into the text");
        }
        assert_eq!(
            p.links,
            ["https://vinellu.com/app", "https://www.vinellu.com/sobre"]
        );
    }

    #[test]
    fn open_graph_description_is_the_fallback_and_text_is_capped() {
        let hosts = site_hosts("https://vinellu.com").unwrap();
        let html = format!(
            r#"<head><meta property="og:description" content="og desc"></head><body>{}</body>"#,
            "palavra ".repeat(3000)
        );
        let p = extract_page("https://vinellu.com/", &html, &hosts);
        assert_eq!(p.description, "og desc");
        assert_eq!(p.text.chars().count(), TEXT_LIMIT);
    }

    #[test]
    fn links_are_capped_at_200() {
        let hosts = site_hosts("https://vinellu.com").unwrap();
        let body: String = (0..500)
            .map(|i| format!(r#"<a href="/p/{i}">x</a>"#))
            .collect();
        let p = extract_page(
            "https://vinellu.com/",
            &format!("<body>{body}</body>"),
            &hosts,
        );
        assert_eq!(p.links.len(), 200);
    }

    async fn client(server: &MockServer) -> SiteClient {
        SiteClient::new(&server.uri(), true).unwrap()
    }

    fn html(body: &str) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_raw(body.to_string(), "text/html; charset=utf-8")
    }

    #[tokio::test]
    async fn a_page_is_fetched_and_parsed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(html(&format!(
                r#"<title>T</title><a href="{}/app">a</a>"#,
                server.uri()
            )))
            .mount(&server)
            .await;
        let p = client(&server)
            .await
            .fetch_page(&format!("{}/", server.uri()))
            .await
            .unwrap();
        assert_eq!((p.status, p.title.as_str()), (200, "T"));
        assert_eq!(p.links, [format!("{}/app", server.uri())]);
    }

    #[tokio::test]
    async fn errors_name_the_status_and_the_content_type() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/data.json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string("{}"),
            )
            .mount(&server)
            .await;
        let c = client(&server).await;
        assert!(
            c.fetch_page(&format!("{}/missing", server.uri()))
                .await
                .unwrap_err()
                .contains("404")
        );
        assert!(
            c.fetch_page(&format!("{}/data.json", server.uri()))
                .await
                .unwrap_err()
                .contains("not HTML")
        );
    }

    #[tokio::test]
    async fn robots_txt_is_respected() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/robots.txt"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("User-agent: *\nDisallow: /private\n"),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(html("<title>ok</title>"))
            .mount(&server)
            .await;
        let c = client(&server).await;
        assert!(
            c.fetch_page(&format!("{}/public", server.uri()))
                .await
                .is_ok()
        );
        let err = c
            .fetch_page(&format!("{}/private/x", server.uri()))
            .await
            .unwrap_err();
        assert!(err.contains("robots"), "{err}");
    }

    #[tokio::test]
    async fn redirects_to_another_host_are_not_followed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/go"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("location", "http://169.254.169.254/latest/meta-data"),
            )
            .mount(&server)
            .await;
        let err = client(&server)
            .await
            .fetch_page(&format!("{}/go", server.uri()))
            .await
            .unwrap_err();
        assert!(
            err.contains("302") || err.to_lowercase().contains("redirect"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn other_hosts_are_refused_before_any_request() {
        let server = MockServer::start().await;
        let c = client(&server).await;
        let err = c.fetch_page("https://evil.example/x").await.unwrap_err();
        assert!(err.contains("not on the business website"), "{err}");
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn loopback_hosts_are_blocked_unless_explicitly_allowed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(html("<title>x</title>"))
            .mount(&server)
            .await;
        let strict = SiteClient::new(&server.uri(), false).unwrap();
        let err = strict
            .fetch_page(&format!("{}/", server.uri()))
            .await
            .unwrap_err();
        assert!(
            err.contains("private") || err.contains("IP address"),
            "{err}"
        );
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "no request may leave for a private address"
        );
    }

    #[tokio::test]
    async fn ip_literals_are_refused_when_private_hosts_are_blocked() {
        let strict = SiteClient::new("http://93.184.216.34/", false).unwrap();
        let err = strict
            .fetch_page("http://93.184.216.34/")
            .await
            .unwrap_err();
        assert!(err.contains("IP address"), "{err}");
        let v6 = SiteClient::new("http://[2606:4700::1111]/", false).unwrap();
        assert!(
            v6.fetch_page("http://[2606:4700::1111]/")
                .await
                .unwrap_err()
                .contains("IP address")
        );
        let _ = Ipv4Addr::LOCALHOST;
    }

    fn urlset(urls: &[String]) -> String {
        let items: String = urls
            .iter()
            .map(|u| format!("<url><loc>{u}</loc></url>"))
            .collect();
        format!(
            r#"<?xml version="1.0"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{items}</urlset>"#
        )
    }

    fn gz(text: &str) -> Vec<u8> {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(text.as_bytes()).unwrap();
        e.finish().unwrap()
    }

    #[tokio::test]
    async fn the_default_sitemap_is_read_and_other_hosts_are_dropped() {
        let server = MockServer::start().await;
        let base = server.uri();
        let body = urlset(&[
            format!("{base}/a"),
            format!("{base}/b"),
            "https://evil.example/c".to_string(),
        ]);
        Mock::given(method("GET"))
            .and(path("/sitemap.xml"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        let urls = client(&server).await.fetch_sitemap(None).await.unwrap();
        assert_eq!(urls, [format!("{base}/a"), format!("{base}/b")]);
    }

    #[tokio::test]
    async fn robots_sitemap_lines_are_followed_through_indexes_and_gzip() {
        let server = MockServer::start().await;
        let base = server.uri();
        Mock::given(method("GET"))
            .and(path("/robots.txt"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                "User-agent: *\nAllow: /\nSitemap: {base}/index.xml\n"
            )))
            .mount(&server)
            .await;
        let index = format!(
            r#"<sitemapindex><sitemap><loc>{base}/s1.xml</loc></sitemap><sitemap><loc>{base}/s2.xml.gz</loc></sitemap></sitemapindex>"#
        );
        Mock::given(method("GET"))
            .and(path("/index.xml"))
            .respond_with(ResponseTemplate::new(200).set_body_string(index))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/s1.xml"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(urlset(&[format!("{base}/one")])),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/s2.xml.gz"))
            .respond_with(
                ResponseTemplate::new(200).set_body_bytes(gz(&urlset(&[format!("{base}/two")]))),
            )
            .mount(&server)
            .await;
        let urls = client(&server).await.fetch_sitemap(None).await.unwrap();
        assert_eq!(urls, [format!("{base}/one"), format!("{base}/two")]);
    }

    #[test]
    fn xml_entities_in_sitemap_urls_are_decoded() {
        let xml = br#"<urlset><url><loc>https://vinellu.com/s?a=1&amp;b=2</loc></url></urlset>"#;
        let (children, pages) = parse_sitemap(xml);
        assert!(children.is_empty());
        assert_eq!(pages, ["https://vinellu.com/s?a=1&b=2"]);
    }

    #[tokio::test]
    async fn a_missing_sitemap_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let err = client(&server).await.fetch_sitemap(None).await.unwrap_err();
        assert!(err.contains("sitemap"), "{err}");
    }

    #[tokio::test]
    async fn the_url_cap_truncates_huge_sitemaps() {
        let server = MockServer::start().await;
        let base = server.uri();
        let urls: Vec<String> = (0..50).map(|i| format!("{base}/p/{i}")).collect();
        Mock::given(method("GET"))
            .and(path("/sitemap.xml"))
            .respond_with(ResponseTemplate::new(200).set_body_string(urlset(&urls)))
            .mount(&server)
            .await;
        let c = client(&server).await.with_sitemap_cap(10);
        assert_eq!(c.fetch_sitemap(None).await.unwrap().len(), 10);
    }

    #[tokio::test]
    async fn a_sitemap_url_on_another_host_is_refused() {
        let server = MockServer::start().await;
        let err = client(&server)
            .await
            .fetch_sitemap(Some("https://evil.example/sitemap.xml"))
            .await
            .unwrap_err();
        assert!(err.contains("not on the business website"), "{err}");
    }
}
