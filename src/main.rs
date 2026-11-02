#![cfg_attr(not(test), windows_subsystem = "windows")]

mod config;
mod renderer;
mod services;
mod spring;
mod state;

use crate::config::{IslandStyle, PillifyConfig};
use crate::renderer::{D2DRenderer, CANVAS_H, CANVAS_W};
use crate::services::SystemServices;
use crate::state::{ActivePage, AppState, DisplayMode, FocusMode, InteractiveControl};
use std::cell::RefCell;
use std::process::Command;
use std::time::Instant;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, TrackMouseEvent, UnregisterHotKey, MOD_ALT, MOD_CONTROL, TME_LEAVE,
    TRACKMOUSEEVENT,
};
use windows::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, Shell_NotifyIconW, HDROP, NIF_ICON, NIF_MESSAGE,
    NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetCursorPos, GetSystemMetrics, KillTimer, LoadCursorW, LoadIconW, PostQuitMessage,
    RegisterClassW, SetForegroundWindow, SetTimer, ShowWindow, TrackPopupMenu, TranslateMessage,
    WaitMessage, CS_HREDRAW, CS_VREDRAW, HTCLIENT, HTTRANSPARENT, IDC_HAND, IDI_APPLICATION,
    MF_SEPARATOR, MF_STRING, MSG, PM_REMOVE, PeekMessageW, SM_CXSCREEN, SW_SHOWNOACTIVATE,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, WINDOW_EX_STYLE, WM_COMMAND, WM_DESTROY, WM_DROPFILES,
    WM_HOTKEY, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCHITTEST,
    WM_RBUTTONUP, WM_TIMER, WM_USER, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};

const WM_MOUSELEAVE: u32 = 0x02A3;

const HOTKEY_ID_TOGGLE: i32 = 9001;
const TIMER_ID_PHYSICS: usize = 1;
const TIMER_ID_TELEMETRY: usize = 2;
const WM_TRAYICON: u32 = WM_USER + 42;

const MENU_TOGGLE_VISIBILITY: usize = 1001;
const MENU_OPEN_CUSTOMIZER: usize = 1002;
const MENU_SIMULATE_ALERT: usize = 1003;
const MENU_EXIT: usize = 1004;

struct AppContext {
    hwnd: HWND,
    window_x: i32,
    window_y: i32,
    state: AppState,
    renderer: D2DRenderer,
    services: SystemServices,
    last_frame: Instant,
    physics_timer_running: bool,
    tracking_mouse_leave: bool,
}

thread_local! {
    static APP_CTX: RefCell<Option<AppContext>> = const { RefCell::new(None) };
}

impl AppContext {
    fn ensure_physics_timer(&mut self) {
        if !self.physics_timer_running {
            unsafe {
                // 8ms (~120 FPS) high-cadence timer ONLY while springs are actively moving
                let _ = SetTimer(self.hwnd, TIMER_ID_PHYSICS, 8, None);
            }
            self.last_frame = Instant::now();
            self.physics_timer_running = true;
        }
    }

    fn stop_physics_timer_if_idle(&mut self) {
        if self.physics_timer_running && !self.state.is_music_active() {
            unsafe {
                let _ = KillTimer(self.hwnd, TIMER_ID_PHYSICS);
            }
            self.physics_timer_running = false;
        }
    }

    fn render(&mut self) {
        self.renderer
            .render_and_present(self.hwnd, &mut self.state, self.window_x, self.window_y);
    }

    fn handle_control_click(&mut self, ctrl: InteractiveControl) {
        match ctrl {
            InteractiveControl::IslandBody => match self.state.mode {
                DisplayMode::Default | DisplayMode::NotificationCompact => {
                    if self.state.focus_mode != FocusMode::Movie {
                        self.state.mode = DisplayMode::Expanded;
                    }
                }
                DisplayMode::Expanded => {}
            },
            InteractiveControl::CollapseButton | InteractiveControl::DismissToast => {
                self.state.mode = DisplayMode::Default;
            }
            InteractiveControl::TabButton(page) => {
                self.state.set_page(page);
            }
            InteractiveControl::MediaPrev => {
                self.services.media_prev();
            }
            InteractiveControl::MediaPlayPause => {
                self.services.media_toggle_play_pause();
                self.state.is_playing = !self.state.is_playing;
            }
            InteractiveControl::MediaNext => {
                self.services.media_next();
            }
            InteractiveControl::FocusChip(mode) => {
                self.state.focus_mode = mode;
            }
            InteractiveControl::ShelfOpenItem(idx) => {
                if let Some(entry) = self.state.shelf_items.get(idx) {
                    let _ = Command::new("explorer.exe").arg(&entry.full_path).spawn();
                }
            }
            InteractiveControl::ShelfRemoveItem(idx) => {
                if idx < self.state.shelf_items.len() {
                    self.state.shelf_items.remove(idx);
                    self.state.config.shelf_files = self
                        .state
                        .shelf_items
                        .iter()
                        .map(|i| i.full_path.clone())
                        .collect();
                    self.state.config.save();
                }
            }
            InteractiveControl::ShelfClearAll => {
                self.state.shelf_items.clear();
                self.state.config.shelf_files.clear();
                self.state.config.save();
            }
            InteractiveControl::PomodoroToggle => {
                self.state.timer_running = !self.state.timer_running;
                self.state.last_timer_tick = Instant::now();
            }
            InteractiveControl::PomodoroReset => {
                self.state.timer_running = false;
                self.state.timer_remaining_secs = self.state.timer_total_secs;
            }
            InteractiveControl::PomodoroPreset(mins) => {
                self.state.timer_running = false;
                self.state.timer_total_secs = mins * 60;
                self.state.timer_remaining_secs = mins * 60;
                self.state.timer_mode_label = match mins {
                    5 => "Short Break 5m".to_string(),
                    15 => "Long Break 15m".to_string(),
                    _ => "Focus 25m".to_string(),
                };
            }
            InteractiveControl::WaterAddGlass => {
                self.state.config.water_glasses_today =
                    (self.state.config.water_glasses_today + 1).min(30);
                self.state.config.save();
            }
            InteractiveControl::WaterReset => {
                self.state.config.water_glasses_today = 0;
                self.state.config.save();
            }
            InteractiveControl::TodoToggle(idx) => {
                if let Some(item) = self.state.config.todos.get_mut(idx) {
                    item.done = !item.done;
                    self.state.config.save();
                }
            }
            InteractiveControl::CustomizeStyleCycle => {
                self.state.config.style = match self.state.config.style {
                    IslandStyle::HybridMorph => IslandStyle::AlwaysNotch,
                    IslandStyle::AlwaysNotch => IslandStyle::AlwaysFloatingPill,
                    IslandStyle::AlwaysFloatingPill => IslandStyle::HybridMorph,
                };
                self.state.config.save();
            }
            InteractiveControl::CustomizeSpringCycle => {
                self.state.config.spring_preset = self.state.config.spring_preset.next();
                self.state.config.save();
            }
            InteractiveControl::CustomizeThemeCycle => {
                self.state.config.accent_theme = self.state.config.accent_theme.next();
                self.state.config.save();
            }
            InteractiveControl::CustomizeWidthMinus => {
                self.state.config.compact_width =
                    (self.state.config.compact_width - 12.0).max(180.0);
                self.state.config.save();
            }
            InteractiveControl::CustomizeWidthPlus => {
                self.state.config.compact_width =
                    (self.state.config.compact_width + 12.0).min(340.0);
                self.state.config.save();
            }
            InteractiveControl::CustomizeRadiusMinus => {
                self.state.config.corner_radius =
                    (self.state.config.corner_radius - 2.0).max(12.0);
                self.state.config.save();
            }
            InteractiveControl::CustomizeRadiusPlus => {
                self.state.config.corner_radius =
                    (self.state.config.corner_radius + 2.0).min(28.0);
                self.state.config.save();
            }
            InteractiveControl::CustomizeFlareMinus => {
                self.state.config.ear_flare_radius =
                    (self.state.config.ear_flare_radius - 2.0).max(6.0);
                self.state.config.save();
            }
            InteractiveControl::CustomizeFlarePlus => {
                self.state.config.ear_flare_radius =
                    (self.state.config.ear_flare_radius + 2.0).min(24.0);
                self.state.config.save();
            }
            InteractiveControl::CustomizeGlowMinus => {
                self.state.config.border_glow_opacity =
                    (self.state.config.border_glow_opacity - 0.05).max(0.05);
                self.state.config.save();
            }
            InteractiveControl::CustomizeGlowPlus => {
                self.state.config.border_glow_opacity =
                    (self.state.config.border_glow_opacity + 0.05).min(0.70);
                self.state.config.save();
            }
        }

        self.state.sync_spring_targets();
        self.ensure_physics_timer();
        self.render();
    }
}

fn main() -> windows::core::Result<()> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let hinstance = GetModuleHandleW(None)?;
        let class_name = w!("PillifyNativeIslandWindow");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_HAND)?,
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let window_x = (screen_w - CANVAS_W) / 2;
        let window_y = 0;

        let config = PillifyConfig::load();
        let mut state = AppState::new(config);
        let mut services = SystemServices::new();
        services.refresh_all(&mut state);
        state.sync_spring_targets();

        let renderer = D2DRenderer::new()?;
        APP_CTX.with(|cell| {
            *cell.borrow_mut() = Some(AppContext {
                hwnd: HWND::default(),
                window_x,
                window_y,
                state,
                renderer,
                services,
                last_frame: Instant::now(),
                physics_timer_running: false,
                tracking_mouse_leave: false,
            });
        });

        let ex_style: WINDOW_EX_STYLE = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
        let hwnd = CreateWindowExW(
            ex_style,
            class_name,
            w!("Pillify"),
            WS_POPUP,
            window_x,
            window_y,
            CANVAS_W,
            CANVAS_H,
            None,
            None,
            hinstance,
            None,
        )?;

        // Enable native Explorer File Drag & Drop onto Pillify
        DragAcceptFiles(hwnd, true);

        // Register global hotkey: Ctrl + Alt + I to toggle Pillify visibility
        let _ = RegisterHotKey(
            hwnd,
            HOTKEY_ID_TOGGLE,
            MOD_CONTROL | MOD_ALT,
            0x49, // 'I'
        );

        // Register System Tray icon
        add_tray_icon(hwnd);

        APP_CTX.with(|cell| {
            if let Some(ctx) = cell.borrow_mut().as_mut() {
                ctx.hwnd = hwnd;
                ctx.render();
            }
        });

        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        // Low-frequency telemetry timer (every 300ms, 0% measurable CPU)
        let _ = SetTimer(hwnd, TIMER_ID_TELEMETRY, 300, None);

        // Pure event-driven message loop (sleeps on WaitMessage when idle)
        let mut msg = MSG::default();
        loop {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_QUIT {
                    remove_tray_icon(hwnd);
                    let _ = UnregisterHotKey(hwnd, HOTKEY_ID_TOGGLE);
                    return Ok(());
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = WaitMessage();
        }
    }
}

unsafe fn add_tray_icon(hwnd: HWND) {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
        ..Default::default()
    };
    let tip = "Pillify — Interactive Dynamic Island (Ctrl+Alt+I)";
    for (i, ch) in tip.encode_utf16().take(nid.szTip.len() - 1).enumerate() {
        nid.szTip[i] = ch;
    }
    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    };
    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
}

unsafe fn show_tray_context_menu(hwnd: HWND) {
    let Ok(menu) = CreatePopupMenu() else {
        return;
    };
    let _ = AppendMenuW(
        menu,
        MF_STRING,
        MENU_TOGGLE_VISIBILITY,
        w!("Show / Hide Pillify (Ctrl+Alt+I)"),
    );
    let _ = AppendMenuW(
        menu,
        MF_STRING,
        MENU_OPEN_CUSTOMIZER,
        w!("Open Notch Customizer Studio"),
    );
    let _ = AppendMenuW(
        menu,
        MF_STRING,
        MENU_SIMULATE_ALERT,
        w!("Test Notification Morph"),
    );
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
    let _ = AppendMenuW(menu, MF_STRING, MENU_EXIT, w!("Exit Pillify"));

    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenu(
        menu,
        TPM_LEFTALIGN | TPM_BOTTOMALIGN,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    );
    let _ = DestroyMenu(menu);
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            // Smart Per-Shape Hit-Testing: Any pixel outside the active Pill/Notch returns HTTRANSPARENT
            // so mouse clicks pass straight through to underlying apps/browser tabs!
            let screen_x = (lparam.0 & 0xFFFF) as i16 as i32;
            let screen_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            let inside = APP_CTX.with(|cell| {
                if let Some(ctx) = cell.borrow().as_ref() {
                    if !ctx.state.visible || ctx.state.suppressed_by_fullscreen {
                        return false;
                    }
                    let local_x = (screen_x - ctx.window_x) as f32;
                    let local_y = (screen_y - ctx.window_y) as f32;
                    let (l, t, r, b) = D2DRenderer::island_bounds(&ctx.state);
                    local_x >= l - 6.0 && local_x <= r + 6.0 && local_y >= t && local_y <= b + 6.0
                } else {
                    false
                }
            });

            if inside {
                LRESULT(HTCLIENT as isize)
            } else {
                LRESULT(HTTRANSPARENT as isize)
            }
        }
        WM_MOUSEMOVE => {
            let local_x = (lparam.0 & 0xFFFF) as i16 as f32;
            let local_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;

            APP_CTX.with(|cell| {
                if let Some(ctx) = cell.borrow_mut().as_mut() {
                    if !ctx.tracking_mouse_leave {
                        let mut tme = TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        let _ = TrackMouseEvent(&mut tme);
                        ctx.tracking_mouse_leave = true;
                    }

                    ctx.state.cursor_pos = (local_x, local_y);
                    let was_hovered = ctx.state.is_hovered;
                    ctx.state.is_hovered = true;

                    // Find the most specific hit zone under the cursor (excluding the fallback IslandBody)
                    let new_ctrl = ctx
                        .state
                        .hit_zones
                        .iter()
                        .find(|z| {
                            z.control != InteractiveControl::IslandBody
                                && z.contains(local_x, local_y)
                        })
                        .map(|z| z.control);

                    if !was_hovered || ctx.state.hovered_control != new_ctrl {
                        ctx.state.hovered_control = new_ctrl;
                        ctx.state.sync_spring_targets();
                        ctx.ensure_physics_timer();
                        ctx.render();
                    }
                }
            });
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            APP_CTX.with(|cell| {
                if let Some(ctx) = cell.borrow_mut().as_mut() {
                    ctx.tracking_mouse_leave = false;
                    ctx.state.is_hovered = false;
                    ctx.state.hovered_control = None;
                    ctx.state.sync_spring_targets();
                    ctx.ensure_physics_timer();
                    ctx.render();
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let local_x = (lparam.0 & 0xFFFF) as i16 as f32;
            let local_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;

            APP_CTX.with(|cell| {
                if let Some(ctx) = cell.borrow_mut().as_mut() {
                    let hit = ctx
                        .state
                        .hit_zones
                        .iter()
                        .find(|z| z.contains(local_x, local_y))
                        .map(|z| z.control);
                    if let Some(ctrl) = hit {
                        ctx.handle_control_click(ctrl);
                    }
                }
            });
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            // Right-clicking the island collapses it if expanded, or shows the quick context menu
            let handled_collapse = APP_CTX.with(|cell| {
                if let Some(ctx) = cell.borrow_mut().as_mut() {
                    if ctx.state.mode == DisplayMode::Expanded {
                        ctx.state.mode = DisplayMode::Default;
                        ctx.state.sync_spring_targets();
                        ctx.ensure_physics_timer();
                        ctx.render();
                        return true;
                    }
                }
                false
            });
            if !handled_collapse {
                show_tray_context_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            APP_CTX.with(|cell| {
                if let Some(ctx) = cell.borrow_mut().as_mut() {
                    if ctx.state.mode == DisplayMode::Expanded {
                        if delta < 0 {
                            ctx.state.cycle_page(1);
                        } else if delta > 0 {
                            ctx.state.cycle_page(-1);
                        }
                        ctx.ensure_physics_timer();
                        ctx.render();
                    }
                }
            });
            LRESULT(0)
        }
        WM_DROPFILES => {
            let hdrop = HDROP(wparam.0 as *mut core::ffi::c_void);
            let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
            let mut dropped_paths = Vec::new();
            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, None);
                if len > 0 {
                    let mut buf = vec![0u16; (len + 1) as usize];
                    DragQueryFileW(hdrop, i, Some(&mut buf));
                    if let Some(null_pos) = buf.iter().position(|&c| c == 0) {
                        buf.truncate(null_pos);
                    }
                    dropped_paths.push(String::from_utf16_lossy(&buf));
                }
            }
            DragFinish(hdrop);

            if !dropped_paths.is_empty() {
                APP_CTX.with(|cell| {
                    if let Some(ctx) = cell.borrow_mut().as_mut() {
                        ctx.state.add_dropped_files(dropped_paths);
                        ctx.ensure_physics_timer();
                        ctx.render();
                    }
                });
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            if wparam.0 as i32 == HOTKEY_ID_TOGGLE {
                APP_CTX.with(|cell| {
                    if let Some(ctx) = cell.borrow_mut().as_mut() {
                        ctx.state.visible = !ctx.state.visible;
                        if ctx.state.visible {
                            ctx.state.mode = DisplayMode::Default;
                            ctx.state.sync_spring_targets();
                            ctx.ensure_physics_timer();
                        }
                        ctx.render();
                    }
                });
            }
            LRESULT(0)
        }
        WM_TRAYICON => {
            let mouse_msg = (lparam.0 & 0xFFFF) as u32;
            if mouse_msg == WM_RBUTTONUP || mouse_msg == WM_LBUTTONUP {
                show_tray_context_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd_id = wparam.0 & 0xFFFF;
            match cmd_id {
                MENU_TOGGLE_VISIBILITY => {
                    APP_CTX.with(|cell| {
                        if let Some(ctx) = cell.borrow_mut().as_mut() {
                            ctx.state.visible = !ctx.state.visible;
                            ctx.render();
                        }
                    });
                }
                MENU_OPEN_CUSTOMIZER => {
                    APP_CTX.with(|cell| {
                        if let Some(ctx) = cell.borrow_mut().as_mut() {
                            ctx.state.visible = true;
                            ctx.state.mode = DisplayMode::Expanded;
                            ctx.state.set_page(ActivePage::Customize);
                            ctx.ensure_physics_timer();
                            ctx.render();
                        }
                    });
                }
                MENU_SIMULATE_ALERT => {
                    APP_CTX.with(|cell| {
                        if let Some(ctx) = cell.borrow_mut().as_mut() {
                            ctx.state.visible = true;
                            ctx.state.trigger_notification(
                                "Pillify",
                                "Interactive Notch Ready ✨",
                                "Click the island to morph open, scroll to switch tabs!",
                            );
                            ctx.ensure_physics_timer();
                            ctx.render();
                        }
                    });
                }
                MENU_EXIT => {
                    PostQuitMessage(0);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_TIMER => {
            let timer_id = wparam.0;
            APP_CTX.with(|cell| {
                if let Some(ctx) = cell.borrow_mut().as_mut() {
                    if timer_id == TIMER_ID_PHYSICS {
                        let now = Instant::now();
                        let dt = (now - ctx.last_frame).as_secs_f32().clamp(0.001, 0.05);
                        ctx.last_frame = now;

                        let springs_moving = ctx.state.animator.step_all(dt);
                        let periodic_dirty = ctx.state.tick_periodic(dt);

                        ctx.render();

                        if !springs_moving && !periodic_dirty {
                            ctx.stop_physics_timer_if_idle();
                        }
                    } else if timer_id == TIMER_ID_TELEMETRY {
                        let mut dirty = ctx.services.poll_lightweight(&mut ctx.state);
                        if !ctx.physics_timer_running {
                            dirty |= ctx.state.tick_periodic(0.3);
                        }
                        if dirty {
                            ctx.ensure_physics_timer();
                            ctx.render();
                        }
                    }
                }
            });
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

