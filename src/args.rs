use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Mode to use (output, region, window)
    #[arg(short, long)]
    pub mode: Mode,

    /// Output path (optional, defaults to Pictures directory with timestamp)
    #[arg(short, long)]
    pub output_path: Option<PathBuf>,

    /// Only copy to clipboard
    #[arg(short, long)]
    pub clipboard_only: bool,

    /// Raw output
    #[arg(short, long)]
    pub raw: bool,

    /// Command to run after taking screenshot
    #[arg(short, long)]
    pub command: Option<Vec<String>>,

    /// Silent mode (no notifications)
    #[arg(short, long)]
    pub silent: bool,

    /// Notification timeout in milliseconds
    #[arg(short, long, default_value_t = 5000)]
    pub notif_timeout: u32,

    /// Debug mode
    #[arg(short, long)]
    pub debug: bool,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum Mode {
    Region,
    Window,
    Screen,
} 