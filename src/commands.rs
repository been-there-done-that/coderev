use crate::{OutputMode, emit_success};
use coderev::ui::{Icons, section, success, banner};
use owo_colors::OwoColorize;

pub fn run_version(output_mode: OutputMode) -> anyhow::Result<()> {
    if output_mode.is_human() {
        banner(
            &format!("{}", "Coderev".bold().style(coderev::ui::theme().info.clone())),
            &format!("Version {}", env!("CARGO_PKG_VERSION").bold())
        );
    } else {
        let data = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
        });
        emit_success(output_mode, "version", data)?;
    }
    Ok(())
}

pub fn run_update(output_mode: OutputMode) -> anyhow::Result<()> {
    if output_mode.is_human() {
        banner(
            &format!("{}", "Update Check".bold().style(coderev::ui::theme().info.clone())),
            "Fetching latest release information..."
        );
        
        // Use curl to avoid extra dependencies for now
        let output = std::process::Command::new("curl")
            .arg("-s")
            .arg("https://api.github.com/repos/been-there-done-that/coderev/releases/latest")
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null);
                if let Some(tag) = json["tag_name"].as_str() {
                    let latest_version = tag.trim_start_matches('v');
                    let current_version = env!("CARGO_PKG_VERSION");
                    
                    if latest_version != current_version {
                        println!("{} New version available: {}", Icons::CHECK.style(coderev::ui::theme().success.clone()), tag.bold());
                        println!();
                        section("How to update:");
                        println!("  {} macOS/Linux (Homebrew):", Icons::INFO.style(coderev::ui::theme().dim.clone()));
                        println!("    {}", "brew upgrade coderev".bold().style(coderev::ui::theme().info.clone()));
                        println!();
                        println!("  {} Unix (Script):", Icons::INFO.style(coderev::ui::theme().dim.clone()));
                        println!("    {}", "curl -sSL https://raw.githubusercontent.com/been-there-done-that/coderev/main/install.sh | sh".bold().style(coderev::ui::theme().info.clone()));
                        println!();
                        println!("  {} Windows (PowerShell):", Icons::INFO.style(coderev::ui::theme().dim.clone()));
                        println!("    {}", "iwr https://raw.githubusercontent.com/been-there-done-that/coderev/main/install.ps1 | iex".bold().style(coderev::ui::theme().info.clone()));
                    } else {
                        success(&format!("You are on the latest version ({})", current_version));
                    }
                } else {
                    anyhow::bail!("Could not parse latest release information.");
                }
            }
            _ => {
                anyhow::bail!("Failed to connect to GitHub. Check your internet connection.");
            }
        }
    } else {
         let data = serde_json::json!({
            "current": env!("CARGO_PKG_VERSION"),
            "note": "Use human mode for detailed instructions",
        });
        emit_success(output_mode, "update", data)?;
    }
    Ok(())
}

pub fn run_upgrade(output_mode: OutputMode) -> anyhow::Result<()> {
    if output_mode.is_human() {
        banner(
            &format!("{}", "Upgrade".bold().style(coderev::ui::theme().info.clone())),
            "Checking environment and starting upgrade..."
        );
        
        // 1. Check if Homebrew is used
        let brew_check = std::process::Command::new("brew")
            .arg("list")
            .arg("coderev")
            .output();

        if let Ok(out) = brew_check {
            if out.status.success() {
                println!("{} Homebrew installation detected.", Icons::INFO.style(coderev::ui::theme().dim.clone()));
                println!("{} Running: {}", Icons::ROCKET, "brew upgrade coderev".bold().style(coderev::ui::theme().info.clone()));
                
                let mut child = std::process::Command::new("brew")
                    .arg("upgrade")
                    .arg("coderev")
                    .spawn()?;
                
                let status = child.wait()?;
                if status.success() {
                    success("Successfully upgraded via Homebrew!");
                } else {
                    anyhow::bail!("Homebrew upgrade failed.");
                }
                return Ok(());
            }
        }

        // 2. Fallback to OS-specific script
        #[cfg(unix)]
        {
            println!("{} Direct installation detected.", Icons::INFO.style(coderev::ui::theme().dim.clone()));
            println!("{} Running: {}", Icons::ROCKET, "curl -sSL ... | sh".bold().style(coderev::ui::theme().info.clone()));
             
            let mut child = std::process::Command::new("sh")
                .arg("-c")
                .arg("curl -sSL https://raw.githubusercontent.com/been-there-done-that/coderev/main/install.sh | sh")
                .spawn()?;
             
            let status = child.wait()?;
            if status.success() {
                success("Successfully upgraded via script!");
            } else {
                anyhow::bail!("Upgrade script failed.");
            }
        }
        
        #[cfg(windows)]
        {
            println!("{} Windows installation detected.", Icons::INFO.style(coderev::ui::theme().dim.clone()));
            println!("{} Running: {}", Icons::ROCKET, "PowerShell -c iwr ... | iex".bold().style(coderev::ui::theme().info.clone()));
             
            let mut child = std::process::Command::new("powershell")
                .arg("-Command")
                .arg("iwr https://raw.githubusercontent.com/been-there-done-that/coderev/main/install.ps1 | iex")
                .spawn()?;
             
            let status = child.wait()?;
            if status.success() {
                success("Successfully upgraded!");
            } else {
                anyhow::bail!("Upgrade failed.");
            }
        }

    } else {
        anyhow::bail!("Upgrade command only supported in human mode.");
    }
    Ok(())
}
