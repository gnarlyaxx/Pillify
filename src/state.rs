// Pillify — Reactive Application State & Hit-Test Action Mapping
// Drives the 4 interactive dashboard tabs, compact status bar, Pomodoro timer, Water tracker, and Notch Customizer.

use crate::config::{IslandStyle, PillifyConfig};
use crate::spring::IslandAnimator;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Default,
    NotificationCompact,
    Expanded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMode {
    None,
    Gaming,
    Movie,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePage {
    Media = 0,
    Shelf = 1,
    Productivity = 2,
    Customize = 3,
}

impl ActivePage {
    pub fn from_index(idx: usize) -> Self {
        match idx % 4 {
            0 => ActivePage::Media,
            1 => ActivePage::Shelf,
            2 => ActivePage::Productivity,
            _ => ActivePage::Customize,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ShelfEntry {
    pub full_path: String,
    pub display_name: String,
    pub extension_badge: String,
    pub is_dir: bool,
}

impl ShelfEntry {
    pub fn from_path(raw_path: &str) -> Self {
        let path = Path::new(raw_path);
        let is_dir = path.is_dir();
        let display_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(raw_path)
            .to_string();
        let extension_badge = if is_dir {
            "DIR".to_string()
        } else {
            path.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_uppercase())
                .unwrap_or_else(|| "FILE".to_string())
        };

        Self {
            full_path: raw_path.to_string(),
            display_name,
            extension_badge,
            is_dir,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum InteractiveControl {
    IslandBody,
    CollapseButton,
    TabButton(ActivePage),
    MediaPrev,
    MediaPlayPause,
    MediaNext,
    FocusChip(FocusMode),
    ShelfOpenItem(usize),
    ShelfRemoveItem(usize),
    ShelfClearAll,
    PomodoroToggle,
    PomodoroReset,
    PomodoroPreset(u32), // minutes: 25, 5, 15
    WaterAddGlass,
    WaterReset,
    TodoToggle(usize),
    CustomizeStyleCycle,
    CustomizeSpringCycle,
    CustomizeThemeCycle,
    CustomizeWidthMinus,
    CustomizeWidthPlus,
    CustomizeRadiusMinus,
    CustomizeRadiusPlus,
    CustomizeFlareMinus,
    CustomizeFlarePlus,
    CustomizeGlowMinus,
    CustomizeGlowPlus,
    DismissToast,
}

#[derive(Debug, Clone, Copy)]
pub struct HitZone {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub control: InteractiveControl,
}

impl HitZone {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

pub struct AppState {
    pub config: PillifyConfig,
    pub animator: IslandAnimator,
    pub mode: DisplayMode,
    pub focus_mode: FocusMode,
    pub page: ActivePage,
    pub visible: bool,
    pub suppressed_by_fullscreen: bool,

    // Cursor & interaction tracking
    pub is_hovered: bool,
    pub cursor_pos: (f32, f32),
    pub hovered_control: Option<InteractiveControl>,
    pub hit_zones: Vec<HitZone>,

    // Clock & System Telemetry
    pub current_time_str: String,
    pub battery_percent: u32,
    pub is_charging: bool,
    pub network_connected: bool,
    pub wifi_ssid: String,

    // Audio, SMTC Media & Synced Lyrics / Spectrum
    pub song_title: String,
    pub artist_name: String,
    pub is_playing: bool,
    pub volume_percent: u32,
    pub is_muted: bool,
    pub audio_spectrum: [f32; 6],
    pub spectrum_phase: f32,
    pub synced_lyric_line: String,
    pub marquee_offset: f32,

    // Volume OSD overlay timer
    pub last_volume_change: Option<Instant>,

    // Notification / Alert banner
    pub toast_app: String,
    pub toast_title: String,
    pub toast_body: String,
    pub toast_expires_at: Option<Instant>,

    // File Shelf
    pub shelf_items: Vec<ShelfEntry>,
    pub is_drag_hovering: bool,

    // Pomodoro & Water Reminder
    pub timer_total_secs: u32,
    pub timer_remaining_secs: u32,
    pub timer_running: bool,
    pub timer_mode_label: String,
    pub last_timer_tick: Instant,
    pub last_water_reminder: Instant,
}

impl AppState {
    pub fn new(config: PillifyConfig) -> Self {
        let is_notch = config.style == IslandStyle::AlwaysNotch;
        let mut animator = IslandAnimator::new(
            config.compact_width,
            config.compact_height,
            config.top_margin,
            config.corner_radius,
            is_notch,
        );
        let (stiff, damp) = config.spring_preset.params();
        animator.set_physics_preset(stiff, damp);

        let shelf_items = config
            .shelf_files
            .iter()
            .map(|p| ShelfEntry::from_path(p))
            .collect();

        Self {
            config,
            animator,
            mode: DisplayMode::Default,
            focus_mode: FocusMode::None,
            page: ActivePage::Media,
            visible: true,
            suppressed_by_fullscreen: false,
            is_hovered: false,
            cursor_pos: (-100.0, -100.0),
            hovered_control: None,
            hit_zones: Vec::with_capacity(32),
            current_time_str: "00:00".to_string(),
            battery_percent: 100,
            is_charging: true,
            network_connected: true,
            wifi_ssid: "Connected".to_string(),
            song_title: String::new(),
            artist_name: String::new(),
            is_playing: false,
            volume_percent: 65,
            is_muted: false,
            audio_spectrum: [0.25, 0.55, 0.85, 0.45, 0.70, 0.35],
            spectrum_phase: 0.0,
            synced_lyric_line: "♪ Pillify Interactive Audio Ready".to_string(),
            marquee_offset: 0.0,
            last_volume_change: None,
            toast_app: String::new(),
            toast_title: String::new(),
            toast_body: String::new(),
            toast_expires_at: None,
            shelf_items,
            is_drag_hovering: false,
            timer_total_secs: 25 * 60,
            timer_remaining_secs: 25 * 60,
            timer_running: false,
            timer_mode_label: "Focus 25m".to_string(),
            last_timer_tick: Instant::now(),
            last_water_reminder: Instant::now(),
        }
    }

    pub fn is_music_active(&self) -> bool {
        self.is_playing && !self.song_title.is_empty()
    }

    /// Recomputes spring targets whenever state, hover, media playback, or customization changes.
    pub fn sync_spring_targets(&mut self) {
        let (stiff, damp) = self.config.spring_preset.params();
        self.animator.set_physics_preset(stiff, damp);

        let hover_bonus_w = if self.is_hovered && self.mode != DisplayMode::Expanded {
            14.0
        } else {
            0.0
        };
        let hover_bonus_h = if self.is_hovered && self.mode != DisplayMode::Expanded {
            3.0
        } else {
            0.0
        };

        match self.mode {
            DisplayMode::Default => {
                let mut target_w = self.config.compact_width;
                if self.is_music_active() {
                    target_w = (target_w + 125.0).max(360.0);
                } else if self.timer_running {
                    target_w = (target_w + 68.0).max(296.0);
                }
                self.animator.width.set_target(target_w + hover_bonus_w);
                self.animator
                    .height
                    .set_target(self.config.compact_height + hover_bonus_h);

                match self.config.style {
                    IslandStyle::AlwaysNotch => {
                        self.animator.top_offset.set_target(0.0);
                        self.animator.notch_flare.set_target(1.0);
                    }
                    IslandStyle::HybridMorph | IslandStyle::AlwaysFloatingPill => {
                        self.animator.top_offset.set_target(self.config.top_margin);
                        self.animator.notch_flare.set_target(0.0);
                    }
                }
                self.animator
                    .corner_radius
                    .set_target(self.config.corner_radius);
                self.animator.expand_progress.set_target(0.0);
                self.animator.compact_alert_blend.set_target(0.0);
            }
            DisplayMode::NotificationCompact => {
                self.animator.width.set_target(360.0 + hover_bonus_w);
                self.animator
                    .height
                    .set_target((self.config.compact_height + 6.0) + hover_bonus_h);

                let flush = self.config.style == IslandStyle::AlwaysNotch;
                self.animator
                    .top_offset
                    .set_target(if flush { 0.0 } else { self.config.top_margin });
                self.animator
                    .notch_flare
                    .set_target(if flush { 1.0 } else { 0.0 });
                self.animator
                    .corner_radius
                    .set_target(self.config.corner_radius);
                self.animator.expand_progress.set_target(0.0);
                self.animator.compact_alert_blend.set_target(1.0);
            }
            DisplayMode::Expanded => {
                self.animator.width.set_target(self.config.expanded_width);
                self.animator.height.set_target(self.config.expanded_height);

                match self.config.style {
                    IslandStyle::HybridMorph | IslandStyle::AlwaysNotch => {
                        self.animator.top_offset.set_target(0.0);
                        self.animator.notch_flare.set_target(1.0);
                    }
                    IslandStyle::AlwaysFloatingPill => {
                        self.animator.top_offset.set_target(self.config.top_margin);
                        self.animator.notch_flare.set_target(0.0);
                    }
                }
                self.animator
                    .corner_radius
                    .set_target((self.config.corner_radius + 4.0).min(28.0));
                self.animator.expand_progress.set_target(1.0);
                self.animator.compact_alert_blend.set_target(0.0);
            }
        }

        self.animator
            .hover_scale
            .set_target(if self.is_hovered { 1.02 } else { 1.0 });
        self.animator
            .page_scroll
            .set_target(self.page.index() as f32);

        let osd_active = self
            .last_volume_change
            .map(|t| t.elapsed() < Duration::from_millis(1650))
            .unwrap_or(false);
        self.animator
            .osd_blend
            .set_target(if osd_active { 1.0 } else { 0.0 });
    }

    pub fn trigger_notification(&mut self, app: &str, title: &str, body: &str) {
        if self.focus_mode == FocusMode::Gaming {
            return;
        }
        self.toast_app = if app.trim().is_empty() {
            "System".to_string()
        } else {
            app.to_string()
        };
        self.toast_title = if title.trim().is_empty() {
            "Notification".to_string()
        } else {
            title.to_string()
        };
        self.toast_body = body.to_string();
        self.toast_expires_at = Some(Instant::now() + Duration::from_secs(5));

        if self.mode == DisplayMode::Default {
            self.mode = DisplayMode::NotificationCompact;
        }
        self.sync_spring_targets();
    }

    pub fn add_dropped_files(&mut self, paths: Vec<String>) {
        for path in paths {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !self
                .shelf_items
                .iter()
                .any(|item| item.full_path.eq_ignore_ascii_case(trimmed))
            {
                self.shelf_items.insert(0, ShelfEntry::from_path(trimmed));
            }
        }
        self.shelf_items.truncate(24);
        self.config.shelf_files = self
            .shelf_items
            .iter()
            .map(|i| i.full_path.clone())
            .collect();
        self.config.save();

        self.mode = DisplayMode::Expanded;
        self.page = ActivePage::Shelf;
        self.sync_spring_targets();
    }

    pub fn set_page(&mut self, page: ActivePage) {
        self.page = page;
        self.sync_spring_targets();
    }

    pub fn cycle_page(&mut self, delta: i32) {
        let current = self.page.index() as i32;
        let next = (current + delta).rem_euclid(4) as usize;
        self.set_page(ActivePage::from_index(next));
    }

    pub fn timer_formatted(&self) -> String {
        let mins = self.timer_remaining_secs / 60;
        let secs = self.timer_remaining_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    #[allow(dead_code)]
    pub fn timer_progress(&self) -> f32 {
        if self.timer_total_secs == 0 {
            0.0
        } else {
            (self.timer_remaining_secs as f32 / self.timer_total_secs as f32).clamp(0.0, 1.0)
        }
    }

    pub fn tick_periodic(&mut self, dt: f32) -> bool {
        let mut needs_redraw = false;

        // Advance audio visualizer & marquee only when playing music
        if self.is_music_active() {
            self.spectrum_phase += dt * 7.5;
            for (i, bar) in self.audio_spectrum.iter_mut().enumerate() {
                let wave = ((self.spectrum_phase + (i as f32) * 1.15).sin() * 0.5 + 0.5)
                    * ((self.spectrum_phase * 0.65 + (i as f32) * 0.7).cos().abs());
                *bar = (0.18 + wave * 0.82).clamp(0.15, 1.0);
            }
            self.marquee_offset += dt * 34.0;
            if self.marquee_offset > 320.0 {
                self.marquee_offset = -180.0;
            }
            needs_redraw = true;
        }

        // Check toast auto-dismiss
        if let Some(expiry) = self.toast_expires_at {
            if Instant::now() >= expiry {
                self.toast_expires_at = None;
                if self.mode == DisplayMode::NotificationCompact {
                    self.mode = DisplayMode::Default;
                    self.sync_spring_targets();
                    needs_redraw = true;
                }
            }
        }

        // Check Volume OSD expiry
        if let Some(vol_time) = self.last_volume_change {
            if vol_time.elapsed() >= Duration::from_millis(1650) {
                self.last_volume_change = None;
                self.sync_spring_targets();
                needs_redraw = true;
            }
        }

        // Check Pomodoro 1-second countdown
        if self.timer_running && self.last_timer_tick.elapsed() >= Duration::from_secs(1) {
            self.last_timer_tick = Instant::now();
            if self.timer_remaining_secs > 0 {
                self.timer_remaining_secs -= 1;
                needs_redraw = true;
                if self.timer_remaining_secs == 0 {
                    self.timer_running = false;
                    self.trigger_notification(
                        "Pillify Focus",
                        "Session Complete! 🎯",
                        "Great focus block! Take a short break & hydrate.",
                    );
                }
            }
        }

        // Check Water Reminder interval
        let reminder_secs = (self.config.water_reminder_minutes.max(15) as u64) * 60;
        if self.last_water_reminder.elapsed() >= Duration::from_secs(reminder_secs) {
            self.last_water_reminder = Instant::now();
            if self.config.water_glasses_today < self.config.water_daily_goal {
                self.trigger_notification(
                    "Hydration",
                    "Time to Drink Water 💧",
                    &format!(
                        "Progress: {}/{} glasses today.",
                        self.config.water_glasses_today, self.config.water_daily_goal
                    ),
                );
                needs_redraw = true;
            }
        }

        needs_redraw
    }
}

