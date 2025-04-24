//examples/cli.rs
use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use chrono::Local;

mod capture;
mod save;
mod utils;

#[derive(Parser)]
#[command(name = "hyprshot-rs", about = "Utility to easily take screenshots in Hyprland")]
struct Args {
    #[arg(short, long, help = "Show help message")]
    help: bool,

    #[arg(short = 'm', long, help = "Mode: output, window, region, active, or OUTPUT_NAME")]
    mode: Vec<Mode>,

    #[arg(short, long, help = "Directory to save screenshot")]
    output_folder: Option<PathBuf>,

    #[arg(short, long, help = "Filename of the screenshot")]
    filename: Option<String>,

    #[arg(short = 'D', long, help = "Delay before taking screenshot (seconds)")]
    delay: Option<u64>,

    #[arg(long, help = "Freeze the screen on initialization")]
    freeze: bool,

    #[arg(short, long, help = "Print debug information")]
    debug: bool,

    #[arg(short, long, help = "Don't send notification")]
    silent: bool,

    #[arg(short, long, help = "Output raw image data to stdout")]
    raw: bool,

    #[arg(short, long, default_value = "5000", help = "Notification timeout (ms)")]
    notif_timeout: u32,

    #[arg(long, help = "Copy to clipboard and don't save to disk")]
    clipboard_only: bool,

    #[arg(last = true, help = "Command to open screenshot (e.g., 'mirage')")]
    command: Vec<String>,
}

impl std::fmt::Debug for Args {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Args")
            .field("help", &self.help)
            .field("mode", &self.mode)
            .field("output_folder", &self.output_folder)
            .field("filename", &self.filename)
            .field("delay", &self.delay)
            .field("freeze", &self.freeze)
            .field("debug", &self.debug)
            .field("silent", &self.silent)
            .field("raw", &self.raw)
            .field("notif_timeout", &self.notif_timeout)
            .field("clipboard_only", &self.clipboard_only)
            .field("command", &self.command)
            .finish()
    }
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
    let args = Args::parse();

    if args.help || args.mode.is_empty() {
        print_help();
        return Ok(());
    }

    let debug = args.debug;
    let clipboard_only = args.clipboard_only;
    let silent = args.silent;
    let raw = args.raw;
    let notif_timeout = args.notif_timeout;
    let freeze = args.freeze;
    let delay = args.delay.unwrap_or(0);
    let command = if args.command.is_empty() { None } else { Some(args.command) };

    let mut option: Option<Mode> = None;
    let mut current = false;
    let mut selected_monitor: Option<String> = None;

    for mode in args.mode {
        match mode {
            Mode::Output | Mode::Window | Mode::Region => option = Some(mode),
            Mode::Active => current = true,
            Mode::OutputName(name) => {
                if utils::is_valid_monitor(&name)? {
                    selected_monitor = Some(name);
                }
            }
        }
    }

    let option = option.context("A mode is required (output, region, window)")?;

    let save_dir = args.output_folder.unwrap_or_else(|| {
        dirs::picture_dir().unwrap_or_else(|| PathBuf::from("~"))
    });
    let filename = args.filename.unwrap_or_else(|| {
        Local::now().format("%Y-%m-%d-%H%M%S_hyprshot.png").to_string()
    });
    let save_fullpath = save_dir.join(&filename);

    if debug && !clipboard_only {
        eprintln!("Saving in: {}", save_fullpath.display());
    }

    let hyprpicker_pid = if freeze && Command::new("hyprpicker").output().is_ok() {
        let pid = Command::new("hyprpicker")
            .args(["-r", "-z"])
            .spawn()
            .context("Failed to start hyprpicker")?
            .id();
        sleep(Duration::from_millis(200));
        Some(pid)
    } else {
        None
    };

    if delay > 0 {
        sleep(Duration::from_secs(delay));
    }

    let geometry = match option {
        Mode::Output => {
            if current {
                capture::grab_active_output(debug)?
            } else if let Some(monitor) = selected_monitor {
                capture::grab_selected_output(&monitor, debug)?
            } else {
                capture::grab_output(debug)?
            }
        }
        Mode::Region => capture::grab_region(debug)?,
        Mode::Window => {
            let geo = if current {
                capture::grab_active_window(debug)?
            } else {
                capture::grab_window(debug)?
            };
            utils::trim(&geo, debug)?
        }
        _ => unreachable!(),
    };

    save::save_geometry(
        &geometry,
        &save_fullpath,
        clipboard_only,
        raw,
        command,
        silent,
        notif_timeout,
        debug,
    )?;

    if let Some(pid) = hyprpicker_pid {
        Command::new("kill")
            .arg(pid.to_string())
            .status()
            .context("Failed to kill hyprpicker")?;
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