use crate::config::{IslandStyle, PillifyConfig};
use crate::spring::IslandAnimator;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Default,
    NotificationCompact,
    Expanded,
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
pub struct LyricLine {
    pub timestamp_secs: f32,
    pub text: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ShelfEntry {
    pub full_path: String,
    pub display_name: String,
    pub extension_badge: String,
    pub is_dir: bool,
    pub icon_bgra: Option<Vec<u8>>,
    pub icon_size: u32,
}

impl ShelfEntry {
    pub fn from_path(raw_path: &str) -> Self {
        let path = Path::new(raw_path);
        let is_dir = path.is_dir();

        // Strip `.lnk` or `.url` from shortcut display names so "VALORANT.lnk" shows cleanly as "VALORANT"
        let raw_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(raw_path);
        let display_name = if raw_name.to_ascii_lowercase().ends_with(".lnk")
            || raw_name.to_ascii_lowercase().ends_with(".url")
        {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(raw_name)
                .to_string()
        } else {
            raw_name.to_string()
        };

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| "FILE".to_string());

        let extension_badge = if is_dir {
            "Folder".to_string()
        } else if ext == "LNK" || ext == "EXE" || ext == "URL" {
            "Application".to_string()
        } else {
            ext
        };

        let icon_bgra = extract_file_icon_bgra(raw_path);

        Self {
            full_path: raw_path.to_string(),
            display_name,
            extension_badge,
            is_dir,
            icon_bgra,
            icon_size: 32,
        }
    }
}

/// Extracts a 32x32 32-bit premultiplied BGRA pixel buffer from any file, folder, or `.lnk` shortcut via Win32 Shell/GDI.
fn extract_file_icon_bgra(path_str: &str) -> Option<Vec<u8>> {
    use std::mem::size_of;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL};

    unsafe {
        let mut wide: Vec<u16> = path_str.encode_utf16().collect();
        wide.push(0);

        let mut shfi = SHFILEINFOW::default();
        let res = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut shfi),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );

        if res == 0 || shfi.hIcon.is_invalid() {
            return None;
        }

        let icon_w = 32i32;
        let icon_h = 32i32;
        let screen_dc = GetDC(HWND::default());
        let mem_dc = CreateCompatibleDC(screen_dc);
        ReleaseDC(HWND::default(), screen_dc);

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: icon_w,
                biHeight: -icon_h, // top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let Ok(dib) = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DestroyIcon(shfi.hIcon);
            let _ = DeleteDC(mem_dc);
            return None;
        };

        let old = SelectObject(mem_dc, dib);
        let _ = DrawIconEx(mem_dc, 0, 0, shfi.hIcon, icon_w, icon_h, 0, None, DI_NORMAL);

        let byte_len = (icon_w * icon_h * 4) as usize;
        let mut pixels = vec![0u8; byte_len];
        if !bits.is_null() {
            std::ptr::copy_nonoverlapping(bits as *const u8, pixels.as_mut_ptr(), byte_len);
        }

        // Convert straight alpha to premultiplied alpha for Direct2D compatibility
        for chunk in pixels.chunks_exact_mut(4) {
            let a = chunk[3] as u32;
            if a == 0 {
                chunk[0] = 0;
                chunk[1] = 0;
                chunk[2] = 0;
            } else if a < 255 {
                chunk[0] = ((chunk[0] as u32 * a) / 255) as u8;
                chunk[1] = ((chunk[1] as u32 * a) / 255) as u8;
                chunk[2] = ((chunk[2] as u32 * a) / 255) as u8;
            }
        }

        let _ = SelectObject(mem_dc, old);
        let _ = DeleteObject(dib);
        let _ = DeleteDC(mem_dc);
        let _ = DestroyIcon(shfi.hIcon);

        Some(pixels)
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
    FocusModeToggle,
    ShelfOpenItem(usize),
    ShelfRemoveItem(usize),
    ShelfClearAll,
    PomodoroToggle,
    PomodoroReset,
    PomodoroPreset(u32),
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

pub struct SharedLyricsBuffer {
    pub track_key: String,
    pub lines: Vec<LyricLine>,
    pub plain_fallback: String,
    pub loading: bool,
}

pub struct AppState {
    pub config: PillifyConfig,
    pub animator: IslandAnimator,
    pub mode: DisplayMode,
    pub focus_mode_enabled: bool,
    pub page: ActivePage,
    pub visible: bool,
    pub suppressed_by_fullscreen: bool,

    pub is_hovered: bool,
    pub cursor_pos: (f32, f32),
    pub hovered_control: Option<InteractiveControl>,
    pub hit_zones: Vec<HitZone>,

    pub current_time_str: String,
    pub battery_percent: u32,
    pub is_charging: bool,
    pub network_connected: bool,
    pub wifi_ssid: String,

    pub song_title: String,
    pub artist_name: String,
    pub is_playing: bool,
    pub playback_position_secs: f32,
    pub playback_duration_secs: f32,
    pub volume_percent: u32,
    pub is_muted: bool,
    pub audio_spectrum: [f32; 6],
    pub spectrum_phase: f32,
    pub synced_lyric_line: String,
    pub next_lyric_line: String,
    pub shared_lyrics: Arc<Mutex<SharedLyricsBuffer>>,
    pub marquee_offset: f32,

    pub last_volume_change: Option<Instant>,

    pub toast_app: String,
    pub toast_title: String,
    pub toast_body: String,
    pub toast_expires_at: Option<Instant>,

    pub shelf_items: Vec<ShelfEntry>,

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
            focus_mode_enabled: false,
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
            playback_position_secs: 0.0,
            playback_duration_secs: 210.0,
            volume_percent: 65,
            is_muted: false,
            audio_spectrum: [0.25, 0.55, 0.85, 0.45, 0.70, 0.35],
            spectrum_phase: 0.0,
            synced_lyric_line: "Play any song to view live synced lyrics".to_string(),
            next_lyric_line: String::new(),
            shared_lyrics: Arc::new(Mutex::new(SharedLyricsBuffer {
                track_key: String::new(),
                lines: Vec::new(),
                plain_fallback: String::new(),
                loading: false,
            })),
            marquee_offset: 0.0,
            last_volume_change: None,
            toast_app: String::new(),
            toast_title: String::new(),
            toast_body: String::new(),
            toast_expires_at: None,
            shelf_items,
            timer_total_secs: 25 * 60,
            timer_remaining_secs: 25 * 60,
            timer_running: false,
            timer_mode_label: "Focus Session".to_string(),
            last_timer_tick: Instant::now(),
            last_water_reminder: Instant::now(),
        }
    }

    pub fn is_music_active(&self) -> bool {
        self.is_playing && !self.song_title.is_empty()
    }

    pub fn sync_spring_targets(&mut self) {
        let (stiff, damp) = self.config.spring_preset.params();
        self.animator.set_physics_preset(stiff, damp);

        let hover_bonus_w = if self.is_hovered && self.mode != DisplayMode::Expanded {
            12.0
        } else {
            0.0
        };
        let hover_bonus_h = if self.is_hovered && self.mode != DisplayMode::Expanded {
            2.0
        } else {
            0.0
        };

        match self.mode {
            DisplayMode::Default => {
                let mut target_w = self.config.compact_width;
                if self.is_music_active() && !self.focus_mode_enabled {
                    target_w = (target_w + 125.0).max(352.0);
                } else if self.timer_running || self.focus_mode_enabled {
                    target_w = (target_w + 70.0).max(292.0);
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
                self.animator.width.set_target(356.0 + hover_bonus_w);
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
            .set_target(if self.is_hovered { 1.015 } else { 1.0 });
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

    /// Toggles real Focus Mode: suppresses notifications, starts the 25m focus timer if idle, and shows the Focus indicator.
    pub fn toggle_focus_mode(&mut self) {
        self.focus_mode_enabled = !self.focus_mode_enabled;
        if self.focus_mode_enabled {
            if !self.timer_running {
                if self.timer_remaining_secs == 0 {
                    self.timer_remaining_secs = self.timer_total_secs.max(25 * 60);
                }
                self.timer_running = true;
                self.last_timer_tick = Instant::now();
            }
        }
        self.sync_spring_targets();
    }

    pub fn trigger_notification(&mut self, app: &str, title: &str, body: &str) {
        // Real Focus Mode behavior: suppress all incoming popups & alerts while Focus Mode is ON
        if self.focus_mode_enabled {
            return;
        }
        self.toast_app = if app.trim().is_empty() {
            "System".to_string()
        } else {
            app.to_string()
        };
        self.toast_title = if title.trim().is_empty() {
            "Alert".to_string()
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
                .any(|entry| entry.full_path.eq_ignore_ascii_case(trimmed))
            {
                self.shelf_items.insert(0, ShelfEntry::from_path(trimmed));
            }
        }
        self.shelf_items.truncate(24);
        self.config.shelf_files = self
            .shelf_items
            .iter()
            .map(|entry| entry.full_path.clone())
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

    pub fn update_active_lyric_from_timeline(&mut self) {
        let Ok(guard) = self.shared_lyrics.try_lock() else {
            return;
        };

        if guard.loading {
            self.synced_lyric_line = "Fetching synced lyrics...".to_string();
            self.next_lyric_line.clear();
            return;
        }

        if !guard.lines.is_empty() {
            let pos = self.playback_position_secs;
            let mut active_idx = 0usize;
            for (idx, line) in guard.lines.iter().enumerate() {
                if line.timestamp_secs <= pos + 0.25 {
                    active_idx = idx;
                } else {
                    break;
                }
            }
            self.synced_lyric_line = guard.lines[active_idx].text.clone();
            self.next_lyric_line = guard
                .lines
                .get(active_idx + 1)
                .map(|l| l.text.clone())
                .unwrap_or_default();
        } else if !guard.plain_fallback.is_empty() {
            self.synced_lyric_line = guard.plain_fallback.clone();
            self.next_lyric_line.clear();
        } else if !self.song_title.is_empty() {
            self.synced_lyric_line = format!("♪  {} — {}", self.song_title, self.artist_name);
            self.next_lyric_line.clear();
        } else {
            self.synced_lyric_line = "Play any song to view live synced lyrics".to_string();
            self.next_lyric_line.clear();
        }
    }

    pub fn tick_periodic(&mut self, dt: f32) -> bool {
        let mut needs_redraw = false;

        if self.is_music_active() {
            self.playback_position_secs =
                (self.playback_position_secs + dt).min(self.playback_duration_secs.max(10.0));
            self.spectrum_phase += dt * 7.5;
            for (idx, bar) in self.audio_spectrum.iter_mut().enumerate() {
                let wave = ((self.spectrum_phase + (idx as f32) * 1.15).sin() * 0.5 + 0.5)
                    * ((self.spectrum_phase * 0.65 + (idx as f32) * 0.7).cos().abs());
                *bar = (0.18 + wave * 0.82).clamp(0.15, 1.0);
            }
            self.update_active_lyric_from_timeline();
            self.marquee_offset += dt * 34.0;
            if self.marquee_offset > 320.0 {
                self.marquee_offset = -180.0;
            }
            needs_redraw = true;
        }

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

        if let Some(vol_time) = self.last_volume_change {
            if vol_time.elapsed() >= Duration::from_millis(1650) {
                self.last_volume_change = None;
                self.sync_spring_targets();
                needs_redraw = true;
            }
        }

        if self.timer_running && self.last_timer_tick.elapsed() >= Duration::from_secs(1) {
            self.last_timer_tick = Instant::now();
            if self.timer_remaining_secs > 0 {
                self.timer_remaining_secs -= 1;
                needs_redraw = true;
                if self.timer_remaining_secs == 0 {
                    self.timer_running = false;
                    self.focus_mode_enabled = false;
                    self.trigger_notification(
                        "Focus",
                        "Session Complete",
                        "Focus interval finished.",
                    );
                }
            }
        }

        let reminder_secs = (self.config.water_reminder_minutes.max(15) as u64) * 60;
        if self.last_water_reminder.elapsed() >= Duration::from_secs(reminder_secs) {
            self.last_water_reminder = Instant::now();
            if self.config.water_glasses_today < self.config.water_daily_goal {
                self.trigger_notification(
                    "Hydration",
                    "Water Reminder",
                    &format!(
                        "{}/{} cups recorded today.",
                        self.config.water_glasses_today, self.config.water_daily_goal
                    ),
                );
                needs_redraw = true;
            }
        }

        needs_redraw
    }
}
