# Pillify

Native Windows 10/11 top-bezel interactive notch and dynamic capsule written in Rust with Direct2D composition.

## Architecture

- **Renderer (`src/renderer.rs`)**: Hardware-accelerated Direct2D and DirectWrite rendering into a 32-bit premultiplied-alpha DIB section presented via `UpdateLayeredWindow`.
- **Physics (`src/spring.rs`)**: Semi-implicit Euler damped harmonic oscillator (`Snappy`, `Bouncy`, and `Smooth` profiles). The render timer runs at 120 Hz during active motion and halts completely (`WaitMessage`) when springs reach equilibrium.
- **Hit-Testing (`src/main.rs`)**: Per-frame bounding evaluation on `WM_NCHITTEST`. Coordinates outside the active capsule geometry return `HTTRANSPARENT`, passing mouse events directly to underlying windows.
- **Telemetry (`src/services.rs`)**: Event-driven and non-blocking WinRT System Media Transport Controls (SMTC), CoreAudio `IAudioEndpointVolume`, `GetSystemPowerStatus`, `NetworkInformation`, and `SHQueryUserNotificationState` fullscreen suppression.

## Modules

1. **MEDIA**: SMTC transport controls (`PREV`, `PLAY`/`PAUSE`, `NEXT`), real-time 6-band spectrum indicator, network/power telemetry, and focus mode gatekeepers (`STD`, `DND`, `MUTE`).
2. **SHELF**: Local file and directory staging via Win32 `WM_DROPFILES`. Drag items from File Explorer onto the capsule to stage them for single-click execution.
3. **FOCUS**: Interval timer (`25M`, `05M`, `15M`), hydration counter, and persistent task checklist.
4. **CONFIG**: Live geometry and physics calibration (capsule mode, spring damping preset, monochrome/signal accent, width, corner radius, top-bezel ear flare, and border hairline opacity) saved to `%LOCALAPPDATA%\Pillify\config.json`.

## Controls

| Input | Action |
|---|---|
| Left-click capsule | Expand notch dashboard |
| Mouse wheel (while expanded) | Cycle between `MEDIA`, `SHELF`, `FOCUS`, and `CONFIG` |
| Right-click (while expanded) | Collapse back to compact capsule |
| Drag files onto capsule | Expand and stage paths in `SHELF` |
| `Ctrl + Alt + I` | Toggle window visibility |

## Build

```powershell
cargo test
cargo build --release
```
