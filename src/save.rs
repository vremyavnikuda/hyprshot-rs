use anyhow::{Context, Result};
use log::info;
use notify_rust::Notification;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::fs::create_dir_all;
use wayland_client::{
    protocol::{wl_registry, wl_shm, wl_output, wl_buffer, wl_shm_pool},
    Connection, Dispatch, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1,
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};
use std::os::unix::io::AsRawFd;
use memmap2::MmapMut;
use std::io::Write;

use crate::wayland::WaylandScreenshot;
use crate::environment::Environment;
use crate::desktop::{save_geometry_with_kde, save_geometry_with_gnome};

// #[cfg(feature = "grim")]
// use crate::grim;

pub fn save_geometry(
    geometry: &str,
    save_fullpath: &PathBuf,
    clipboard_only: bool,
    raw: bool,
    command: Option<Vec<String>>,
    silent: bool,
    notif_timeout: u32,
    debug: bool,
) -> Result<()> {
    let env = Environment::new(debug)?;
    let desktop = env.detect_desktop_environment()?;

    match desktop.as_str() {
        "kde" => save_geometry_with_kde(
            geometry,
            save_fullpath,
            clipboard_only,
            raw,
            command,
            silent,
            notif_timeout,
            debug,
        ),
        "gnome" => save_geometry_with_gnome(
            geometry,
            save_fullpath,
            clipboard_only,
            raw,
            command,
            silent,
            notif_timeout,
            debug,
        ),
        _ => save_geometry_with_native(
            geometry,
            save_fullpath,
            clipboard_only,
            raw,
            command,
            silent,
            notif_timeout,
            debug,
        ),
    }
}

pub fn save_geometry_with_native(
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
        info!("Saving geometry with native Wayland implementation: {}", geometry);
    }

    // Parse geometry
    let parts: Vec<&str> = geometry.split(' ').collect();
    let coords: Vec<&str> = parts[0].split(',').collect();
    let dims: Vec<&str> = parts[1].split('x').collect();
    let x = coords[0].parse::<i32>()?;
    let y = coords[1].parse::<i32>()?;
    let width = dims[0].parse::<u32>()?;
    let height = dims[1].parse::<u32>()?;

    if !clipboard_only {
        create_dir_all(save_fullpath.parent().unwrap())
            .context("Failed to create screenshot directory")?;
    }

    // Capture screenshot using Wayland
    let mut screenshot = WaylandScreenshot::new(debug)?;
    let data = screenshot.capture_region(x, y, width, height)?;

    // Save to file if needed
    if !clipboard_only {
        std::fs::write(save_fullpath, &data)
            .context("Failed to write screenshot to file")?;
    }

    // Copy to clipboard
    let mut clipboard = Command::new("wl-copy");
    clipboard.arg("--type").arg("image/png");
    let mut child = clipboard
        .stdin(Stdio::piped())
        .spawn()
        .context("Failed to start wl-copy")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&data)?;
    }

    child.wait().context("Failed to wait for wl-copy")?;

    // Show notification
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

fn save_geometry_with_portal(
    save_fullpath: &PathBuf,
    clipboard_only: bool,
    silent: bool,
    notif_timeout: u32,
    debug: bool,
) -> Result<()> {
    use notify_rust::Notification;
    use std::{
        fs::{self, File},
        path::PathBuf,
        process::{Command, Stdio},
        time::{SystemTime, UNIX_EPOCH},
    };

    let start = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    if debug {
        eprintln!("Capturing screenshot via xdg-desktop-portal...");
    }

    let status = Command::new("dbus-send")
        .args([
            "--session",
            "--dest=org.freedesktop.portal.Desktop",
            "--object-path=/org/freedesktop/portal/desktop",
            "--print-reply",
            "org.freedesktop.portal.Screenshot.Screenshot",
            "string:\"\"",
            "a{sv}:{}",
        ])
        .status()
        .context("Failed to run dbus-send for screenshot")?;

    if !status.success() {
        return Err(anyhow::anyhow!("xdg-desktop-portal screenshot failed"));
    }

    // Search for new file in ~/Pictures
    let pictures_dir = dirs::picture_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut found = None;

    for entry in fs::read_dir(&pictures_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|ext| ext == "png").unwrap_or(false) {
            let metadata = fs::metadata(&path)?;
            let created = metadata.created().unwrap_or(SystemTime::UNIX_EPOCH);
            if created.duration_since(UNIX_EPOCH)?.as_secs() >= start {
                found = Some(path);
                break;
            }
        }
    }

    let found_path = found.ok_or_else(|| anyhow::anyhow!("No screenshot found after portal capture"))?;

    if !clipboard_only {
        fs::create_dir_all(save_fullpath.parent().unwrap())?;
        fs::copy(&found_path, save_fullpath)?;
    }

    if clipboard_only {
        let mut wl_copy = Command::new("wl-copy")
            .arg("--type")
            .arg("image/png")
            .stdin(Stdio::piped())
            .spawn()
            .context("Failed to start wl-copy")?;

        let mut input = File::open(&found_path)?;
        let mut stdin = wl_copy.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to open wl-copy stdin"))?;
        std::io::copy(&mut input, &mut stdin)?;
        let status = wl_copy.wait()?;
        if !status.success() {
            return Err(anyhow::anyhow!("wl-copy failed"));
        }
    }

    if !silent {
        let message = if clipboard_only {
            "Image copied to the clipboard".to_string()
        } else {
            format!("Image saved in <i>{}</i>", save_fullpath.display())
        };

        Notification::new()
            .summary("Screenshot saved")
            .body(&message)
            .icon("screenshot")
            .timeout(notif_timeout as i32)
            .appname("Hyprshot-rs")
            .show()
            .ok(); // Ignore failure
    }

    Ok(())
}
