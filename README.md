# Pillify

Native Windows 10/11 top-bezel interactive notch and dynamic capsule written in Rust with Direct2D composition.

## Architecture

- **Renderer (`src/renderer.rs`)**: Hardware-accelerated Direct2D and DirectWrite rendering into a 32-bit premultiplied-alpha DIB section presented via `UpdateLayeredWindow`. Includes custom cubic-Bezier top-bezel MacBook notch ear geometry (`ID2D1PathGeometry`), axis-aligned viewport clipping (`PushAxisAlignedClip`), and character-ellipsis text trimming.
- **Physics (`src/spring.rs`)**: Semi-implicit Euler damped harmonic oscillator (`Snappy`, `Bouncy`, and `Smooth` profiles) driving capsule dimensions, corner radii, tab transitions, and continuous vertical lyric scrolling. The render timer runs at 120 Hz during active motion and halts (`WaitMessage`) when springs reach equilibrium.
- **Hit-Testing (`src/main.rs`)**: Per-frame bounding evaluation on `WM_NCHITTEST`. Coordinates outside the active capsule geometry return `HTTRANSPARENT`, passing mouse events directly to underlying windows.
- **Telemetry & Integrations (`src/services.rs`)**:
  - **Media & Synced Lyrics**: Non-blocking WinRT System Media Transport Controls (`GlobalSystemMediaTransportControlsSessionManager`) with real-time playback timeline tracking (`GetTimelineProperties`) and automatic LRC timestamped lyrics retrieval via the LRCLIB HTTPS API (`Windows::Web::Http::HttpClient`).
  - **Shell Icon Extraction (`src/state.rs`)**: Native 32x32 32-bit premultiplied BGRA icon extraction for `.lnk` shortcuts, executables, documents, and folders via `SHGetFileInfoW` and `DrawIconEx`.
  - **System Sensors**: CoreAudio `IAudioEndpointVolume` listener, `GetSystemPowerStatus` battery readout, `NetworkInformation` SSID detection, and `SHQueryUserNotificationState` fullscreen/Direct3D detection.

## Views

1. **Now Playing**: SMTC media transport controls (`Previous`, `Play`/`Pause`, `Next`), animated 5-bar audio visualizer, Spotify-style vertically scrolling synced lyrics viewport, live progress bar with elapsed/remaining timestamps, system status bar, and `Focus Mode: ON / OFF` toggle.
2. **Tray**: Drag-and-drop application shortcut, file, and folder staging via Win32 `WM_DROPFILES`. Renders native shell icons and display names for single-click execution (`ShellExecuteW`).
3. **Timer**: Configurable Pomodoro focus timer (`25m`, `5m`, `15m`), water intake counter, and quick checklist.
4. **Notch**: Live geometry and motion calibration (capsule behavior mode, spring physics preset, accent palette, expanded width, corner radius, top-bezel ear flare, and border opacity) persisted to `%LOCALAPPDATA%\Pillify\config.json`.

## Controls

| Input | Action |
|---|---|
| Left-click compact notch | Expand interactive notch |
| Top navigation pills / Mouse wheel | Switch between `Now Playing`, `Tray`, `Timer`, and `Notch` |
| Right-click (while expanded) | Collapse back to compact notch |
| Drag files or `.lnk` shortcuts onto notch | Expand and pin items into `Tray` with native icons |
| `Ctrl + Alt + I` | Toggle notch visibility |

## Build & Test

```powershell
cargo test
cargo build --release
```
