// Pillify — User Configuration & Live Notch Customizer Persistence
// Stored locally in %LOCALAPPDATA%\Pillify\config.json with strict input sanitization.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IslandStyle {
    /// Floating iPhone-style capsule with top margin, morphs into Notch on expand
    HybridMorph,
    /// Always docked flush to the top bezel with concave MacBook-style flared ears
    AlwaysNotch,
    /// Always floating rounded pill even when expanded
    AlwaysFloatingPill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpringPreset {
    Bouncy,
    Snappy,
    Smooth,
}

impl SpringPreset {
    pub fn params(self) -> (f32, f32) {
        match self {
            SpringPreset::Bouncy => (265.0, 18.0),
            SpringPreset::Snappy => (380.0, 27.0),
            SpringPreset::Smooth => (230.0, 29.0),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SpringPreset::Bouncy => "Bouncy",
            SpringPreset::Snappy => "Snappy",
            SpringPreset::Smooth => "Smooth",
        }
    }

    pub fn next(self) -> Self {
        match self {
            SpringPreset::Bouncy => SpringPreset::Snappy,
            SpringPreset::Snappy => SpringPreset::Smooth,
            SpringPreset::Smooth => SpringPreset::Bouncy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccentTheme {
    CyanPulse,
    EmeraldNusa,
    SunsetCoral,
    RoyalViolet,
    MonochromeWhite,
}

impl AccentTheme {
    /// Returns (Primary RGB, Secondary RGB) in normalized 0.0..=1.0
    pub fn colors(self) -> ((f32, f32, f32), (f32, f32, f32)) {
        match self {
            AccentTheme::CyanPulse => ((0.0, 0.898, 1.0), (0.314, 0.980, 0.482)),
            AccentTheme::EmeraldNusa => ((0.180, 0.925, 0.580), (0.0, 0.820, 0.950)),
            AccentTheme::SunsetCoral => ((1.0, 0.450, 0.380), (1.0, 0.750, 0.280)),
            AccentTheme::RoyalViolet => ((0.680, 0.480, 1.0), (0.0, 0.898, 1.0)),
            AccentTheme::MonochromeWhite => ((0.96, 0.96, 0.98), (0.72, 0.72, 0.78)),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AccentTheme::CyanPulse => "Cyan",
            AccentTheme::EmeraldNusa => "Nusa Green",
            AccentTheme::SunsetCoral => "Coral",
            AccentTheme::RoyalViolet => "Violet",
            AccentTheme::MonochromeWhite => "Mono",
        }
    }

    pub fn next(self) -> Self {
        match self {
            AccentTheme::CyanPulse => AccentTheme::EmeraldNusa,
            AccentTheme::EmeraldNusa => AccentTheme::SunsetCoral,
            AccentTheme::SunsetCoral => AccentTheme::RoyalViolet,
            AccentTheme::RoyalViolet => AccentTheme::MonochromeWhite,
            AccentTheme::MonochromeWhite => AccentTheme::CyanPulse,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PillifyConfig {
    pub style: IslandStyle,
    pub spring_preset: SpringPreset,
    pub accent_theme: AccentTheme,
    pub compact_width: f32,
    pub compact_height: f32,
    pub expanded_width: f32,
    pub expanded_height: f32,
    pub top_margin: f32,
    pub ear_flare_radius: f32,
    pub corner_radius: f32,
    pub border_glow_opacity: f32,
    pub auto_hide_fullscreen: bool,
    pub water_glasses_today: u32,
    pub water_daily_goal: u32,
    pub water_reminder_minutes: u32,
    pub todos: Vec<TodoItem>,
    pub shelf_files: Vec<String>,
}

impl Default for PillifyConfig {
    fn default() -> Self {
        Self {
            style: IslandStyle::HybridMorph,
            spring_preset: SpringPreset::Bouncy,
            accent_theme: AccentTheme::CyanPulse,
            compact_width: 236.0,
            compact_height: 40.0,
            expanded_width: 450.0,
            expanded_height: 224.0,
            top_margin: 8.0,
            ear_flare_radius: 14.0,
            corner_radius: 20.0,
            border_glow_opacity: 0.24,
            auto_hide_fullscreen: true,
            water_glasses_today: 2,
            water_daily_goal: 8,
            water_reminder_minutes: 45,
            todos: vec![
                TodoItem {
                    text: "Ship Pillify v2.0 in Rust".to_string(),
                    done: true,
                },
                TodoItem {
                    text: "Customize Notch shape & spring physics".to_string(),
                    done: false,
                },
                TodoItem {
                    text: "Drag & drop files into File Shelf".to_string(),
                    done: false,
                },
            ],
            shelf_files: Vec::new(),
        }
    }
}

impl PillifyConfig {
    /// Validates and clamps all user-modifiable fields so corrupted JSON cannot break rendering.
    pub fn sanitize(&mut self) {
        self.compact_width = self.compact_width.clamp(180.0, 340.0);
        self.compact_height = self.compact_height.clamp(32.0, 56.0);
        self.expanded_width = self.expanded_width.clamp(380.0, 540.0);
        self.expanded_height = self.expanded_height.clamp(190.0, 270.0);
        self.top_margin = self.top_margin.clamp(0.0, 24.0);
        self.ear_flare_radius = self.ear_flare_radius.clamp(6.0, 24.0);
        self.corner_radius = self.corner_radius.clamp(12.0, 28.0);
        self.border_glow_opacity = self.border_glow_opacity.clamp(0.05, 0.70);
        self.water_daily_goal = self.water_daily_goal.clamp(4, 16);
        self.water_glasses_today = self.water_glasses_today.min(30);
        self.todos.truncate(12);
        for item in &mut self.todos {
            item.text = item.text.trim().chars().take(60).collect();
        }
        self.shelf_files.retain(|p| !p.trim().is_empty());
        self.shelf_files.truncate(24);
    }

    pub fn config_path() -> Option<PathBuf> {
        let local_app_data = std::env::var_os("LOCALAPPDATA")?;
        let mut dir = PathBuf::from(local_app_data);
        dir.push("Pillify");
        let _ = fs::create_dir_all(&dir);
        dir.push("config.json");
        Some(dir)
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if let Ok(raw) = fs::read_to_string(path) {
                if let Ok(mut parsed) = serde_json::from_str::<Self>(&raw) {
                    parsed.sanitize();
                    return parsed;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Ok(serialized) = serde_json::to_string_pretty(self) {
                let _ = fs::write(path, serialized);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_sanitizes_out_of_range_values() {
        let mut cfg = PillifyConfig {
            compact_width: 9999.0,
            top_margin: -50.0,
            border_glow_opacity: 5.0,
            ..Default::default()
        };
        cfg.sanitize();
        assert_eq!(cfg.compact_width, 340.0);
        assert_eq!(cfg.top_margin, 0.0);
        assert_eq!(cfg.border_glow_opacity, 0.70);
    }
}

