# Hyprshot-rs

   A utility to easily take screenshots in Hyprland using your mouse.

   ## Features
   - Capture screenshots of windows, regions, or monitors.
   - Save screenshots to a specified folder or copy to the clipboard.
   - Support for delayed captures, screen freezing, and custom commands.

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

   ## Usage

   ```bash
   hyprshot-rs [options ..] [-m [mode] ..] -- [command]
   ```

   Examples:
   - Capture a window:
     ```bash
     hyprshot-rs -m window
     ```
   - Capture active window to clipboard:
     ```bash
     hyprshot-rs -m window -m active --clipboard-only
     ```
   - Capture a specific monitor:
     ```bash
     hyprshot-rs -m output -m DP-1
     ```
     
> где DP-1 активный дисплей

   Run `hyprshot-rs --help` for more options.

   ## License
   Licensed under either MIT or Apache-2.0 at your option.