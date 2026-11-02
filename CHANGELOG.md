# Changelog

All notable changes to **Pillify** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v1.1.0] - 2026-11-03
### Added
- **Live Synced Lyrics (`src/services.rs`, `src/renderer.rs`)**: Automatic LRC timestamped lyrics fetcher via LRCLIB HTTPS API synchronized to Windows SMTC `TimelineProperties` playback position, rendered in a dedicated 2-line lyrics card inside the `Now Playing` view.
- **Native Application & Shortcut Icon Extraction (`src/state.rs`, `src/renderer.rs`)**: Automatically extracts 32x32 32-bit premultiplied BGRA icons from `.lnk` shortcuts, `.exe` binaries, and folders via Win32 `SHGetFileInfoW` + `DrawIconEx` and renders them as Direct2D bitmaps in `Tray`.
- **Single Focus Mode On/Off Toggle**: Replaced the 3 static mode buttons with a single `Focus Mode: ON / OFF` toggle pill that suppresses incoming notifications, starts the 25-minute focus timer if idle, and displays a compact focus indicator when collapsed.

### Fixed
- **Expanded Notch Layout & Text Clipping**: Expanded default notch height (`244px`) and width (`484px`) and enforced `D2D1_DRAW_TEXT_OPTIONS_CLIP` + DirectWrite character ellipsis trimming (`...`) so long song titles never overlap transport buttons.

## [v1.0.2] - 2026-11-03
### Changed
- **macOS Notch / Dynamic Island Visual Refinement**: Set default geometry to flush top-bezel MacBook Notch (`AlwaysNotch`, `#000000` pitch-black) with fluid `Bouncy` spring physics, full-capsule pill controls (`Now Playing`, `Tray`, `Timer`, `Notch`), and crisp native `Segoe Fluent Icons` media transport glyphs.

## [v1.0.1] - 2026-11-03
### Changed
- **Anti-Slop UI & Typography Overhaul**: Replaced all emojis, decorative badges, and neon gradients with a high-contrast matte black (`#09090B`) hardware aesthetic, 1px structural hairlines, and monospace (`Consolas`) + `Segoe UI Variable Text` technical readouts.
- **Structured Shelf Layout**: Replaced generic card grids in the `SHELF` view with structured file rows (`[EXT] filename` + `DEL`).
- **Documentation**: Rewrote `README.md` with concise technical documentation and zero decorative emojis.

## [v1.0.0] - 2026-11-03
### Added
- Initial native Rust + Direct2D implementation of Pillify (`src/spring.rs`, `src/renderer.rs`, `src/state.rs`, `src/services.rs`, `src/config.rs`, `src/main.rs`).
- Spring-driven Bezier capsule-to-notch morphing, `WM_NCHITTEST` click-through hit-testing, SMTC media integration, file shelf, Pomodoro/hydration tracker, and live geometry customizer.
