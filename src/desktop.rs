use anyhow::{Context, Result};
use log::info;
use notify_rust::Notification;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::fs::create_dir_all;
use std::io::Write;

pub fn save_geometry_with_kde(
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
        info!("Saving geometry with KDE Spectacle: {}", geometry);
    }

    // Parse geometry for KDE format
    let parts: Vec<&str> = geometry.split(' ').collect();
    let coords: Vec<&str> = parts[0].split(',').collect();
    let dims: Vec<&str> = parts[1].split('x').collect();
    let x = coords[0].parse::<i32>()?;
    let y = coords[1].parse::<i32>()?;
    let width = dims[0].parse::<i32>()?;
    let height = dims[1].parse::<i32>()?;

    if !clipboard_only {
        create_dir_all(save_fullpath.parent().unwrap())
            .context("Failed to create screenshot directory")?;
    }

    let mut spectacle = Command::new("spectacle");
    spectacle
        .arg("--background")
        .arg("--nonotify")
        .arg("--region")
        .arg(format!("{}x{}+{}+{}", width, height, x, y));

    if clipboard_only {
        spectacle.arg("--clipboard");
    } else {
        spectacle.arg("--output").arg(save_fullpath);
    }

    let status = spectacle.status().context("Failed to run spectacle")?;
    if !status.success() {
        return Err(anyhow::anyhow!("spectacle failed to capture screenshot"));
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

pub fn save_geometry_with_gnome(
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
        info!("Saving geometry with GNOME Screenshot: {}", geometry);
    }

    // Parse geometry for GNOME format
    let parts: Vec<&str> = geometry.split(' ').collect();
    let coords: Vec<&str> = parts[0].split(',').collect();
    let dims: Vec<&str> = parts[1].split('x').collect();
    let x = coords[0].parse::<i32>()?;
    let y = coords[1].parse::<i32>()?;
    let width = dims[0].parse::<i32>()?;
    let height = dims[1].parse::<i32>()?;

    if !clipboard_only {
        create_dir_all(save_fullpath.parent().unwrap())
            .context("Failed to create screenshot directory")?;
    }

    let mut gnome_screenshot = Command::new("gnome-screenshot");
    gnome_screenshot
        .arg("--area")
        .arg(format!("{},{},{},{}", x, y, width, height));

    if clipboard_only {
        gnome_screenshot.arg("--clipboard");
    } else {
        gnome_screenshot.arg("--file").arg(save_fullpath);
    }

    let status = gnome_screenshot.status().context("Failed to run gnome-screenshot")?;
    if !status.success() {
        return Err(anyhow::anyhow!("gnome-screenshot failed to capture screenshot"));
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