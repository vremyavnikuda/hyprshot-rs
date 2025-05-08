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