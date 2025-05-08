# cli hyprshot-rs

### Additional options
- Pre-capture delay (`--delay`): Implemented via sleep.
- Screen Freeze (`--freeze`): Uses hyprpicker to freeze.
- Notifications: Via notify_rust, with a configurable timeout (`--notif-timeout`) and a mute option (`--silent`).
- Running a command after capture (for example, opening an image): It is supported via `-- [command]`.
- Clipboard-only mode (`--clipboard-only`): Saving to disk is disabled.
- Debugging mode (`--debug`): Outputs detailed logs.
- Сhecking the version `hyprshot-rs -v` and `hyprshot-rs --version` is supported.