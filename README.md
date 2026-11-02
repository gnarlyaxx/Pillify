# Pillify ✨

**Pillify** is an ultra-lightweight, NUSA-inspired interactive **Dynamic Island & Custom Notch** for Windows 10 and Windows 11, written natively in **Rust** and rendered via **Direct2D GPU Composition**.

---

## ✨ Key Features

- **0% Idle CPU & Native GPU Rendering**: Powered by Direct2D & DirectWrite premultiplied-alpha composition (`UpdateLayeredWindow`). The physics loop sleeps completely (`WaitMessage`) when no animation is active.
- **Real Spring Physics (120+ FPS)**: Every hover, expansion, and alert is driven by a damped harmonic oscillator (`Bouncy`, `Snappy`, and `Smooth` presets).
- **True Bezier Pill ↔ Notch Morphing**: Seamlessly morphs from a floating capsule into a flush concave flared notch at the top bezel of your monitor.
- **100% Click-Through Outside the Capsule**: Custom `WM_NCHITTEST` hit-testing ensures areas outside the active pill never block your browser tabs or window titlebars.
- **4 Built-In Interactive Tabs** (switch via click or mouse-wheel scroll):
  1. **🎵 Media**: Real-time Windows SMTC integration (Spotify, YouTube, Apple Music), animated 6-bar equalizer, and Focus Mode gatekeepers (`Normal`, `Gaming`, `Movie`).
  2. **📥 File Shelf**: Drag & drop files or folders anywhere onto Pillify to pin them for instant 1-click launch.
  3. **⏱ Focus & Wellness**: Pomodoro timer (`25m`, `5m`, `15m`), Daily Hydration / Water tracker with automatic reminders, and an interactive To-Do checklist.
  4. **✨ Notch Customizer Studio**: Live customization of Notch Mode (`Hybrid Morph`, `Always Notch`, `Floating Pill`), Spring Physics preset, Accent Color Theme, Compact Width, Corner Radius, Ear Flare Radius, and Border Glow—automatically persisted to `%LOCALAPPDATA%\Pillify\config.json`.

---

## ⌨️ Shortcuts & Controls

| Action | Control |
|--------|---------|
| **Expand / Interact** | Left-click the capsule or any pill button |
| **Switch Dashboard Tabs** | Scroll mouse wheel while expanded, or click tab pills |
| **Collapse to Compact Pill** | Right-click inside the expanded notch, or click `✕` |
| **Pin Files to Shelf** | Drag & drop files/folders from File Explorer onto the pill |
| **Toggle Visibility (DND)** | `Ctrl + Alt + I` (or via System Tray icon) |

---

## 🛠️ Build & Run

```powershell
cargo test
cargo run --release
```

