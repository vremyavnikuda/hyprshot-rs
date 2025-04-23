use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use notify_rust::Notification;
use serde_json::Value;
use std::fs::create_dir_all;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use chrono::Local;

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
                if is_valid_monitor(&name)? {
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
                grab_active_output(debug)?
            } else if let Some(monitor) = selected_monitor {
                grab_selected_output(&monitor, debug)?
            } else {
                grab_output(debug)?
            }
        }
        Mode::Region => grab_region(debug)?,
        Mode::Window => {
            let geo = if current {
                grab_active_window(debug)?
            } else {
                grab_window(debug)?
            };
            trim(&geo, debug)?
        }
        _ => unreachable!(),
    };

    save_geometry(
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

fn is_valid_monitor(name: &str) -> Result<bool> {
    let output = Command::new("hyprctl")
        .arg("monitors")
        .arg("-j")
        .output()
        .context("Failed to run hyprctl monitors")?;
    let monitors: Value = serde_json::from_slice(&output.stdout)?;
    Ok(monitors
        .as_array()
        .map(|arr| arr.iter().any(|m| m["name"].as_str() == Some(name)))
        .unwrap_or(false))
}

fn trim(geometry: &str, debug: bool) -> Result<String> {
    if debug {
        eprintln!("Input geometry: {}", geometry);
    }

    let parts: Vec<&str> = geometry.split(' ').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid geometry format: expected 'x,y wxh', got '{}'",
            geometry
        ));
    }

    let xy: Vec<&str> = parts[0].split(',').collect();
    let wh: Vec<&str> = parts[1].split('x').collect();
    if xy.len() != 2 || wh.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid geometry format: expected 'x,y wxh', got '{}'",
            geometry
        ));
    }

    let x: i32 = xy[0]
        .parse()
        .context(format!("Failed to parse x coordinate from '{}'", xy[0]))?;
    let y: i32 = xy[1]
        .parse()
        .context(format!("Failed to parse y coordinate from '{}'", xy[1]))?;
    let width: i32 = wh[0]
        .parse()
        .context(format!("Failed to parse width from '{}'", wh[0]))?;
    let height: i32 = wh[1]
        .parse()
        .context(format!("Failed to parse height from '{}'", wh[1]))?;

    if width <= 0 || height <= 0 {
        return Err(anyhow::anyhow!(
            "Invalid geometry dimensions: width={} or height={} is non-positive",
            width,
            height
        ));
    }

    let monitors_output = Command::new("hyprctl")
        .arg("monitors")
        .arg("-j")
        .output()
        .context("Failed to run hyprctl monitors")?;
    let monitors: Value = serde_json::from_slice(&monitors_output.stdout)?;

    let max_width = monitors
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|m| {
                    let transform = m["transform"].as_i64().unwrap_or(0);
                    let x = m["x"].as_i64().unwrap_or(0) as i32;
                    let width = m["width"].as_i64().unwrap_or(0) as i32;
                    let height = m["height"].as_i64().unwrap_or(0) as i32;
                    if transform % 2 == 0 {
                        x + width
                    } else {
                        x + height
                    }
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let max_height = monitors
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|m| {
                    let transform = m["transform"].as_i64().unwrap_or(0);
                    let y = m["y"].as_i64().unwrap_or(0) as i32;
                    let width = m["width"].as_i64().unwrap_or(0) as i32;
                    let height = m["height"].as_i64().unwrap_or(0) as i32;
                    if transform % 2 == 0 {
                        y + height
                    } else {
                        y + width
                    }
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let min_x = monitors
        .as_array()
        .map(|arr| arr.iter().map(|m| m["x"].as_i64().unwrap_or(0) as i32).min().unwrap_or(0))
        .unwrap_or(0);

    let min_y = monitors
        .as_array()
        .map(|arr| arr.iter().map(|m| m["y"].as_i64().unwrap_or(0) as i32).min().unwrap_or(0))
        .unwrap_or(0);

    let mut cropped_x = x;
    let mut cropped_y = y;
    let mut cropped_width = width;
    let mut cropped_height = height;

    if x + width > max_width {
        cropped_width = max_width - x;
    }
    if y + height > max_height {
        cropped_height = max_height - y;
    }
    if x < min_x {
        cropped_x = min_x;
        cropped_width += x - min_x;
    }
    if y < min_y {
        cropped_y = min_y;
        cropped_height += y - min_y;
    }

    if cropped_width <= 0 || cropped_height <= 0 {
        return Err(anyhow::anyhow!(
            "Invalid cropped dimensions: width={} or height={}",
            cropped_width,
            cropped_height
        ));
    }

    let cropped = format!("{},{}\t{}x{}", cropped_x, cropped_y, cropped_width, cropped_height);
    if debug {
        eprintln!("Cropped geometry: {}", cropped);
    }
    Ok(cropped)
}

fn save_geometry(
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
        eprintln!("Saving geometry: {}", geometry);
    }

    if raw {
        let output = Command::new("grim")
            .arg("-g")
            .arg(geometry)
            .arg("-")
            .output()
            .context("Failed to run grim")?;
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
        let wl_copy_status = wl_copy
            .wait()
            .context("Failed to wait for wl-copy")?;
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

fn grab_output(debug: bool) -> Result<String> {
    let output = Command::new("slurp")
        .arg("-or")
        .output()
        .context("Failed to run slurp")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!("slurp failed to select output"));
    }
    let geometry = String::from_utf8(output.stdout)
        .context("slurp output is not valid UTF-8")?
        .trim()
        .to_string();
    if debug {
        eprintln!("Output geometry: {}", geometry);
    }
    if geometry.is_empty() {
        return Err(anyhow::anyhow!("slurp returned empty geometry"));
    }
    Ok(geometry)
}

fn grab_active_output(debug: bool) -> Result<String> {
    let active_workspace: Value = serde_json::from_slice(
        &Command::new("hyprctl")
            .arg("activeworkspace")
            .arg("-j")
            .output()
            .context("Failed to run hyprctl activeworkspace")?
            .stdout,
    )?;
    let monitors: Value = serde_json::from_slice(
        &Command::new("hyprctl")
            .arg("monitors")
            .arg("-j")
            .output()
            .context("Failed to run hyprctl monitors")?
            .stdout,
    )?;

    if debug {
        eprintln!("Monitors: {}", monitors);
        eprintln!("Active workspace: {}", active_workspace);
    }

    let current_monitor = monitors
        .as_array()
        .and_then(|arr| arr.iter().find(|m| m["activeWorkspace"]["id"] == active_workspace["id"]))
        .context("No matching monitor found")?;

    if debug {
        eprintln!("Current output: {}", current_monitor);
    }

    let x = current_monitor["x"].as_i64().unwrap_or(0);
    let y = current_monitor["y"].as_i64().unwrap_or(0);
    let width = current_monitor["width"].as_i64().unwrap_or(0) as f64;
    let height = current_monitor["height"].as_i64().unwrap_or(0) as f64;
    let scale = current_monitor["scale"].as_f64().unwrap_or(1.0);

    let geometry = format!(
        "{},{} {}x{}",
        x,
        y,
        (width / scale).round() as i32,
        (height / scale).round() as i32
    );
    if debug {
        eprintln!("Active output geometry: {}", geometry);
    }
    Ok(geometry)
}

fn grab_selected_output(monitor: &str, debug: bool) -> Result<String> {
    let monitors: Value = serde_json::from_slice(
        &Command::new("hyprctl")
            .arg("monitors")
            .arg("-j")
            .output()
            .context("Failed to run hyprctl monitors")?
            .stdout,
    )?;

    let monitor_data = monitors
        .as_array()
        .and_then(|arr| arr.iter().find(|m| m["name"].as_str() == Some(monitor)))
        .context(format!("Monitor '{}' not found", monitor))?;

    if debug {
        eprintln!("Capturing monitor: {}", monitor);
    }

    let x = monitor_data["x"].as_i64().unwrap_or(0);
    let y = monitor_data["y"].as_i64().unwrap_or(0);
    let width = monitor_data["width"].as_i64().unwrap_or(0) as f64;
    let height = monitor_data["height"].as_i64().unwrap_or(0) as f64;
    let scale = monitor_data["scale"].as_f64().unwrap_or(1.0);

    let geometry = format!(
        "{},{} {}x{}",
        x,
        y,
        (width / scale).round() as i32,
        (height / scale).round() as i32
    );
    if debug {
        eprintln!("Selected output geometry: {}", geometry);
    }
    Ok(geometry)
}

fn grab_region(debug: bool) -> Result<String> {
    let output = Command::new("slurp")
        .arg("-d")
        .output()
        .context("Failed to run slurp")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!("slurp failed to select region"));
    }
    let geometry = String::from_utf8(output.stdout)
        .context("slurp output is not valid UTF-8")?
        .trim()
        .to_string();
    if debug {
        eprintln!("Region geometry: {}", geometry);
    }
    if geometry.is_empty() {
        return Err(anyhow::anyhow!("slurp returned empty geometry"));
    }
    Ok(geometry)
}

fn grab_window(debug: bool) -> Result<String> {
    let monitors: Value = serde_json::from_slice(
        &Command::new("hyprctl")
            .arg("monitors")
            .arg("-j")
            .output()
            .context("Failed to run hyprctl monitors")?
            .stdout,
    )?;
    let clients: Value = serde_json::from_slice(
        &Command::new("hyprctl")
            .arg("clients")
            .arg("-j")
            .output()
            .context("Failed to run hyprctl clients")?
            .stdout,
    )?;

    let workspace_ids: String = monitors
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["activeWorkspace"]["id"].as_i64())
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();

    let filtered_clients: Vec<Value> = clients
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|c| {
                    c["workspace"]["id"]
                        .as_i64()
                        .map(|id| workspace_ids.contains(&id.to_string()))
                        .unwrap_or(false)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    if debug {
        eprintln!("Monitors: {}", monitors);
        eprintln!("Clients: {}", serde_json::to_string(&filtered_clients)?);
    }

    let boxes: String = filtered_clients
        .into_iter()
        .filter_map(|c| {
            let at = c["at"].as_array()?;
            let size = c["size"].as_array()?;
            Some(format!(
                "{},{} {}x{} {}",
                at[0].as_i64()?,
                at[1].as_i64()?,
                size[0].as_i64()?,
                size[1].as_i64()?,
                c["title"].as_str().unwrap_or("")
            ))
        })
        .collect::<Vec<_>>()
        .join("\n");

    if debug {
        eprintln!("Window boxes:\n{}", boxes);
    }

    if boxes.is_empty() {
        return Err(anyhow::anyhow!("No windows found to capture"));
    }

    let mut slurp = Command::new("slurp")
        .arg("-r")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to start slurp")?;

    slurp
        .stdin
        .as_mut()
        .unwrap()
        .write_all(boxes.as_bytes())
        .context("Failed to write to slurp stdin")?;

    let output = slurp.wait_with_output().context("Failed to run slurp")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!("slurp failed to select window"));
    }

    let geometry = String::from_utf8(output.stdout)
        .context("slurp output is not valid UTF-8")?
        .trim()
        .to_string();
    if debug {
        eprintln!("Window geometry: {}", geometry);
    }
    if geometry.is_empty() {
        return Err(anyhow::anyhow!("slurp returned empty geometry"));
    }
    Ok(geometry)
}

fn grab_active_window(debug: bool) -> Result<String> {
    let active_window: Value = serde_json::from_slice(
        &Command::new("hyprctl")
            .arg("activewindow")
            .arg("-j")
            .output()
            .context("Failed to run hyprctl activewindow")?
            .stdout,
    )?;

    if debug {
        eprintln!("Active window: {}", active_window);
    }

    let at = active_window["at"]
        .as_array()
        .context("Invalid active window data: missing 'at' field")?;
    let size = active_window["size"]
        .as_array()
        .context("Invalid active window data: missing 'size' field")?;

    let x = at[0].as_i64().context("Invalid x coordinate")?;
    let y = at[1].as_i64().context("Invalid y coordinate")?;
    let width = size[0].as_i64().context("Invalid width")?;
    let height = size[1].as_i64().context("Invalid height")?;

    if width <= 0 || height <= 0 {
        return Err(anyhow::anyhow!(
            "Invalid window dimensions: width={} or height={}",
            width,
            height
        ));
    }

    let geometry = format!("{},{} {}x{}", x, y, width, height);
    if debug {
        eprintln!("Active window geometry: {}", geometry);
    }
    Ok(geometry)
}