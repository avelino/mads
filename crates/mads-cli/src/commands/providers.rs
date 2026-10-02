use mads_providers::{ProviderStatus, list_image_providers, list_providers};

pub fn run() -> anyhow::Result<i32> {
    println!("{:<16} {:<5} STATUS", "PROVIDER", "KIND");
    for p in list_providers() {
        let kind = match p.kind {
            mads_providers::ProviderKind::Api => "api",
            mads_providers::ProviderKind::Cli => "cli",
        };
        let marker = if p.status == ProviderStatus::Ready {
            "ready"
        } else {
            &p.status.describe()
        };
        println!("{:<16} {:<5} {marker}", p.name, kind);
    }
    println!("\n{:<16} {:<5} STATUS", "IMAGE PROVIDER", "");
    for (name, status) in list_image_providers() {
        println!("{name:<16} {:<5} {}", "", status.describe());
    }
    Ok(0)
}
