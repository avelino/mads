use std::{fs::OpenOptions, io::Write, path::Path};

use serde_json::json;
use time::OffsetDateTime;

/// Appends one audit line under `<base>/.mads/audit.ndjson`. Best effort:
/// a full disk or a read only project must not fail the action.
pub fn append(base: &Path, action: &str, detail: &str) {
    let dir = base.join(".mads");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("audit.ndjson");
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let Ok(mut file) = options.open(&path) else {
        return;
    };
    let ts = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let line = json!({"ts": ts, "action": action, "detail": detail});
    if let Ok(text) = serde_json::to_string(&line) {
        let _ = writeln!(file, "{text}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_audit_line_is_appended() {
        let dir = tempfile::tempdir().unwrap();
        append(dir.path(), "job_started", "generate 20261024-153012-a1b2c3");
        append(dir.path(), "job_finished", "done");
        let text = std::fs::read_to_string(dir.path().join(".mads/audit.ndjson")).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("job_started"));
        assert!(text.contains("job_finished"));
    }
}
