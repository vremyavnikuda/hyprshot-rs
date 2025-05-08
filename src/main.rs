// The `core` crate is implicitly linked, no need for explicit import

use anyhow::{Context, Result};
use chrono::Local;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use log::{info, debug};
use serde_json;
use tempfile;

mod args;
mod wayland;
mod grim;
mod environment;
mod desktop;
mod capture;
mod save;
mod utils;

use args::{Args, Mode};
use save::save_geometry;

fn generate_filename() -> PathBuf {
    let pictures_dir = dirs::picture_dir().unwrap_or_else(|| PathBuf::from("."));
    let timestamp = Local::now().format("%Y-%m-%d-%H%M%S").to_string();
    pictures_dir.join(format!("{}_hyprshot.png", timestamp))
}

fn select_region() -> Result<String> {
    let output = std::process::Command::new("slurp")
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run slurp: {}", e))?;

    if !output.status.success() {
        return Ok(String::new());
    }

    let geometry = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    if cfg!(debug_assertions) {
        println!("Region geometry: {}", geometry);
    }

    Ok(geometry)
}

fn select_window() -> Result<String> {
    // Получаем список окон через hyprctl
    let output = std::process::Command::new("hyprctl")
        .args(["clients", "-j"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run hyprctl: {}", e))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("hyprctl failed to get window list"));
    }

    // Парсим JSON с информацией об окнах
    let windows: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
        .map_err(|e| anyhow::anyhow!("Failed to parse window list: {}", e))?;

    // Создаем список окон для выбора
    let mut window_list = String::new();
    for (i, window) in windows.iter().enumerate() {
        let title = window["title"].as_str().unwrap_or("Untitled");
        let class = window["class"].as_str().unwrap_or("Unknown");
        let address = window["address"].as_str().unwrap_or("0x0");
        window_list.push_str(&format!("{}. {} ({}) - {}\n", i + 1, title, class, address));
    }

    // Сохраняем список во временный файл
    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| anyhow::anyhow!("Failed to create temp file: {}", e))?;
    std::fs::write(&temp_file, window_list)
        .map_err(|e| anyhow::anyhow!("Failed to write window list: {}", e))?;

    // Запускаем rofi для выбора окна
    let rofi_output = std::process::Command::new("rofi")
        .args([
            "-dmenu",
            "-i",
            "-p", "Select window",
            "-format", "i",
            "-theme", "default",
        ])
        .stdin(std::fs::File::open(&temp_file)?)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run rofi: {}", e))?;

    if !rofi_output.status.success() {
        return Ok(String::new());
    }

    // Получаем индекс выбранного окна
    let selected_index = String::from_utf8_lossy(&rofi_output.stdout)
        .trim()
        .parse::<usize>()
        .map_err(|e| anyhow::anyhow!("Failed to parse selected index: {}", e))?;

    if selected_index == 0 || selected_index > windows.len() {
        return Ok(String::new());
    }

    // Получаем геометрию выбранного окна
    let window = &windows[selected_index - 1];
    let x = window["at"][0].as_i64().unwrap_or(0) as i32;
    let y = window["at"][1].as_i64().unwrap_or(0) as i32;
    let width = window["size"][0].as_i64().unwrap_or(0) as u32;
    let height = window["size"][1].as_i64().unwrap_or(0) as u32;

    Ok(format!("{},{} {}x{}", x, y, width, height))
}

fn select_screen() -> Result<String> {
    // Получаем список мониторов через hyprctl
    let output = std::process::Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run hyprctl: {}", e))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("hyprctl failed to get monitor list"));
    }

    // Парсим JSON с информацией о мониторах
    let monitors: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
        .map_err(|e| anyhow::anyhow!("Failed to parse monitor list: {}", e))?;

    // Создаем список мониторов для выбора
    let mut monitor_list = String::new();
    for (i, monitor) in monitors.iter().enumerate() {
        let name = monitor["name"].as_str().unwrap_or("Unknown");
        let width = monitor["width"].as_i64().unwrap_or(0);
        let height = monitor["height"].as_i64().unwrap_or(0);
        monitor_list.push_str(&format!("{}. {} ({}x{})\n", i + 1, name, width, height));
    }

    // Сохраняем список во временный файл
    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| anyhow::anyhow!("Failed to create temp file: {}", e))?;
    std::fs::write(&temp_file, monitor_list)
        .map_err(|e| anyhow::anyhow!("Failed to write monitor list: {}", e))?;

    // Запускаем rofi для выбора монитора
    let rofi_output = std::process::Command::new("rofi")
        .args([
            "-dmenu",
            "-i",
            "-p", "Select monitor",
            "-format", "i",
            "-theme", "default",
        ])
        .stdin(std::fs::File::open(&temp_file)?)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run rofi: {}", e))?;

    if !rofi_output.status.success() {
        return Ok(String::new());
    }

    // Получаем индекс выбранного монитора
    let selected_index = String::from_utf8_lossy(&rofi_output.stdout)
        .trim()
        .parse::<usize>()
        .map_err(|e| anyhow::anyhow!("Failed to parse selected index: {}", e))?;

    if selected_index == 0 || selected_index > monitors.len() {
        return Ok(String::new());
    }

    // Получаем геометрию выбранного монитора
    let monitor = &monitors[selected_index - 1];
    let x = monitor["x"].as_i64().unwrap_or(0) as i32;
    let y = monitor["y"].as_i64().unwrap_or(0) as i32;
    let width = monitor["width"].as_i64().unwrap_or(0) as u32;
    let height = monitor["height"].as_i64().unwrap_or(0) as u32;

    Ok(format!("{},{} {}x{}", x, y, width, height))
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    if args.debug {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
            .init();
    } else {
        env_logger::init();
    }

    info!("Starting hyprshot-rs");
    debug!("Arguments: {:?}", args);

    // Генерируем путь для сохранения файла, если он не указан
    let save_path = if args.clipboard_only {
        PathBuf::new()
    } else {
        args.output_path.unwrap_or_else(generate_filename)
    };

    if args.debug {
        info!("Saving to: {}", save_path.display());
    }

    match args.mode {
        Mode::Region => {
            let geometry = select_region()?;
            save_geometry(
                &geometry,
                &save_path,
                args.clipboard_only,
                args.raw,
                args.command,
                args.silent,
                args.notif_timeout,
                args.debug,
            )?;
        }
        Mode::Window => {
            let geometry = select_window()?;
            save_geometry(
                &geometry,
                &save_path,
                args.clipboard_only,
                args.raw,
                args.command,
                args.silent,
                args.notif_timeout,
                args.debug,
            )?;
        }
        Mode::Screen => {
            let geometry = select_screen()?;
            save_geometry(
                &geometry,
                &save_path,
                args.clipboard_only,
                args.raw,
                args.command,
                args.silent,
                args.notif_timeout,
                args.debug,
            )?;
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
