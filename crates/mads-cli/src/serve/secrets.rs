use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

/// Provider name to the variable mads reads. The studio uses the same names.
pub const PROVIDERS: [(&str, Option<&str>); 10] = [
    ("anthropic", Some("ANTHROPIC_API_KEY")),
    ("openai", Some("OPENAI_API_KEY")),
    ("gemini", Some("GEMINI_API_KEY")),
    ("openrouter", Some("OPENROUTER_API_KEY")),
    ("groq", Some("GROQ_API_KEY")),
    ("deepseek", Some("DEEPSEEK_API_KEY")),
    ("xai", Some("XAI_API_KEY")),
    ("ollama", None),
    ("openai-compat", Some("MADS_API_KEY")),
    ("replay", None),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Env,
    File,
    Missing,
    NotNeeded,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Env => "variável de ambiente",
            Source::File => "arquivo 0600",
            Source::Missing => "ausente",
            Source::NotNeeded => "não precisa",
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyStatus {
    pub provider: String,
    pub source_label: String,
    pub ready: bool,
    pub needs_key: bool,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct KeyFile {
    #[serde(default)]
    keys: BTreeMap<String, String>,
}

pub struct Secrets {
    path: PathBuf,
    keys: Mutex<BTreeMap<String, String>>,
    error: Mutex<Option<String>>,
}

impl Secrets {
    pub fn new() -> Self {
        Self::at(default_path())
    }

    /// A store at an explicit path. Tests use it to stay away from the real config.
    pub fn at(path: PathBuf) -> Self {
        let (keys, error) = load(&path);
        Self {
            path,
            keys: Mutex::new(keys),
            error: Mutex::new(error),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|e| e.clone())
    }

    pub fn status(&self, provider: &str) -> KeyStatus {
        let env = env_for(provider);
        let source = match env {
            None => Source::NotNeeded,
            Some(var) if std::env::var_os(var).is_some_and(|v| !v.is_empty()) => Source::Env,
            Some(_) if self.stored(provider).is_some() => Source::File,
            Some(_) => Source::Missing,
        };
        KeyStatus {
            provider: provider.to_string(),
            source_label: source.label().to_string(),
            ready: matches!(source, Source::Env | Source::File | Source::NotNeeded),
            needs_key: env.is_some(),
        }
    }

    pub fn statuses(&self) -> Vec<KeyStatus> {
        PROVIDERS
            .iter()
            .map(|(provider, _)| self.status(provider))
            .collect()
    }

    /// Environment wins over the file. The map is never sent to the browser.
    pub fn resolved(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let stored = self.keys.lock().map(|k| k.clone()).unwrap_or_default();
        for (provider, env) in PROVIDERS {
            let Some(var) = env else { continue };
            let value = std::env::var(var)
                .ok()
                .filter(|v| !v.is_empty())
                .or_else(|| stored.get(provider).cloned());
            if let Some(value) = value {
                out.insert(var.to_string(), value);
            }
        }
        out
    }

    pub fn set(&self, provider: &str, value: &str) -> Result<(), String> {
        let value = value.trim();
        if value.is_empty() {
            return Err("a chave está vazia".into());
        }
        if !PROVIDERS.iter().any(|(name, _)| *name == provider) {
            return Err("provedor desconhecido".into());
        }
        self.require_loaded()?;
        let mut keys = self.keys.lock().map_err(|_| "store indisponível")?;
        keys.insert(provider.to_string(), value.to_string());
        write(&self.path, &keys)?;
        if let Ok(mut error) = self.error.lock() {
            *error = None;
        }
        Ok(())
    }

    pub fn remove(&self, provider: &str) -> Result<(), String> {
        if !PROVIDERS.iter().any(|(name, _)| *name == provider) {
            return Err("provedor desconhecido".into());
        }
        self.require_loaded()?;
        let mut keys = self.keys.lock().map_err(|_| "store indisponível")?;
        keys.remove(provider);
        write(&self.path, &keys)
    }

    /// A store that failed to load holds an empty map: writing it over the real
    /// file would silently destroy every saved key.
    fn require_loaded(&self) -> Result<(), String> {
        match self.error() {
            None => Ok(()),
            Some(error) => Err(format!(
                "o arquivo de chaves não foi carregado; resolva o problema antes de salvar: {error}"
            )),
        }
    }

    fn stored(&self, provider: &str) -> Option<String> {
        self.keys
            .lock()
            .ok()?
            .get(provider)
            .filter(|v| !v.is_empty())
            .cloned()
    }
}

pub fn env_for(provider: &str) -> Option<&'static str> {
    PROVIDERS
        .iter()
        .find(|(name, _)| *name == provider)
        .and_then(|(_, env)| *env)
}

fn default_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(xdg).join("mads").join("secrets.toml");
    }
    if let Some(appdata) = std::env::var_os("APPDATA").filter(|v| !v.is_empty()) {
        return PathBuf::from(appdata).join("mads").join("secrets.toml");
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("mads")
        .join("secrets.toml")
}

fn load(path: &Path) -> (BTreeMap<String, String>, Option<String>) {
    let Ok(meta) = fs::metadata(path) else {
        return (BTreeMap::new(), None);
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return (
                BTreeMap::new(),
                Some(format!(
                    "{} é legível por outros usuários. Rode: chmod 600 {}",
                    path.display(),
                    path.display()
                )),
            );
        }
    }
    let Ok(text) = fs::read_to_string(path) else {
        return (
            BTreeMap::new(),
            Some("não foi possível ler o arquivo de chaves".into()),
        );
    };
    match toml::from_str::<KeyFile>(&text) {
        Ok(file) => (file.keys, None),
        Err(e) => (
            BTreeMap::new(),
            Some(format!("arquivo de chaves inválido: {e}")),
        ),
    }
}

fn write(path: &Path, keys: &BTreeMap<String, String>) -> Result<(), String> {
    let parent = path.parent().ok_or("caminho de chaves inválido")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
    }
    let text = toml::to_string(&KeyFile { keys: keys.clone() }).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    // The temp file holds key material: create it 0600, never write first and
    // tighten later.
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_to_env_is_documented() {
        assert_eq!(env_for("anthropic"), Some("ANTHROPIC_API_KEY"));
        assert_eq!(env_for("openai-compat"), Some("MADS_API_KEY"));
        assert_eq!(env_for("ollama"), None);
        assert_eq!(env_for("nope"), None);
    }

    #[test]
    fn a_secret_file_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.toml");
        let mut keys = BTreeMap::new();
        keys.insert("anthropic".to_string(), "sk-test".to_string());
        write(&path, &keys).unwrap();
        let (loaded, error) = load(&path);
        assert!(error.is_none(), "{error:?}");
        assert_eq!(loaded.get("anthropic").map(String::as_str), Some("sk-test"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
