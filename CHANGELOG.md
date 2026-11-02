# Changelog

All notable changes to **Pillify** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v2.0.0] - 2026-11-03
### Added
- **Full Rust + Direct2D GPU Rewrite (`Pillify`)**: Complete migration from .NET 10 WPF to native Rust (`windows-rs` + Direct2D + DirectWrite + `UpdateLayeredWindow` premultiplied alpha) with 0% CPU usage when idle.
- **Real Spring Physics Engine (`src/spring.rs`)**: Damped harmonic oscillator with velocity preservation and 3 user-selectable physics presets (`Bouncy`, `Snappy`, `Smooth`).
- **True Mathematical Bezier Notch Morphing (`src/renderer.rs`)**: Smooth geometric interpolation between a floating rounded pill and a MacBook/NUSA-style concave top-bezel notch.
- **Smart Click-Through Hit-Testing (`WM_NCHITTEST`)**: Pixels outside the active capsule/notch return `HTTRANSPARENT` so clicks pass directly through to underlying windows.
- **4 Interactive Dashboard Pages**:
  - `🎵 Media`: WinRT SMTC playback controls, animated 6-bar audio equalizer, live lyrics/status banner, and Focus Mode chips (`Normal`, `Gaming`, `Movie`).
  - `📥 Shelf`: Drag-and-drop file & folder shelf with extension badges and one-click Explorer launch.
  - `⏱ Focus`: Interactive Pomodoro countdown timer (`25m`, `5m`, `15m`), Daily Water Intake tracker (`+1 Cup` & automatic hydration reminders), and interactive Quick Tasks checklist.
  - `✨ Notch`: Live in-app Notch Customizer Studio to tweak Island Mode (`Hybrid Morph`, `Always Notch`, `Floating Pill`), Spring Preset, Accent Theme, Compact Width, Corner Radius, Notch Ear Flare, and Border Glow opacity in real time.
- **Hardware Volume OSD & Auto-Hide Fullscreen**: Automatic volume bar morphing when pressing media volume keys and automatic suppression during fullscreen games (`SHQueryUserNotificationState`).
- **System Tray Icon & Context Menu**: Quick access to Show/Hide (`Ctrl+Alt+I`), Notch Customizer Studio, Test Notification Morph, and Exit.

### Changed
- Renamed project directory, package, and application identity from `dynamic_island` to **Pillify**.

### Removed
- Legacy .NET 10 WPF XAML storyboards, `CommunityToolkit.Mvvm`, and `NAudio` dependencies.

