[![Crates.io Version](https://img.shields.io/crates/v/hyprshot-rs.svg)](https://crates.io/crates/hyprshot-rs) ![Crates.io Recent Downloads](https://img.shields.io/crates/dr/hyprshot-rs?style=flat&color=green) [![Crates.io License](https://img.shields.io/crates/l/hyprshot-rs.svg)](https://crates.io/crates/hyprshot-rs) [![Rust](https://github.com/vremyavnikuda/hyprshot-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/vremyavnikuda/hyprshot-rs/actions/workflows/rust.yml) [![Dependencies](https://deps.rs/repo/github/vremyavnikuda/hyprshot-rs/status.svg)](https://deps.rs/repo/github/vremyavnikuda/hyprshot-rs)
___
# Hyprshot-rs

A utility to easily take screenshots in Hyprland using your mouse.

## Features
- Capture screenshots.
    - Capture the entire monitor (output)
    - Capture the active monitor (active)
    - Capturing the selected (region)
    - Capturing the selected (window)
    - Capture of the (active window)
- Save screenshots to a specified folder or copy to the clipboard.
     - Screenshots are saved only in PNG. The choice of other formats (JPEG, WEBP) or compression settings is not supported.
- Additional options and custom commands [cli](doc/CLI.md).

## Installation

Install via Cargo:
```bash
cargo install hyprshot-rs
```

Ensure the following dependencies are installed:
- `grim`
- `slurp`
- `wl-clipboard`
- `hyprland`
- `hyprpicker` (optional)

On Arch Linux:
```bash
sudo pacman -S grim slurp wl-clipboard hyprland hyprpicker
```
___
## Usage
Make it available regardless of the shell
```bash
sudo ln -s ~/.local/share/cargo/bin/hyprshot-rs /usr/local/bin/
```

```bash
hyprshot-rs [options ..] [-m [mode] ..] -- [command]
```
```
possible values: output, window, region, active
```

Possible values:
- Capture a window:
```bash
hyprshot-rs -m window
```
- To take a screenshot of a specific area of the screen, use:
```bash
hyprshot-rs -m region
```
- If you have 2 or more monitors and want to take a screenshot of the workspace on a specific monitor: 
```bash
hyprshot-rs -m output
```
- Quick capture (instant screenshot of the workspace where the cursor is):
```bash
hyprshot-rs -m active -m output
```
- Take a screenshot of a selected area and save it in the current directory:
~/repository
```bash
hyprshot-rs -m region -r > output.png
```
redirects the output to output.png in your current working directory. So if you're currently in ~/repository when running this command, that's where the screenshot will be saved, not in the default ~/Pictures directory.


Run `hyprshot-rs --help` or `hyprshot-rs -h`for more options.

Binding to specific key combinations
Add to the hyprland.conf configuration file:
```cfg
bind = , PRINT , exec , hyprshot-rs -m active -m output
bind = $mainMod, PRINT , exec , hyprshot-rs -m region
bind = $shiftMod , PRINT , exec ,  hyprshot-rs -m output
```
Based on the implementation: [Hypershot](https://github.com/Gustash/Hyprshot)
## License
[GPL-3.0](LICENSE.md)