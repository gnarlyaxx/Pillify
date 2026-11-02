// Pillify — Windows Native Services (SMTC Media, CoreAudio Volume, Battery, Wi-Fi, Fullscreen Detector)
// Designed for zero-overhead integration with Win32 & WinRT via `windows-rs`.

use crate::state::AppState;
use std::time::Instant;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Networking::Connectivity::{NetworkConnectivityLevel, NetworkInformation};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::Foundation::SYSTEMTIME;
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

        // Check volume changes rapidly (every ~250ms when woken) so hardware media keys trigger the Volume OSD
        if self.update_volume(state, true) {
            changed = true;
        }

        if self.last_media_poll.elapsed().as_millis() >= 900 {
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
                Ok((is_conn, if name.is_empty() { "Wi-Fi".to_string() } else { name }))
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
            // Initialize SMTC on a background MTA worker so we never deadlock the STA UI thread
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

        if state.is_playing != playing || state.song_title != title || state.artist_name != artist
        {
            let track_changed = state.song_title != title;
            state.is_playing = playing;
            state.song_title = title;
            state.artist_name = artist;
            if track_changed {
                state.synced_lyric_line = if state.song_title.is_empty() {
                    "Ready to play".to_string()
                } else if state.artist_name.is_empty() {
                    state.song_title.clone()
                } else {
                    format!("{} — {}", state.song_title, state.artist_name)
                };
            }
            state.sync_spring_targets();
            return true;
        }
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

