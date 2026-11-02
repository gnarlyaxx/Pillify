# Changelog

All notable changes to **Pillify** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
