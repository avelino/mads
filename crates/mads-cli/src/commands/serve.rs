use std::path::PathBuf;

use uuid::Uuid;

use crate::cli::{ServeArgs, usage};

/// Starts `mads serve`. The server owns the process until Ctrl-C.
pub async fn run(args: ServeArgs) -> anyhow::Result<i32> {
    let root = args.root.unwrap_or_else(default_root);
    if !args.bind.is_loopback() && !args.allow_remote {
        return Err(usage(
            "--bind outside loopback needs --allow-remote and --token-file",
        ));
    }
    if args.allow_remote && args.token_file.is_none() {
        return Err(usage("--allow-remote needs --token-file"));
    }
    let token = match &args.token_file {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
            let token = text.trim().to_string();
            if token.len() < 32 {
                return Err(usage("--token-file must hold at least 32 characters"));
            }
            token
        }
        None => random_token(),
    };
    create_root(&root)?;
    let config = crate::serve::Config {
        root,
        bind: args.bind,
        port: args.port,
        token,
        allow_remote: args.allow_remote,
        open: args.open,
        print_token: args.token_file.is_none(),
    };
    crate::serve::serve(config).await?;
    Ok(0)
}

fn default_root() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mads")
}

fn random_token() -> String {
    let mut token = Uuid::new_v4().simple().to_string();
    token.push_str(&Uuid::new_v4().simple().to_string());
    token
}

fn create_root(root: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(root)
        .map_err(|e| anyhow::anyhow!("cannot create {}: {e}", root.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::*;

    #[test]
    fn random_tokens_are_long_and_distinct() {
        let a = random_token();
        let b = random_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }

    #[test]
    fn default_root_ends_with_mads() {
        assert_eq!(default_root().file_name().unwrap(), "mads");
    }

    #[test]
    fn loopback_check_accepts_v4_and_v6() {
        assert!(IpAddr::from([127, 0, 0, 1]).is_loopback());
        assert!("::1".parse::<IpAddr>().unwrap().is_loopback());
        assert!(!IpAddr::from([0, 0, 0, 0]).is_loopback());
    }
}
