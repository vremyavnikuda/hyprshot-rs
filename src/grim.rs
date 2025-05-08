use anyhow::{Context, Result};
use log::info;
use notify_rust::Notification;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::fs::create_dir_all;
use std::io::Write;

pub fn save_geometry_with_grim(
    geometry: &str,
    save_fullpath: &PathBuf,
    clipboard_only: bool,
    raw: bool,
    command: Option<Vec<String>>,
    silent: bool,
    notif_timeout: u32,
    debug: bool,
) -> Result<()> {
    if debug {
        info!("Saving geometry with grim: {}", geometry);
    }

    if raw {
        let output = Command::new("grim")
            .arg("-g")
            .arg(geometry)
            .arg("-")
            .output()
            .context("Failed to run grim")?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("grim failed to capture screenshot"));
        }
        std::io::stdout().write_all(&output.stdout)?;
        return Ok(());
    }

    if !clipboard_only {
        create_dir_all(save_fullpath.parent().unwrap())
            .context("Failed to create screenshot directory")?;
        let grim_status = Command::new("grim")
            .arg("-g")
            .arg(geometry)
            .arg(save_fullpath)
            .status()
            .context("Failed to run grim")?;
        if !grim_status.success() {
            return Err(anyhow::anyhow!("grim failed to capture screenshot"));
        }

        let wl_copy_status = Command::new("wl-copy")
            .arg("--type")
            .arg("image/png")
            .stdin(std::fs::File::open(save_fullpath).context(format!(
                "Failed to open screenshot file '{}'",
                save_fullpath.display()
            ))?)
            .status()
            .context("Failed to run wl-copy")?;
        if !wl_copy_status.success() {
            return Err(anyhow::anyhow!("wl-copy failed to copy screenshot"));
        }

        if let Some(cmd) = command {
            let cmd_status = Command::new(&cmd[0])
                .args(&cmd[1..])
                .arg(save_fullpath)
                .status()
                .context(format!("Failed to run command '{}'", cmd[0]))?;
            if !cmd_status.success() {
                return Err(anyhow::anyhow!("Command '{}' failed", cmd[0]));
            }
        }
    } else {
        let grim_output = Command::new("grim")
            .arg("-g")
            .arg(geometry)
            .arg("-")
            .output()
            .context("Failed to run grim")?;
        if !grim_output.status.success() {
            return Err(anyhow::anyhow!("grim failed to capture screenshot"));
        }

        let mut wl_copy = Command::new("wl-copy")
            .arg("--type")
            .arg("image/png")
            .stdin(Stdio::piped())
            .spawn()
            .context("Failed to start wl-copy")?;
        wl_copy
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&grim_output.stdout)
            .context("Failed to write to wl-copy stdin")?;
        let wl_copy_status = wl_copy.wait().context("Failed to wait for wl-copy")?;
        if !wl_copy_status.success() {
            return Err(anyhow::anyhow!("wl-copy failed to copy screenshot"));
        }
    }

    if !silent {
        let message = if clipboard_only {
            "Image copied to the clipboard".to_string()
        } else {
            format!(
                "Image saved in <i>{}</i> and copied to the clipboard.",
                save_fullpath.display()
            )
        };
        Notification::new()
            .summary("Screenshot saved")
            .body(&message)
            .icon(save_fullpath.to_str().unwrap_or("screenshot"))
            .timeout(notif_timeout as i32)
            .appname("Hyprshot-rs")
            .show()
            .context("Failed to show notification")?;
    }

    Ok(())
} 