use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IslandStyle {
    HybridMorph,
    AlwaysNotch,
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
    GraphiteWhite,
    AmberSignal,
    EmeraldSignal,
    CobaltSignal,
}

impl AccentTheme {
    pub fn colors(self) -> ((f32, f32, f32), (f32, f32, f32)) {
        match self {
            AccentTheme::GraphiteWhite => ((0.96, 0.96, 0.96), (0.62, 0.62, 0.65)),
            AccentTheme::AmberSignal => ((0.96, 0.62, 0.04), (0.78, 0.78, 0.80)),
            AccentTheme::EmeraldSignal => ((0.16, 0.82, 0.48), (0.78, 0.78, 0.80)),
            AccentTheme::CobaltSignal => ((0.28, 0.56, 0.98), (0.78, 0.78, 0.80)),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AccentTheme::GraphiteWhite => "Monochrome",
            AccentTheme::AmberSignal => "Amber",
            AccentTheme::EmeraldSignal => "Emerald",
            AccentTheme::CobaltSignal => "Cobalt",
        }
    }

    pub fn next(self) -> Self {
        match self {
            AccentTheme::GraphiteWhite => AccentTheme::AmberSignal,
            AccentTheme::AmberSignal => AccentTheme::EmeraldSignal,
            AccentTheme::EmeraldSignal => AccentTheme::CobaltSignal,
            AccentTheme::CobaltSignal => AccentTheme::GraphiteWhite,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoTask {
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
    pub todos: Vec<TodoTask>,
    pub shelf_files: Vec<String>,
}

impl Default for PillifyConfig {
    fn default() -> Self {
        Self {
            style: IslandStyle::HybridMorph,
            spring_preset: SpringPreset::Snappy,
            accent_theme: AccentTheme::GraphiteWhite,
            compact_width: 224.0,
            compact_height: 36.0,
            expanded_width: 440.0,
            expanded_height: 204.0,
            top_margin: 8.0,
            ear_flare_radius: 14.0,
            corner_radius: 18.0,
            border_glow_opacity: 0.14,
            auto_hide_fullscreen: true,
            water_glasses_today: 2,
            water_daily_goal: 8,
            water_reminder_minutes: 45,
            todos: vec![
                TodoTask {
                    text: "Review system architecture".to_string(),
                    done: true,
                },
                TodoTask {
                    text: "Calibrate notch spring damping".to_string(),
                    done: false,
                },
                TodoTask {
                    text: "Stage files in shelf".to_string(),
                    done: false,
                },
            ],
            shelf_files: Vec::new(),
        }
    }
}

impl PillifyConfig {
    pub fn sanitize(&mut self) {
        self.compact_width = self.compact_width.clamp(180.0, 340.0);
        self.compact_height = self.compact_height.clamp(32.0, 52.0);
        self.expanded_width = self.expanded_width.clamp(380.0, 540.0);
        self.expanded_height = self.expanded_height.clamp(180.0, 260.0);
        self.top_margin = self.top_margin.clamp(0.0, 24.0);
        self.ear_flare_radius = self.ear_flare_radius.clamp(6.0, 24.0);
        self.corner_radius = self.corner_radius.clamp(12.0, 26.0);
        self.border_glow_opacity = self.border_glow_opacity.clamp(0.06, 0.45);
        self.water_daily_goal = self.water_daily_goal.clamp(4, 16);
        self.water_glasses_today = self.water_glasses_today.min(30);
        self.todos.truncate(12);
        for task in &mut self.todos {
            task.text = task.text.trim().chars().take(60).collect();
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
        assert_eq!(cfg.border_glow_opacity, 0.45);
    }
}
