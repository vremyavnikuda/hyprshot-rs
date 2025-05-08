// The `core` crate is implicitly linked, no need for explicit import

use anyhow::{Context, Result};
use chrono::Local;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use log::LevelFilter;

mod wayland;
mod grim;
mod environment;
mod desktop;
mod capture;
mod save;
mod utils;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Mode to use (output, region, window)
    #[arg(short, long)]
    mode: String,

    /// Save directory
    #[arg(short, long)]
    save_dir: Option<PathBuf>,

    /// Save filename
    #[arg(short, long)]
    filename: Option<String>,

    /// Only copy to clipboard
    #[arg(short, long)]
    clipboard_only: bool,

    /// Raw output
    #[arg(short, long)]
    raw: bool,

    /// Command to run after taking screenshot
    #[arg(short, long)]
    command: Option<Vec<String>>,

    /// Silent mode (no notifications)
    #[arg(short, long)]
    silent: bool,

    /// Notification timeout in milliseconds
    #[arg(short, long, default_value_t = 5000)]
    notif_timeout: u32,

    /// Debug mode
    #[arg(short, long)]
    debug: bool,
}

#[derive(Clone, Debug, ValueEnum)]
enum Mode {
    Output,
    Window,
    Region,
    Active,
    #[clap(skip)]
    OutputName(String),
}

fn main() -> Result<()> {
    // Initialize logger
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "debug");
    }
    simple_logger::init_with_level(log::Level::Debug)?;

    let args = Args::parse();

    let save_dir = args.save_dir.unwrap_or_else(|| {
        dirs::picture_dir()
            .unwrap_or_else(|| PathBuf::from("."))
    });

    let filename = args.filename.unwrap_or_else(|| {
        Local::now()
            .format("%Y-%m-%d-%H%M%S_hyprshot.png")
            .to_string()
    });

    let save_fullpath = save_dir.join(filename);

    if args.debug {
        println!("Saving in: {}", save_fullpath.display());
    }

    match args.mode.as_str() {
        "output" => {
            // TODO: Implement output mode
            unimplemented!("Output mode not implemented yet");
        }
        "region" => {
            // Get region geometry from slurp
            let output = std::process::Command::new("slurp")
                .output()
                .map_err(|e| anyhow::anyhow!("Failed to run slurp: {}", e))?;

            if !output.status.success() {
                return Ok(());
            }

            let geometry = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string();

            if args.debug {
                println!("Region geometry: {}", geometry);
            }

            save::save_geometry(
                &geometry,
                &save_fullpath,
                args.clipboard_only,
                args.raw,
                args.command,
                args.silent,
                args.notif_timeout,
                args.debug,
            )?;
        }
        "window" => {
            // TODO: Implement window mode
            unimplemented!("Window mode not implemented yet");
        }
        _ => {
            return Err(anyhow::anyhow!("Invalid mode: {}", args.mode));
        }
    }

    Ok(())
}

fn print_help() {
    println!(
        r#"
Usage: hyprshot-rs [options ..] [-m [mode] ..] -- [command]

Hyprshot-rs is an utility to easily take screenshot in Hyprland using your mouse.

It allows taking screenshots of windows, regions and monitors which are saved to a folder of your choosing and copied to your clipboard.

Examples:
  capture a window                      `hyprshot-rs -m window`
  capture active window to clipboard    `hyprshot-rs -m window -m active --clipboard-only`
  capture selected monitor              `hyprshot-rs -m output -m DP-1`

Options:
  -h, --help                show help message
  -v, --version             show version information
  -m, --mode                one of: output, window, region, active, OUTPUT_NAME
  -o, --output-folder       directory in which to save screenshot
  -f, --filename            the file name of the resulting screenshot
  -D, --delay               how long to delay taking the screenshot after selection (seconds)
  -z, --freeze              freeze the screen on initialization
  -d, --debug               print debug information
  -s, --silent              don't send notification when screenshot is saved
  -r, --raw                 output raw image data to stdout
  -t, --notif-timeout       notification timeout in milliseconds (default 5000)
  --clipboard-only          copy screenshot to clipboard and don't save image in disk
  -- [command]              open screenshot with a command of your choosing. e.g. hyprshot-rs -m window -- mirage

Modes:
  output        take screenshot of an entire monitor
  window        take screenshot of an open window
  region        take screenshot of selected region
  active        take screenshot of active window|output
                (you must use --mode again with the intended selection)
  OUTPUT_NAME   take screenshot of output with OUTPUT_NAME
                (you must use --mode again with the intended selection)
                (you can get this from `hyprctl monitors`)
"#
    );
}
