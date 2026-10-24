use std::{collections::HashMap, sync::Mutex};

use axum::http::{HeaderMap, header};
use subtle::ConstantTimeEq;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

pub const SESSION_COOKIE: &str = "mads_session";
const SESSION_TTL: Duration = Duration::hours(12);

#[derive(Debug, Clone)]
struct Session {
    csrf: String,
    lang: String,
    created: OffsetDateTime,
}

/// One process, one operator. The bootstrap token is exchanged for a session
/// cookie. Every mutation also needs the session CSRF token.
pub struct Auth {
    token: String,
    sessions: Mutex<HashMap<String, Session>>,
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub csrf: String,
    pub lang: String,
    #[allow(dead_code)]
    pub bearer: bool,
}

impl Auth {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn authenticate(&self, headers: &HeaderMap) -> Option<Identity> {
        if let Some(value) = bearer(headers)
            && same(value, &self.token)
        {
            return Some(Identity {
                csrf: String::new(),
                lang: "pt-BR".into(),
                bearer: true,
            });
        }
        let id = cookie_value(headers, SESSION_COOKIE)?;
        let now = OffsetDateTime::now_utc();
        let mut sessions = self.sessions.lock().ok()?;
        let session = sessions.get(&id)?;
        if session.created + SESSION_TTL < now {
            sessions.remove(&id);
            return None;
        }
        Some(Identity {
            csrf: session.csrf.clone(),
            lang: session.lang.clone(),
            bearer: false,
        })
    }

    /// Validates the bootstrap token and creates a session. Returns
    /// `(session_id, csrf_token)`, both random.
    #[cfg(test)]
    pub fn create_session(&self, token: &str) -> Option<(String, String)> {
        self.create_session_with_lang(token, "pt-BR")
    }

    pub fn create_session_with_lang(&self, token: &str, lang: &str) -> Option<(String, String)> {
        if !same(token, &self.token) {
            return None;
        }
        let id = new_secret();
        let csrf = new_secret();
        let session = Session {
            csrf: csrf.clone(),
            lang: lang.to_string(),
            created: OffsetDateTime::now_utc(),
        };
        let mut sessions = self.sessions.lock().ok()?;
        sessions.retain(|_, s| s.created + SESSION_TTL >= OffsetDateTime::now_utc());
        sessions.insert(id.clone(), session);
        Some((id, csrf))
    }

    pub fn set_language(&self, headers: &HeaderMap, lang: &str) -> bool {
        let Some(id) = cookie_value(headers, SESSION_COOKIE) else {
            return false;
        };
        let Ok(mut sessions) = self.sessions.lock() else {
            return false;
        };
        let Some(session) = sessions.get_mut(&id) else {
            return false;
        };
        session.lang = lang.to_string();
        true
    }

    pub fn destroy_session(&self, headers: &HeaderMap) {
        if let Some(id) = cookie_value(headers, SESSION_COOKIE)
            && let Ok(mut sessions) = self.sessions.lock()
        {
            sessions.remove(&id);
        }
    }

    pub fn cookie(id: &str, secure: bool) -> String {
        let secure = if secure { "; Secure" } else { "" };
        format!("{SESSION_COOKIE}={id}; Path=/; HttpOnly; SameSite=Strict; Max-Age=43200{secure}")
    }

    pub fn clear_cookie() -> String {
        format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0")
    }
}

pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let Some((key, value)) = part.trim().split_once('=') else {
            continue;
        };
        if key == name {
            return Some(value.to_string());
        }
    }
    None
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.as_bytes().ct_eq(b.as_bytes()).into()
}

fn new_secret() -> String {
    let mut value = Uuid::new_v4().simple().to_string();
    value.push_str(&Uuid::new_v4().simple().to_string());
    value
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (k, v) in pairs {
            map.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        map
    }

    #[test]
    fn a_session_can_be_created_and_used() {
        let auth = Auth::new("a".repeat(64));
        let (id, csrf) = auth.create_session(&"a".repeat(64)).unwrap();
        assert_eq!(csrf.len(), 64);
        let identity = auth
            .authenticate(&headers(&[("cookie", &format!("{SESSION_COOKIE}={id}"))]))
            .unwrap();
        assert_eq!(identity.csrf, csrf);
        assert!(!identity.bearer);
    }

    #[test]
    fn a_wrong_bootstrap_token_is_rejected() {
        let auth = Auth::new("a".repeat(64));
        assert!(auth.create_session(&"b".repeat(64)).is_none());
    }

    #[test]
    fn a_bearer_token_authenticates_without_a_cookie() {
        let auth = Auth::new("c".repeat(64));
        let identity = auth
            .authenticate(&headers(&[(
                "authorization",
                &format!("Bearer {}", "c".repeat(64)),
            )]))
            .unwrap();
        assert!(identity.bearer);
    }

    #[test]
    fn a_missing_session_is_rejected() {
        let auth = Auth::new("a".repeat(64));
        assert!(auth.authenticate(&HeaderMap::new()).is_none());
        assert!(
            auth.authenticate(&headers(&[("cookie", "mads_session=nope")]))
                .is_none()
        );
    }

    #[test]
    fn destroying_a_session_invalidates_the_cookie() {
        let auth = Auth::new("a".repeat(64));
        let (id, _) = auth.create_session(&"a".repeat(64)).unwrap();
        let h = headers(&[("cookie", &format!("{SESSION_COOKIE}={id}"))]);
        auth.destroy_session(&h);
        assert!(auth.authenticate(&h).is_none());
    }

    #[test]
    fn the_cookie_flags_are_http_only_and_strict() {
        let cookie = Auth::cookie("abc", false);
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(!cookie.contains("Secure"));
        assert!(Auth::cookie("abc", true).contains("Secure"));
    }
}
