use anyhow::{Context, Result};
use log::info;
use std::process::Command;

pub struct Environment {
    pub desktop: String,
    pub debug: bool,
}

impl Environment {
    pub fn new(debug: bool) -> Result<Self> {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_else(|_| std::env::var("DESKTOP_SESSION").unwrap_or_else(|_| "unknown".to_string()));

        Ok(Self { desktop, debug })
    }

    pub fn detect_desktop_environment(&self) -> Result<String> {
        if self.debug {
            info!("Detected desktop environment: {}", self.desktop);
        }
        Ok(self.desktop.to_lowercase())
    }
}

pub fn detect_desktop_environment() -> Result<String> {
    let output = Command::new("echo")
        .arg("$XDG_CURRENT_DESKTOP")
        .output()
        .context("Failed to detect desktop environment")?;

    let desktop = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
    
    if desktop.contains("hyprland") {
        Ok("Hyprland".to_string())
    } else if desktop.contains("kde") {
        Ok("KDE".to_string())
    } else if desktop.contains("gnome") {
        Ok("GNOME".to_string())
    } else {
        Ok("Unknown".to_string())
    }
} 