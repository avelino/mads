//! The image step: turns the briefs of image campaigns into files after the missions.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::{sync::Semaphore, task::JoinSet};

use super::{ImageModel, ImageRequest, check_image, extension, fit_to_ratio};
use crate::{
    events::{Event, EventSink},
    google::{AspectRatio, Campaign},
    input::{Input, slugify},
    tools::SharedWorkspace,
    web::Web,
};

/// Folder under the platform directory for files Google Ads Editor imports.
pub const EDITOR_DIR: &str = "editor";
const REFERENCE_MAX_BYTES: usize = 10 * 1024 * 1024;
const ATTEMPTS: usize = 2;
/// Appended to every brief: Google overlays the ad text and logo itself.
const CLEAN_PICTURE: &str =
    "No text, letters, numbers, logos, watermarks, borders or user interface in the picture.";
const USE_REFERENCE: &str = "Show the product from the reference photo exactly as it is: same shape, colors and label. Do not redraw or invent its label.";

/// The prompt the image model receives.
pub fn model_prompt(brief_prompt: &str, has_reference: bool, palette: Option<&str>) -> String {
    let mut p = brief_prompt.trim().to_string();
    if let Some(line) = palette {
        p.push_str("\n\n");
        p.push_str(line);
    }
    if has_reference {
        p.push_str("\n\n");
        p.push_str(USE_REFERENCE);
    }
    p.push_str("\n\n");
    if has_reference {
        p.push_str("Apart from the product's own label: ");
    }
    p.push_str(CLEAN_PICTURE);
    p
}

#[derive(Clone)]
pub struct ImageStepConfig {
    /// `google-ads/editor/`: the CSV and the `images/` folder live here.
    pub dir: PathBuf,
    /// Saved after every new image, so an interrupted run keeps what it paid for.
    pub workspace_path: Option<PathBuf>,
    pub model: Option<Arc<dyn ImageModel>>,
    /// New images allowed in this run.
    pub max_new: usize,
    pub parallel: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImageStepResult {
    pub model: Option<String>,
    pub generated: usize,
    pub reused: usize,
    /// One line per image that could not be made, for the report.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct Job {
    at: (usize, usize, usize),
    label: String,
    rel: String,
    request_prompt: String,
    ratio: AspectRatio,
    reference_url: Option<String>,
    /// The brand colors line from DESIGN.md, the same for every job.
    palette: Option<String>,
}

/// `images/<campaign>/<group>-<id>.jpg`, relative to the editor folder.
pub fn image_rel_path(campaign: &str, group: &str, id: &str) -> String {
    format!("images/{}/{}-{}.jpg", campaign, slugify(group), slugify(id))
}

/// `images/logo.<ext>`, relative to the editor folder. None without a logo.
pub fn logo_rel_path(input: &Input) -> Option<String> {
    let ext = Path::new(input.logo.as_deref()?)
        .extension()?
        .to_string_lossy()
        .to_lowercase();
    Some(format!("images/logo.{ext}"))
}

fn file_ok(dir: &Path, rel: &str, ratio: AspectRatio) -> bool {
    std::fs::read(dir.join(rel)).is_ok_and(|b| check_image(&b, ratio).is_ok())
}

/// Keeps the briefs whose file is still good and returns the others as jobs.
fn plan_jobs(campaigns: &mut [Campaign], input: &Input, dir: &Path) -> (Vec<Job>, usize) {
    let mut jobs = Vec::new();
    let mut reused = 0;
    for (ci, c) in campaigns.iter_mut().enumerate() {
        if !c.kind.has_images() {
            continue;
        }
        for (gi, g) in c.asset_groups.iter_mut().enumerate() {
            for (bi, b) in g.images.iter_mut().enumerate() {
                let rel = image_rel_path(&c.slug, &g.name, &b.id);
                if b.file.as_deref() == Some(rel.as_str()) && file_ok(dir, &rel, b.ratio) {
                    reused += 1;
                    continue;
                }
                b.file = None;
                let reference_url = b.reference.as_ref().and_then(|id| {
                    input
                        .catalog
                        .iter()
                        .find(|e| &e.id == id)
                        .and_then(|e| e.image.clone())
                });
                jobs.push(Job {
                    at: (ci, gi, bi),
                    label: format!("{}/{}/{}", c.name, g.name, b.id),
                    rel,
                    request_prompt: b.prompt.clone(),
                    ratio: b.ratio,
                    reference_url,
                    palette: super::palette_line(&input.design),
                });
            }
        }
    }
    (jobs, reused)
}

fn copy_logo(input: &Input, dir: &Path) -> Result<(), String> {
    let (Some(src), Some(rel)) = (input.logo.as_deref(), logo_rel_path(input)) else {
        return Ok(());
    };
    let dest = dir.join(rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::copy(src, &dest)
        .map(|_| ())
        .map_err(|e| format!("cannot copy the logo {src}: {e}"))
}

async fn download_references(
    jobs: &[Job],
    web: &dyn Web,
) -> BTreeMap<String, Result<Arc<Vec<u8>>, String>> {
    let mut out = BTreeMap::new();
    for url in jobs.iter().filter_map(|j| j.reference_url.clone()) {
        if out.contains_key(&url) {
            continue;
        }
        let got = web.fetch_bytes(&url, REFERENCE_MAX_BYTES).await;
        let checked = got.and_then(|b| match extension(&b) {
            Some(_) => Ok(Arc::new(b)),
            None => Err(format!("{url} is not a PNG or JPEG")),
        });
        out.insert(url, checked);
    }
    out
}

async fn make_one(model: &dyn ImageModel, req: &ImageRequest) -> Result<Vec<u8>, String> {
    let mut last = String::new();
    for _ in 0..ATTEMPTS {
        let made = model
            .generate(req)
            .await
            .and_then(|raw| fit_to_ratio(&raw, req.ratio))
            .and_then(|jpg| check_image(&jpg, req.ratio).map(|()| jpg));
        match made {
            Ok(jpg) => return Ok(jpg),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn write_file(dir: &Path, rel: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Records a new file on its brief, unless the brief changed meanwhile.
async fn record(ws: &SharedWorkspace, job: &Job, cfg: &ImageStepConfig) -> Result<(), String> {
    let mut guard = ws.lock().await;
    let (ci, gi, bi) = job.at;
    let brief = guard
        .account
        .campaigns
        .get_mut(ci)
        .and_then(|c| c.asset_groups.get_mut(gi))
        .and_then(|g| g.images.get_mut(bi))
        .filter(|b| b.prompt == job.request_prompt)
        .ok_or("the brief changed while its image was made")?;
    brief.file = Some(job.rel.clone());
    match &cfg.workspace_path {
        Some(p) => guard.save(p).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

async fn run_job(
    job: Job,
    reference: Option<Result<Arc<Vec<u8>>, String>>,
    ws: SharedWorkspace,
    cfg: ImageStepConfig,
    model: Arc<dyn ImageModel>,
    events: EventSink,
) -> Result<(), String> {
    let reference = match reference {
        Some(Ok(bytes)) => Some(bytes.as_ref().clone()),
        Some(Err(e)) => return Err(format!("{}: reference photo: {e}", job.label)),
        None => None,
    };
    let req = ImageRequest {
        prompt: model_prompt(
            &job.request_prompt,
            reference.is_some(),
            job.palette.as_deref(),
        ),
        ratio: job.ratio,
        reference,
    };
    let jpg = make_one(model.as_ref(), &req)
        .await
        .map_err(|e| format!("{}: {e}", job.label))?;
    let path = write_file(&cfg.dir, &job.rel, &jpg).map_err(|e| format!("{}: {e}", job.label))?;
    record(&ws, &job, &cfg)
        .await
        .map_err(|e| format!("{}: {e}", job.label))?;
    events.emit(Event::ArtifactWritten {
        path: path.display().to_string(),
    });
    Ok(())
}

fn step(events: &EventSink, detail: String) {
    events.emit(Event::Step {
        name: "images".into(),
        detail,
    });
}

/// Generates the missing images of every image campaign, within the run's cap.
pub async fn run_image_step(
    ws: &SharedWorkspace,
    web: &dyn Web,
    cfg: &ImageStepConfig,
    events: &EventSink,
) -> ImageStepResult {
    let mut result = ImageStepResult {
        model: cfg.model.as_ref().map(|m| m.id()),
        ..Default::default()
    };
    let (jobs, input) = {
        let mut guard = ws.lock().await;
        if !guard.account.campaigns.iter().any(|c| c.kind.has_images()) {
            return result;
        }
        let input = guard.input.clone();
        let (jobs, reused) = plan_jobs(&mut guard.account.campaigns, &input, &cfg.dir);
        result.reused = reused;
        (fair_order(jobs), input)
    };
    if let Err(e) = copy_logo(&input, &cfg.dir) {
        result.notes.push(e);
    }
    if jobs.is_empty() {
        step(
            events,
            format!("{} reused, nothing to generate", result.reused),
        );
        return result;
    }
    let Some(model) = cfg.model.clone() else {
        let msg = format!("{} images missing and no image model", jobs.len());
        step(events, msg.clone());
        result.notes.push(msg);
        return result;
    };
    generate_all(jobs, ws, web, cfg, model, events, &mut result).await;
    result
}

/// Orders the jobs so a cap is shared: the first picture of every group, then the second of
/// every group, and so on. Without it, one group with many briefs can take the whole cap and
/// leave another group with none, which stops the export.
fn fair_order(jobs: Vec<Job>) -> Vec<Job> {
    let mut seen: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut ranked: Vec<(usize, Job)> = jobs
        .into_iter()
        .map(|j| {
            let n = seen.entry((j.at.0, j.at.1)).or_insert(0);
            *n += 1;
            (*n, j)
        })
        .collect();
    // Stable: within one round the original order (campaign, group) is kept.
    ranked.sort_by_key(|(rank, _)| *rank);
    ranked.into_iter().map(|(_, j)| j).collect()
}

/// True when the asset group still meets Google's minimum pictures without brief `bi`.
fn can_drop(input: &Input, c: &Campaign, gi: usize, bi: usize) -> bool {
    let mut candidate = c.clone();
    let Some(group) = candidate.asset_groups.get_mut(gi) else {
        return false;
    };
    if bi >= group.images.len() || group.images.len() < 2 {
        return false;
    }
    group.images.remove(bi);
    let prefix = format!("c.asset_groups[{gi}].images");
    !crate::google::Rules::new(input, usize::MAX)
        .campaign(&candidate, None, false, "c")
        .iter()
        .any(|i| i.code == "E16" && i.path.starts_with(&prefix))
}

/// Jobs over the run's cap: a brief the asset group can do without is removed, so the export
/// still runs. One it cannot do without stays and stops the export with E20.
async fn over_the_cap(
    jobs: &mut Vec<Job>,
    ws: &SharedWorkspace,
    cfg: &ImageStepConfig,
    result: &mut ImageStepResult,
) {
    if jobs.len() <= cfg.max_new {
        return;
    }
    let skipped = jobs.split_off(cfg.max_new);
    let mut guard = ws.lock().await;
    let input = guard.input.clone();
    // Later briefs first, so removing one never shifts the index of another.
    for job in skipped.iter().rev() {
        let (ci, gi, bi) = job.at;
        let droppable = guard
            .account
            .campaigns
            .get(ci)
            .is_some_and(|c| can_drop(&input, c, gi, bi));
        let limit = cfg.max_new;
        if droppable {
            guard.account.campaigns[ci].asset_groups[gi]
                .images
                .remove(bi);
            result.notes.push(format!(
                "{}: dropped, over the limit of {limit} new images, the asset group has enough without it",
                job.label
            ));
        } else {
            result.notes.push(format!(
                "{}: over the limit of {limit} new images",
                job.label
            ));
        }
    }
    if let Some(p) = &cfg.workspace_path
        && let Err(e) = guard.save(p)
    {
        result.notes.push(format!("cannot save the workspace: {e}"));
    }
}

async fn generate_all(
    mut jobs: Vec<Job>,
    ws: &SharedWorkspace,
    web: &dyn Web,
    cfg: &ImageStepConfig,
    model: Arc<dyn ImageModel>,
    events: &EventSink,
    result: &mut ImageStepResult,
) {
    over_the_cap(&mut jobs, ws, cfg, result).await;
    step(
        events,
        format!("{} to generate with {}", jobs.len(), model.id()),
    );
    let references = download_references(&jobs, web).await;
    let gate = Arc::new(Semaphore::new(cfg.parallel.max(1)));
    let mut set = JoinSet::new();
    for job in jobs {
        let reference = job
            .reference_url
            .as_ref()
            .and_then(|u| references.get(u).cloned());
        let (ws, cfg, model, events, gate) = (
            ws.clone(),
            cfg.clone(),
            model.clone(),
            events.clone(),
            gate.clone(),
        );
        set.spawn(async move {
            let _permit = gate.acquire().await;
            run_job(job, reference, ws, cfg, model, events).await
        });
    }
    while let Some(done) = set.join_next().await {
        match done {
            Ok(Ok(())) => result.generated += 1,
            Ok(Err(e)) => {
                step(events, format!("failed {e}"));
                result.notes.push(e);
            }
            Err(e) => result.notes.push(format!("image task stopped: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use super::*;
    use crate::{
        google::{AssetGroup, BidStrategy, CampaignKind, Cents, ImageBrief, Intent},
        images::solid_png,
        testutil,
        workspace::Workspace,
    };

    struct Recorder(StdMutex<Vec<Option<usize>>>);

    #[async_trait]
    impl ImageModel for Recorder {
        fn id(&self) -> String {
            "rec".into()
        }
        async fn generate(&self, req: &ImageRequest) -> Result<Vec<u8>, String> {
            if let Ok(mut seen) = self.0.lock() {
                seen.push(req.reference.as_ref().map(Vec::len));
            }
            Ok(solid_png(req.ratio, [0, 0, 0]))
        }
    }

    struct PhotoWeb(bool);

    #[async_trait]
    impl Web for PhotoWeb {
        async fn check_url(&self, _url: &str) -> Result<u16, String> {
            Ok(200)
        }
        async fn fetch_bytes(&self, url: &str, _limit: usize) -> Result<Vec<u8>, String> {
            if self.0 {
                Ok(solid_png(AspectRatio::Square, [9, 9, 9]))
            } else {
                Err(format!("{url}: HTTP 404"))
            }
        }
    }

    fn workspace() -> SharedWorkspace {
        let mut input = testutil::input();
        input.catalog[0].image = Some("https://cdn.x/a.jpg".into());
        let mut ws = Workspace::new(input);
        let brief = |id: &str, reference: Option<&str>| ImageBrief {
            id: id.into(),
            ratio: AspectRatio::Square,
            prompt: "a prompt long enough to pass".into(),
            reference: reference.map(String::from),
            file: None,
        };
        ws.account.campaigns.push(Campaign {
            name: "C".into(),
            slug: "c".into(),
            kind: CampaignKind::PerformanceMax,
            intent: Intent::Generic,
            daily_budget: Cents(100),
            bid_strategy: BidStrategy::MaximizeConversions,
            rationale: String::new(),
            planned_ad_groups: vec![],
            ad_groups: vec![],
            asset_groups: vec![AssetGroup {
                name: "G".into(),
                final_url: String::new(),
                business_name: String::new(),
                headlines: vec![],
                long_headlines: vec![],
                descriptions: vec![],
                search_themes: vec![],
                images: vec![brief("a", Some("alamos-malbec")), brief("b", None)],
            }],
            negatives: vec![],
            assets: None,
        });
        Arc::new(Mutex::new(ws))
    }

    fn cfg(dir: &Path, model: Arc<dyn ImageModel>) -> ImageStepConfig {
        ImageStepConfig {
            dir: dir.to_path_buf(),
            workspace_path: None,
            model: Some(model),
            max_new: 10,
            parallel: 2,
        }
    }

    #[tokio::test]
    async fn the_reference_photo_reaches_the_model_once_per_brief() {
        let dir = tempfile::tempdir().unwrap();
        let rec = Arc::new(Recorder(StdMutex::new(Vec::new())));
        let ws = workspace();
        let (events, _rx) = EventSink::channel();
        let r = run_image_step(&ws, &PhotoWeb(true), &cfg(dir.path(), rec.clone()), &events).await;
        assert_eq!(r.generated, 2, "{:?}", r.notes);
        let mut seen = rec.0.lock().unwrap().clone();
        seen.sort();
        assert_eq!(seen[0], None);
        assert!(seen[1].is_some_and(|n| n > 0));
        let files: Vec<_> = ws.lock().await.account.campaigns[0].asset_groups[0]
            .images
            .iter()
            .map(|b| b.file.clone())
            .collect();
        assert_eq!(
            files,
            [
                Some("images/c/g-a.jpg".into()),
                Some("images/c/g-b.jpg".into())
            ]
        );
    }

    #[tokio::test]
    async fn a_reference_that_cannot_be_downloaded_fails_only_its_image() {
        let dir = tempfile::tempdir().unwrap();
        let rec = Arc::new(Recorder(StdMutex::new(Vec::new())));
        let ws = workspace();
        let (events, _rx) = EventSink::channel();
        let r = run_image_step(&ws, &PhotoWeb(false), &cfg(dir.path(), rec), &events).await;
        assert_eq!(r.generated, 1);
        assert!(
            r.notes[0].contains("reference photo") && r.notes[0].contains("404"),
            "{:?}",
            r.notes
        );
        let images = ws.lock().await.account.campaigns[0].asset_groups[0]
            .images
            .clone();
        assert_eq!(images[0].file, None);
    }

    #[test]
    fn the_model_prompt_forbids_text_and_keeps_the_real_product() {
        let p = model_prompt("  A table  ", true, None);
        assert!(p.starts_with("A table\n\n"));
        assert!(p.contains("reference photo") && p.ends_with("user interface in the picture."));
        assert!(p.contains("Apart from the product's own label: No text"));
        assert!(!model_prompt("A table", false, None).contains("reference"));
        let branded = model_prompt("A bus", false, Some("Brand colors: pink (#F0476A)."));
        assert!(branded.starts_with("A bus\n\nBrand colors: pink (#F0476A).\n\nNo text"));
    }

    #[tokio::test]
    async fn without_a_model_nothing_is_generated_and_the_report_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let ws = workspace();
        let (events, _rx) = EventSink::channel();
        let mut c = cfg(dir.path(), Arc::new(Recorder(StdMutex::new(Vec::new()))));
        c.model = None;
        let r = run_image_step(&ws, &PhotoWeb(true), &c, &events).await;
        assert_eq!(r.generated, 0);
        assert!(r.notes[0].contains("no image model"));
    }
}

#[cfg(test)]
mod design_tests {
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use super::*;
    use crate::{
        google::{AssetGroup, BidStrategy, CampaignKind, Cents, ImageBrief, Intent},
        images::solid_png,
        testutil,
        workspace::Workspace,
    };

    struct Prompts(StdMutex<Vec<String>>);

    #[async_trait]
    impl ImageModel for Prompts {
        fn id(&self) -> String {
            "prompts".into()
        }
        async fn generate(&self, req: &ImageRequest) -> Result<Vec<u8>, String> {
            if let Ok(mut p) = self.0.lock() {
                p.push(req.prompt.clone());
            }
            Ok(solid_png(req.ratio, [0, 0, 0]))
        }
    }

    struct NoWeb;

    #[async_trait]
    impl Web for NoWeb {
        async fn check_url(&self, _url: &str) -> Result<u16, String> {
            Ok(200)
        }
    }

    #[tokio::test]
    async fn every_prompt_carries_the_design_palette() {
        let mut input = testutil::input();
        input.design = "# Design\n\n## Colors\n\n- Pink #EE395D: logo\n".into();
        let mut ws = Workspace::new(input);
        ws.account.campaigns.push(Campaign {
            name: "C".into(),
            slug: "c".into(),
            kind: CampaignKind::DemandGen,
            intent: Intent::Generic,
            daily_budget: Cents(100),
            bid_strategy: BidStrategy::MaximizeClicks { max_cpc: None },
            rationale: String::new(),
            planned_ad_groups: vec![],
            ad_groups: vec![],
            asset_groups: vec![AssetGroup {
                name: "G".into(),
                final_url: String::new(),
                business_name: String::new(),
                headlines: vec![],
                long_headlines: vec![],
                descriptions: vec![],
                search_themes: vec![],
                images: vec![ImageBrief {
                    id: "a".into(),
                    ratio: AspectRatio::Square,
                    prompt: "A traveler at a terminal".into(),
                    reference: None,
                    file: None,
                }],
            }],
            negatives: vec![],
            assets: None,
        });
        let ws = Arc::new(Mutex::new(ws));
        let dir = tempfile::tempdir().unwrap();
        let model = Arc::new(Prompts(StdMutex::new(Vec::new())));
        let cfg = ImageStepConfig {
            dir: dir.path().to_path_buf(),
            workspace_path: None,
            model: Some(model.clone()),
            max_new: 5,
            parallel: 1,
        };
        let (events, _rx) = EventSink::channel();
        run_image_step(&ws, &NoWeb, &cfg, &events).await;
        let prompts = model.0.lock().unwrap().clone();
        assert_eq!(prompts.len(), 1);
        assert!(
            prompts[0].contains("Brand colors: pink (#EE395D)."),
            "{}",
            prompts[0]
        );
    }
}

#[cfg(test)]
mod fair_tests {
    use super::*;

    fn job(ci: usize, gi: usize, bi: usize) -> Job {
        Job {
            at: (ci, gi, bi),
            label: format!("{ci}/{gi}/{bi}"),
            rel: String::new(),
            request_prompt: String::new(),
            ratio: AspectRatio::Square,
            reference_url: None,
            palette: None,
        }
    }

    #[test]
    fn the_cap_is_shared_across_groups_first_picture_of_each_group_first() {
        // An App group with 4 briefs, then two Demand Gen groups: a cap of 3 must reach all three.
        let jobs = vec![
            job(0, 0, 0),
            job(0, 0, 1),
            job(0, 0, 2),
            job(0, 0, 3),
            job(1, 0, 0),
            job(1, 0, 1),
            job(1, 1, 0),
        ];
        let order: Vec<String> = fair_order(jobs).into_iter().map(|j| j.label).collect();
        assert_eq!(
            order,
            [
                "0/0/0", "1/0/0", "1/1/0", "0/0/1", "1/0/1", "0/0/2", "0/0/3"
            ]
        );
    }
}

#[cfg(test)]
mod cap_tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use super::*;
    use crate::{
        google::{AssetGroup, BidStrategy, CampaignKind, Cents, ImageBrief, Intent},
        images::SolidImageModel,
        testutil,
        workspace::Workspace,
    };

    struct NoWeb;

    #[async_trait]
    impl Web for NoWeb {
        async fn check_url(&self, _url: &str) -> Result<u16, String> {
            Ok(200)
        }
    }

    fn group(name: &str, n: usize) -> AssetGroup {
        let ratios = [
            AspectRatio::Landscape,
            AspectRatio::Square,
            AspectRatio::Portrait,
        ];
        AssetGroup {
            name: name.into(),
            final_url: String::new(),
            business_name: String::new(),
            headlines: vec![],
            long_headlines: vec![],
            descriptions: vec![],
            search_themes: vec![],
            images: (0..n)
                .map(|i| ImageBrief {
                    id: format!("p{i}"),
                    ratio: ratios[i % ratios.len()],
                    prompt: format!("a prompt long enough number {i}"),
                    reference: None,
                    file: None,
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn a_big_group_does_not_starve_a_small_one() {
        // The Vinellu run: 8 briefs in the first group, 1 in the last, a cap that only fits 3.
        let mut ws = Workspace::new(testutil::input());
        ws.account.campaigns.push(Campaign {
            name: "C".into(),
            slug: "c".into(),
            kind: CampaignKind::DemandGen,
            intent: Intent::Generic,
            daily_budget: Cents(100),
            bid_strategy: BidStrategy::MaximizeClicks { max_cpc: None },
            rationale: String::new(),
            planned_ad_groups: vec![],
            ad_groups: vec![],
            asset_groups: vec![group("big", 8), group("small", 1)],
            negatives: vec![],
            assets: None,
        });
        let ws = Arc::new(Mutex::new(ws));
        let dir = tempfile::tempdir().unwrap();
        let cfg = ImageStepConfig {
            dir: dir.path().to_path_buf(),
            workspace_path: None,
            model: Some(Arc::new(SolidImageModel)),
            max_new: 3,
            parallel: 2,
        };
        let (events, _rx) = EventSink::channel();
        let r = run_image_step(&ws, &NoWeb, &cfg, &events).await;
        assert_eq!(r.generated, 3, "{:?}", r.notes);
        let groups = ws.lock().await.account.campaigns[0].asset_groups.clone();
        assert!(
            groups[1].images[0].file.is_some(),
            "the small group got its picture"
        );
        assert!(
            groups[0].images.iter().all(|b| b.file.is_some()),
            "the big group kept only what it got"
        );
        assert_eq!(groups[0].images.len(), 2);
    }
}
