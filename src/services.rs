use crate::state::{AppState, LyricLine, SharedLyricsBuffer};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use windows::core::HSTRING;
use windows::Foundation::Uri;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Networking::Connectivity::{NetworkConnectivityLevel, NetworkInformation};
use windows::Web::Http::HttpClient;
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};

pub struct SystemServices {
    smtc_manager: Option<GlobalSystemMediaTransportControlsSessionManager>,
    smtc_init_attempted: bool,
    endpoint_volume: Option<IAudioEndpointVolume>,
    last_media_poll: Instant,
    last_telemetry_poll: Instant,
}

impl SystemServices {
    pub fn new() -> Self {
        let endpoint_volume = unsafe {
            CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .and_then(|enumerator| enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia))
                .and_then(|device| device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None))
                .ok()
        };

        Self {
            smtc_manager: None,
            smtc_init_attempted: false,
            endpoint_volume,
            last_media_poll: Instant::now(),
            last_telemetry_poll: Instant::now(),
        }
    }

    pub fn refresh_all(&mut self, state: &mut AppState) {
        self.update_clock(state);
        self.update_power(state);
        self.update_network(state);
        self.update_volume(state, false);
        self.update_media(state);
        self.update_fullscreen_state(state);
    }

    pub fn poll_lightweight(&mut self, state: &mut AppState) -> bool {
        let mut changed = false;

        if self.update_volume(state, true) {
            changed = true;
        }

        if self.last_media_poll.elapsed().as_millis() >= 850 {
            self.last_media_poll = Instant::now();
            if self.update_media(state) {
                changed = true;
            }
            if self.update_fullscreen_state(state) {
                changed = true;
            }
        }

        if self.last_telemetry_poll.elapsed().as_secs() >= 5 {
            self.last_telemetry_poll = Instant::now();
            if self.update_clock(state) {
                changed = true;
            }
            if self.update_power(state) {
                changed = true;
            }
            if self.update_network(state) {
                changed = true;
            }
        }

        changed
    }

    pub fn update_clock(&self, state: &mut AppState) -> bool {
        let st: SYSTEMTIME = unsafe { GetLocalTime() };
        let formatted = format!("{:02}:{:02}", st.wHour, st.wMinute);
        if state.current_time_str != formatted {
            state.current_time_str = formatted;
            true
        } else {
            false
        }
    }

    pub fn update_power(&self, state: &mut AppState) -> bool {
        let mut sps = SYSTEM_POWER_STATUS::default();
        let ok = unsafe { GetSystemPowerStatus(&mut sps).is_ok() };
        let (pct, charging) = if ok {
            let pct = if sps.BatteryLifePercent <= 100 {
                sps.BatteryLifePercent as u32
            } else {
                100
            };
            let charging = sps.ACLineStatus == 1 || (sps.BatteryFlag & 8) != 0;
            (pct, charging)
        } else {
            (100, true)
        };

        if state.battery_percent != pct || state.is_charging != charging {
            state.battery_percent = pct;
            state.is_charging = charging;
            true
        } else {
            false
        }
    }

    pub fn update_network(&self, state: &mut AppState) -> bool {
        let (connected, ssid) = NetworkInformation::GetInternetConnectionProfile()
            .and_then(|profile| {
                let level = profile.GetNetworkConnectivityLevel()?;
                let is_conn = level != NetworkConnectivityLevel::None;
                let name = profile.ProfileName()?.to_string();
                Ok((
                    is_conn,
                    if name.is_empty() {
                        "Wi-Fi".to_string()
                    } else {
                        name
                    },
                ))
            })
            .unwrap_or((false, "Offline".to_string()));

        if state.network_connected != connected || state.wifi_ssid != ssid {
            state.network_connected = connected;
            state.wifi_ssid = ssid;
            true
        } else {
            false
        }
    }

    pub fn update_volume(&self, state: &mut AppState, trigger_osd: bool) -> bool {
        let Some(vol) = &self.endpoint_volume else {
            return false;
        };
        unsafe {
            let level = vol
                .GetMasterVolumeLevelScalar()
                .map(|v| (v * 100.0).round() as u32)
                .unwrap_or(state.volume_percent);
            let muted = vol
                .GetMute()
                .map(|b| b.as_bool())
                .unwrap_or(state.is_muted);

            if state.volume_percent != level || state.is_muted != muted {
                state.volume_percent = level;
                state.is_muted = muted;
                if trigger_osd {
                    state.last_volume_change = Some(Instant::now());
                    state.sync_spring_targets();
                }
                return true;
            }
        }
        false
    }

    pub fn update_media(&mut self, state: &mut AppState) -> bool {
        if !self.smtc_init_attempted {
            self.smtc_init_attempted = true;
            self.smtc_manager = std::thread::spawn(|| {
                GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                    .and_then(|op| op.get())
                    .ok()
            })
            .join()
            .ok()
            .flatten();
        }

        let Some(manager) = &self.smtc_manager else {
            return false;
        };

        let Ok(session) = manager.GetCurrentSession() else {
            if state.is_playing || !state.song_title.is_empty() {
                state.is_playing = false;
                state.song_title.clear();
                state.artist_name.clear();
                state.sync_spring_targets();
                return true;
            }
            return false;
        };

        let playing = session
            .GetPlaybackInfo()
            .and_then(|info| info.PlaybackStatus())
            .map(|status| status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
            .unwrap_or(false);

        // Read accurate playback position & duration from SMTC TimelineProperties
        if let Ok(timeline) = session.GetTimelineProperties() {
            if let Ok(pos) = timeline.Position() {
                let pos_secs = (pos.Duration as f64 / 10_000_000.0) as f32;
                if pos_secs >= 0.0 {
                    state.playback_position_secs = pos_secs;
                }
            }
            if let Ok(end) = timeline.EndTime() {
                let dur_secs = (end.Duration as f64 / 10_000_000.0) as f32;
                if dur_secs > 5.0 {
                    state.playback_duration_secs = dur_secs;
                }
            }
        }

        let session_clone = session.clone();
        let (title, artist) = std::thread::spawn(move || {
            session_clone
                .TryGetMediaPropertiesAsync()
                .and_then(|op| op.get())
                .map(|props| {
                    let t = props.Title().map(|s| s.to_string()).unwrap_or_default();
                    let a = props.Artist().map(|s| s.to_string()).unwrap_or_default();
                    (t, a)
                })
                .unwrap_or_default()
        })
        .join()
        .unwrap_or_default();

        let track_changed = state.song_title != title || state.artist_name != artist;
        let state_changed = state.is_playing != playing || track_changed;

        if state_changed {
            state.is_playing = playing;
            state.song_title = title.clone();
            state.artist_name = artist.clone();

            if track_changed && !title.trim().is_empty() {
                spawn_lyrics_fetch(title, artist, Arc::clone(&state.shared_lyrics));
            }
            state.update_active_lyric_from_timeline();
            state.sync_spring_targets();
            return true;
        }

        state.update_active_lyric_from_timeline();
        false
    }

    pub fn update_fullscreen_state(&self, state: &mut AppState) -> bool {
        if !state.config.auto_hide_fullscreen {
            if state.suppressed_by_fullscreen {
                state.suppressed_by_fullscreen = false;
                return true;
            }
            return false;
        }

        let fullscreen = unsafe {
            SHQueryUserNotificationState()
                .map(|q| {
                    q == QUNS_RUNNING_D3D_FULL_SCREEN
                        || q == QUNS_PRESENTATION_MODE
                        || q == QUNS_BUSY
                })
                .unwrap_or(false)
        };

        if state.suppressed_by_fullscreen != fullscreen {
            state.suppressed_by_fullscreen = fullscreen;
            true
        } else {
            false
        }
    }

    pub fn media_toggle_play_pause(&self) {
        if let Some(mgr) = &self.smtc_manager {
            if let Ok(session) = mgr.GetCurrentSession() {
                let _ = session.TryTogglePlayPauseAsync();
            }
        }
    }

    pub fn media_next(&self) {
        if let Some(mgr) = &self.smtc_manager {
            if let Ok(session) = mgr.GetCurrentSession() {
                let _ = session.TrySkipNextAsync();
            }
        }
    }

    pub fn media_prev(&self) {
        if let Some(mgr) = &self.smtc_manager {
            if let Ok(session) = mgr.GetCurrentSession() {
                let _ = session.TrySkipPreviousAsync();
            }
        }
    }
}

fn url_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() + 16);
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => encoded.push_str(&format!("%{:02X}", b)),
        }
    }
    encoded
}

/// Parses LRC timestamped lines like `[01:12.45] Lyric text` into sorted `LyricLine` structs.
pub fn parse_lrc_lines(lrc_raw: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw_line in lrc_raw.lines() {
        let trimmed = raw_line.trim();
        if !trimmed.starts_with('[') {
            continue;
        }
        let Some(close_bracket) = trimmed.find(']') else {
            continue;
        };
        let time_part = &trimmed[1..close_bracket];
        let text_part = trimmed[close_bracket + 1..].trim();
        if text_part.is_empty() {
            continue;
        }
        let mut parts = time_part.split(':');
        let Some(mins_str) = parts.next() else {
            continue;
        };
        let Some(secs_str) = parts.next() else {
            continue;
        };
        if let (Ok(mins), Ok(secs)) = (mins_str.parse::<f32>(), secs_str.parse::<f32>()) {
            lines.push(LyricLine {
                timestamp_secs: mins * 60.0 + secs,
                text: text_part.to_string(),
            });
        }
    }
    lines.sort_by(|a, b| {
        a.timestamp_secs
            .partial_cmp(&b.timestamp_secs)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    lines
}

/// Fetches synced LRC lyrics over HTTPS from `https://lrclib.net/api/search` in a background thread.
fn spawn_lyrics_fetch(title: String, artist: String, shared: Arc<Mutex<SharedLyricsBuffer>>) {
    let track_key = format!("{}::{}", title, artist);
    if let Ok(mut guard) = shared.lock() {
        if guard.track_key == track_key {
            return;
        }
        guard.track_key = track_key.clone();
        guard.lines.clear();
        guard.plain_fallback.clear();
        guard.loading = true;
    }

    std::thread::spawn(move || {
        let query = if artist.trim().is_empty() {
            title.clone()
        } else {
            format!("{} {}", title, artist)
        };
        let url_str = format!("https://lrclib.net/api/search?q={}", url_encode(&query));

        let fetched = (|| -> Option<(Vec<LyricLine>, String)> {
            let uri = Uri::CreateUri(&HSTRING::from(&url_str)).ok()?;
            let client = HttpClient::new().ok()?;
            let body = client
                .GetStringAsync(&uri)
                .and_then(|op| op.get())
                .ok()?
                .to_string();

            let parsed: serde_json::Value = serde_json::from_str(&body).ok()?;
            let arr = parsed.as_array()?;
            for item in arr {
                if let Some(synced) = item.get("syncedLyrics").and_then(|v| v.as_str()) {
                    let lines = parse_lrc_lines(synced);
                    if !lines.is_empty() {
                        return Some((lines, String::new()));
                    }
                }
                if let Some(plain) = item.get("plainLyrics").and_then(|v| v.as_str()) {
                    let first_non_empty = plain
                        .lines()
                        .map(|l| l.trim())
                        .find(|l| !l.is_empty())
                        .unwrap_or("")
                        .to_string();
                    if !first_non_empty.is_empty() {
                        return Some((Vec::new(), first_non_empty));
                    }
                }
            }
            None
        })();

        if let Ok(mut guard) = shared.lock() {
            if guard.track_key == track_key {
                guard.loading = false;
                if let Some((lines, plain)) = fetched {
                    guard.lines = lines;
                    guard.plain_fallback = plain;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc_parser_extracts_sorted_timestamps() {
        let sample = "[00:12.50] First lyric line\n[01:05.00] Second lyric line";
        let parsed = parse_lrc_lines(sample);
        assert_eq!(parsed.len(), 2);
        assert!((parsed[0].timestamp_secs - 12.5).abs() < 0.01);
        assert_eq!(parsed[0].text, "First lyric line");
        assert!((parsed[1].timestamp_secs - 65.0).abs() < 0.01);
    }
}
