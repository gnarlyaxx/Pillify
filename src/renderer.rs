// Pillify — Direct2D & DirectWrite Hardware-Accelerated Renderer
// Renders true mathematical Bezier Notch morphing, audio spectrum bars, File Shelf, Pomodoro/Water, and Notch Customizer
// into a 32-bit premultiplied-alpha DIB section updated via `UpdateLayeredWindow` at monitor refresh rate (0% CPU when idle).

use crate::config::IslandStyle;
use crate::state::{ActivePage, AppState, FocusMode, HitZone, InteractiveControl};
use std::mem::size_of;
use windows::core::{w, Interface};
use windows::Win32::Foundation::{HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_END_CLOSED, D2D1_PIXEL_FORMAT, D2D_POINT_2F, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory, ID2D1PathGeometry, ID2D1RenderTarget,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS,
    HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

pub const CANVAS_W: i32 = 600;
pub const CANVAS_H: i32 = 300;

pub struct D2DRenderer {
    d2d_factory: ID2D1Factory,
    dc_target: ID2D1RenderTarget,
    mem_dc: HDC,
    dib_bitmap: HBITMAP,
    old_obj: HGDIOBJ,
    font_bold_13: IDWriteTextFormat,
    font_semibold_12: IDWriteTextFormat,
    font_medium_11: IDWriteTextFormat,
    font_regular_10: IDWriteTextFormat,
    font_timer_24: IDWriteTextFormat,
}

fn color(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

impl D2DRenderer {
    pub fn new() -> windows::core::Result<Self> {
        unsafe {
            let d2d_factory: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;

            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: Default::default(),
            };

            let dcr: ID2D1DCRenderTarget = d2d_factory.CreateDCRenderTarget(&props)?;

            let screen_dc = GetDC(HWND::default());
            let mem_dc = CreateCompatibleDC(screen_dc);
            ReleaseDC(HWND::default(), screen_dc);

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: CANVAS_W,
                    biHeight: -CANVAS_H, // top-down DIB
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let dib_bitmap = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)?;
            let old_obj = SelectObject(mem_dc, dib_bitmap);

            let bind_rect = RECT {
                left: 0,
                top: 0,
                right: CANVAS_W,
                bottom: CANVAS_H,
            };
            dcr.BindDC(mem_dc, &bind_rect)?;
            let dc_target: ID2D1RenderTarget = dcr.cast()?;

            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

            let make_fmt = |weight, size: f32| -> windows::core::Result<IDWriteTextFormat> {
                let fmt = dwrite.CreateTextFormat(
                    w!("Segoe UI Variable Display"),
                    None,
                    weight,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("en-us"),
                )?;
                fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                fmt.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                Ok(fmt)
            };

            let font_bold_13 = make_fmt(DWRITE_FONT_WEIGHT_BOLD, 13.0)?;
            let font_semibold_12 = make_fmt(DWRITE_FONT_WEIGHT_SEMI_BOLD, 11.5)?;
            let font_medium_11 = make_fmt(DWRITE_FONT_WEIGHT_MEDIUM, 11.0)?;
            let font_regular_10 = make_fmt(DWRITE_FONT_WEIGHT_REGULAR, 10.0)?;
            let font_timer_24 = make_fmt(DWRITE_FONT_WEIGHT_BOLD, 22.0)?;

            Ok(Self {
                d2d_factory,
                dc_target,
                mem_dc,
                dib_bitmap,
                old_obj,
                font_bold_13,
                font_semibold_12,
                font_medium_11,
                font_regular_10,
                font_timer_24,
            })
        }
    }

    /// Builds a smooth Bezier path that morphs continuously between a floating rounded capsule (`flare = 0.0`)
    /// and a MacBook/NUSA concave top-bezel notch (`flare = 1.0`).
    unsafe fn build_island_geometry(
        &self,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        corner_r: f32,
        ear_r: f32,
        flare: f32,
    ) -> windows::core::Result<ID2D1PathGeometry> {
        let path = self.d2d_factory.CreatePathGeometry()?;
        let sink = path.Open()?;

        let w = (right - left).max(40.0);
        let h = (bottom - top).max(24.0);
        let r = corner_r.min(w * 0.45).min(h * 0.48);
        let flare_clamped = flare.clamp(0.0, 1.0);
        // Top ear flare extends outward at the top edge when docked to the bezel
        let ear = ear_r * flare_clamped;
        let top_r = r * (1.0 - flare_clamped);
        let k = 0.55228475; // cubic Bezier circle approximation constant

        // Start at top-left (including outward ear flare when flare > 0)
        sink.BeginFigure(
            D2D_POINT_2F {
                x: left - ear + top_r,
                y: top,
            },
            D2D1_FIGURE_BEGIN_FILLED,
        );

        // Top edge to top-right
        sink.AddLine(D2D_POINT_2F {
            x: right + ear - top_r,
            y: top,
        });

        if flare_clamped > 0.02 {
            // Concave top-right ear curving inward into right vertical wall
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F {
                    x: right + ear * (1.0 - k),
                    y: top,
                },
                point2: D2D_POINT_2F {
                    x: right,
                    y: top + ear * (1.0 - k),
                },
                point3: D2D_POINT_2F {
                    x: right,
                    y: top + ear,
                },
            });
        } else if top_r > 0.5 {
            // Standard convex rounded top-right corner
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F {
                    x: right - top_r * (1.0 - k),
                    y: top,
                },
                point2: D2D_POINT_2F {
                    x: right,
                    y: top + top_r * (1.0 - k),
                },
                point3: D2D_POINT_2F {
                    x: right,
                    y: top + top_r,
                },
            });
        }

        // Right vertical edge down to bottom-right corner
        sink.AddLine(D2D_POINT_2F {
            x: right,
            y: bottom - r,
        });

        // Bottom-right rounded corner
        sink.AddBezier(&D2D1_BEZIER_SEGMENT {
            point1: D2D_POINT_2F {
                x: right,
                y: bottom - r * (1.0 - k),
            },
            point2: D2D_POINT_2F {
                x: right - r * (1.0 - k),
                y: bottom,
            },
            point3: D2D_POINT_2F {
                x: right - r,
                y: bottom,
            },
        });

        // Bottom horizontal edge
        sink.AddLine(D2D_POINT_2F {
            x: left + r,
            y: bottom,
        });

        // Bottom-left rounded corner
        sink.AddBezier(&D2D1_BEZIER_SEGMENT {
            point1: D2D_POINT_2F {
                x: left + r * (1.0 - k),
                y: bottom,
            },
            point2: D2D_POINT_2F {
                x: left,
                y: bottom - r * (1.0 - k),
            },
            point3: D2D_POINT_2F {
                x: left,
                y: bottom - r,
            },
        });

        // Left vertical edge up to top-left corner/ear
        let left_top_y = if flare_clamped > 0.02 {
            top + ear
        } else {
            top + top_r
        };
        sink.AddLine(D2D_POINT_2F {
            x: left,
            y: left_top_y,
        });

        if flare_clamped > 0.02 {
            // Concave top-left ear curving outward to top bezel
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F {
                    x: left,
                    y: top + ear * (1.0 - k),
                },
                point2: D2D_POINT_2F {
                    x: left - ear * (1.0 - k),
                    y: top,
                },
                point3: D2D_POINT_2F {
                    x: left - ear,
                    y: top,
                },
            });
        } else if top_r > 0.5 {
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: D2D_POINT_2F {
                    x: left,
                    y: top + top_r * (1.0 - k),
                },
                point2: D2D_POINT_2F {
                    x: left + top_r * (1.0 - k),
                    y: top,
                },
                point3: D2D_POINT_2F {
                    x: left + top_r,
                    y: top,
                },
            });
        }

        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        Ok(path)
    }

    pub fn render_and_present(&self, hwnd: HWND, state: &mut AppState, window_x: i32, window_y: i32) {
        unsafe {
            state.hit_zones.clear();

            self.dc_target.BeginDraw();
            self.dc_target.Clear(Some(&color(0.0, 0.0, 0.0, 0.0)));

            if state.visible && !state.suppressed_by_fullscreen {
                self.draw_island(state);
            }

            let _ = self.dc_target.EndDraw(None, None);

            let pt_dst = POINT {
                x: window_x,
                y: window_y,
            };
            let size = SIZE {
                cx: CANVAS_W,
                cy: CANVAS_H,
            };
            let pt_src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            let screen_dc = GetDC(HWND::default());
            let _ = UpdateLayeredWindow(
                hwnd,
                screen_dc,
                Some(&pt_dst),
                Some(&size),
                self.mem_dc,
                Some(&pt_src),
                windows::Win32::Foundation::COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
            ReleaseDC(HWND::default(), screen_dc);
        }
    }

    /// Computes exact bounding rectangle of the active island capsule for mouse hit-testing (`WM_NCHITTEST`).
    pub fn island_bounds(state: &AppState) -> (f32, f32, f32, f32) {
        let scale = state.animator.hover_scale.value;
        let w = state.animator.width.value * scale;
        let h = state.animator.height.value * scale;
        let top = state.animator.top_offset.value;
        let center_x = (CANVAS_W as f32) * 0.5;
        let left = center_x - w * 0.5;
        let right = center_x + w * 0.5;
        let bottom = top + h;
        (left, top, right, bottom)
    }

    unsafe fn draw_island(&self, state: &mut AppState) {
        let (left, top, right, bottom) = Self::island_bounds(state);
        let corner_r = state.animator.corner_radius.value;
        let ear_r = state.config.ear_flare_radius;
        let flare = state.animator.notch_flare.value;

        let ((acc_r, acc_g, acc_b), (sec_r, sec_g, sec_b)) = state.config.accent_theme.colors();

        let Ok(geo) =
            self.build_island_geometry(left, top, right, bottom, corner_r, ear_r, flare)
        else {
            return;
        };

        // Deep obsidian background brush (#0B0C10)
        let Ok(bg_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(0.043, 0.047, 0.063, 0.97), None)
        else {
            return;
        };
        self.dc_target.FillGeometry(&geo, &bg_brush, None);

        // Outer subtle glass border + Accent Glow when hovered or dragging files
        let glow_alpha = if state.is_drag_hovering {
            0.85
        } else if state.is_hovered {
            (state.config.border_glow_opacity + 0.16).min(0.85)
        } else {
            state.config.border_glow_opacity
        };
        if let Ok(border_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(acc_r, acc_g, acc_b, glow_alpha), None)
        {
            self.dc_target
                .DrawGeometry(&geo, &border_brush, if state.is_hovered { 1.4 } else { 1.0 }, None);
        }

        let expand = state.animator.expand_progress.value.clamp(0.0, 1.0);
        let osd_blend = state.animator.osd_blend.value.clamp(0.0, 1.0);
        let alert_blend = state.animator.compact_alert_blend.value.clamp(0.0, 1.0);

        // Draw Compact Bar when expand_progress < 0.75
        if expand < 0.75 {
            let compact_alpha = ((0.75 - expand) / 0.75).clamp(0.0, 1.0);
            if osd_blend > 0.2 {
                self.draw_volume_osd(
                    state,
                    left,
                    top,
                    right,
                    bottom,
                    compact_alpha * osd_blend,
                    (acc_r, acc_g, acc_b),
                );
            } else if alert_blend > 0.35 {
                self.draw_compact_notification(
                    state,
                    left,
                    top,
                    right,
                    bottom,
                    compact_alpha * alert_blend,
                    (acc_r, acc_g, acc_b),
                );
            } else {
                self.draw_compact_default(
                    state,
                    left,
                    top,
                    right,
                    bottom,
                    compact_alpha * (1.0 - alert_blend),
                    (acc_r, acc_g, acc_b),
                    (sec_r, sec_g, sec_b),
                );
            }
        }

        // Draw Expanded Multi-Page Dashboard when expand_progress > 0.25
        if expand > 0.25 {
            let exp_alpha = ((expand - 0.25) / 0.75).clamp(0.0, 1.0);
            self.draw_expanded_dashboard(
                state,
                left,
                top,
                right,
                bottom,
                exp_alpha,
                (acc_r, acc_g, acc_b),
                (sec_r, sec_g, sec_b),
            );
        }

        // Register fallback click zone on the island body itself
        state.hit_zones.push(HitZone {
            left,
            top,
            right,
            bottom,
            control: InteractiveControl::IslandBody,
        });
    }

    unsafe fn draw_compact_default(
        &self,
        state: &AppState,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        alpha: f32,
        accent: (f32, f32, f32),
        secondary: (f32, f32, f32),
    ) {
        let mid_y = (top + bottom) * 0.5;
        let pad_x = 16.0;

        // Left: Current Time (or Pomodoro countdown if running)
        let left_label = if state.timer_running {
            format!("⏱ {}", state.timer_formatted())
        } else {
            state.current_time_str.clone()
        };
        self.draw_text(
            &left_label,
            &self.font_bold_13,
            D2D_RECT_F {
                left: left + pad_x,
                top,
                right: left + 100.0,
                bottom,
            },
            color(0.98, 0.98, 1.0, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        // Center: Animated 6-bar Audio Equalizer + Song Title when playing music
        if state.is_music_active() {
            let spec_x = left + 70.0;
            if let Ok(bar_brush) = self
                .dc_target
                .CreateSolidColorBrush(&color(accent.0, accent.1, accent.2, alpha), None)
            {
                for (i, &amp) in state.audio_spectrum.iter().enumerate() {
                    let bx = spec_x + (i as f32) * 4.5;
                    let bh = (4.0 + amp * 14.0).clamp(4.0, 18.0);
                    let rrect = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: bx,
                            top: mid_y - bh * 0.5,
                            right: bx + 2.8,
                            bottom: mid_y + bh * 0.5,
                        },
                        radiusX: 1.4,
                        radiusY: 1.4,
                    };
                    self.dc_target.FillRoundedRectangle(&rrect, &bar_brush);
                }
            }

            let song_label = if state.artist_name.is_empty() {
                state.song_title.clone()
            } else {
                format!("{} • {}", state.song_title, state.artist_name)
            };
            let chars: String = song_label.chars().take(24).collect();
            self.draw_text(
                &chars,
                &self.font_semibold_12,
                D2D_RECT_F {
                    left: spec_x + 32.0,
                    top,
                    right: right - 82.0,
                    bottom,
                },
                color(accent.0, accent.1, accent.2, alpha),
                DWRITE_TEXT_ALIGNMENT_LEADING,
            );
        }

        // Right: Wi-Fi status dot + Battery pill & percentage
        let wifi_dot_color = if state.network_connected {
            color(accent.0, accent.1, accent.2, alpha)
        } else {
            color(1.0, 0.35, 0.35, alpha)
        };
        if let Ok(dot_brush) = self.dc_target.CreateSolidColorBrush(&wifi_dot_color, None) {
            let rrect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: right - 68.0,
                    top: mid_y - 3.5,
                    right: right - 61.0,
                    bottom: mid_y + 3.5,
                },
                radiusX: 3.5,
                radiusY: 3.5,
            };
            self.dc_target.FillRoundedRectangle(&rrect, &dot_brush);
        }

        let bat_label = if state.is_charging {
            format!("⚡{}%", state.battery_percent)
        } else {
            format!("{}%", state.battery_percent)
        };
        self.draw_text(
            &bat_label,
            &self.font_semibold_12,
            D2D_RECT_F {
                left: right - 58.0,
                top,
                right: right - pad_x,
                bottom,
            },
            color(secondary.0, secondary.1, secondary.2, alpha),
            DWRITE_TEXT_ALIGNMENT_TRAILING,
        );
    }

    unsafe fn draw_volume_osd(
        &self,
        state: &AppState,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        alpha: f32,
        accent: (f32, f32, f32),
    ) {
        let mid_y = (top + bottom) * 0.5;
        let label = if state.is_muted {
            "🔇 Muted".to_string()
        } else {
            format!("🔊 {}%", state.volume_percent)
        };
        self.draw_text(
            &label,
            &self.font_bold_13,
            D2D_RECT_F {
                left: left + 16.0,
                top,
                right: left + 95.0,
                bottom,
            },
            color(1.0, 1.0, 1.0, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let bar_l = left + 98.0;
        let bar_r = right - 18.0;
        let bar_w = (bar_r - bar_l).max(20.0);

        if let Ok(track_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.16 * alpha), None)
        {
            self.dc_target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: bar_l,
                        top: mid_y - 3.5,
                        right: bar_r,
                        bottom: mid_y + 3.5,
                    },
                    radiusX: 3.5,
                    radiusY: 3.5,
                },
                &track_brush,
            );
        }

        let fill_ratio = if state.is_muted {
            0.0
        } else {
            (state.volume_percent as f32 / 100.0).clamp(0.0, 1.0)
        };
        if fill_ratio > 0.01 {
            if let Ok(fill_brush) = self
                .dc_target
                .CreateSolidColorBrush(&color(accent.0, accent.1, accent.2, alpha), None)
            {
                self.dc_target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: bar_l,
                            top: mid_y - 3.5,
                            right: bar_l + bar_w * fill_ratio,
                            bottom: mid_y + 3.5,
                        },
                        radiusX: 3.5,
                        radiusY: 3.5,
                    },
                    &fill_brush,
                );
            }
        }
    }

    unsafe fn draw_compact_notification(
        &self,
        state: &AppState,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        alpha: f32,
        accent: (f32, f32, f32),
    ) {
        let mid_y = (top + bottom) * 0.5;
        if let Ok(badge_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(accent.0, accent.1, accent.2, alpha), None)
        {
            self.dc_target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: left + 14.0,
                        top: mid_y - 9.0,
                        right: left + 32.0,
                        bottom: mid_y + 9.0,
                    },
                    radiusX: 9.0,
                    radiusY: 9.0,
                },
                &badge_brush,
            );
        }

        let summary = format!("{} • {}", state.toast_title, state.toast_app);
        self.draw_text(
            &summary,
            &self.font_bold_13,
            D2D_RECT_F {
                left: left + 40.0,
                top,
                right: right - 18.0,
                bottom,
            },
            color(1.0, 1.0, 1.0, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );
    }

    unsafe fn draw_expanded_dashboard(
        &self,
        state: &mut AppState,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        alpha: f32,
        accent: (f32, f32, f32),
        secondary: (f32, f32, f32),
    ) {
        let pad_x = 20.0;
        let header_top = top + 12.0;
        let header_bottom = header_top + 26.0;

        // Top Navigation Pill Bar: [🎵 Media] [📥 Shelf] [⏱ Focus] [✨ Notch]   [✕]
        let tabs = [
            (ActivePage::Media, "🎵 Media", 74.0),
            (ActivePage::Shelf, "📥 Shelf", 72.0),
            (ActivePage::Productivity, "⏱ Focus", 72.0),
            (ActivePage::Customize, "✨ Notch", 76.0),
        ];

        let mut cur_x = left + pad_x;
        for (tab_page, label, tab_w) in tabs {
            let active = state.page == tab_page;
            let ctrl = InteractiveControl::TabButton(tab_page);
            let hovered = state.hovered_control == Some(ctrl);
            let rect = D2D_RECT_F {
                left: cur_x,
                top: header_top,
                right: cur_x + tab_w,
                bottom: header_bottom,
            };
            self.draw_pill_button(rect, label, active, hovered, alpha, accent);
            state.hit_zones.push(HitZone {
                left: rect.left,
                top: rect.top,
                right: rect.right,
                bottom: rect.bottom,
                control: ctrl,
            });
            cur_x += tab_w + 6.0;
        }

        // Collapse button [✕] on top-right
        let close_rect = D2D_RECT_F {
            left: right - pad_x - 28.0,
            top: header_top,
            right: right - pad_x,
            bottom: header_bottom,
        };
        let close_hovered = state.hovered_control == Some(InteractiveControl::CollapseButton);
        self.draw_pill_button(close_rect, "✕", false, close_hovered, alpha, accent);
        state.hit_zones.push(HitZone {
            left: close_rect.left,
            top: close_rect.top,
            right: close_rect.right,
            bottom: close_rect.bottom,
            control: InteractiveControl::CollapseButton,
        });

        let content_top = header_bottom + 10.0;
        let content_bottom = bottom - 22.0;

        match state.page {
            ActivePage::Media => {
                self.draw_page_media(
                    state,
                    left + pad_x,
                    content_top,
                    right - pad_x,
                    content_bottom,
                    alpha,
                    accent,
                    secondary,
                );
            }
            ActivePage::Shelf => {
                self.draw_page_shelf(
                    state,
                    left + pad_x,
                    content_top,
                    right - pad_x,
                    content_bottom,
                    alpha,
                    accent,
                );
            }
            ActivePage::Productivity => {
                self.draw_page_productivity(
                    state,
                    left + pad_x,
                    content_top,
                    right - pad_x,
                    content_bottom,
                    alpha,
                    accent,
                    secondary,
                );
            }
            ActivePage::Customize => {
                self.draw_page_customize(
                    state,
                    left + pad_x,
                    content_top,
                    right - pad_x,
                    content_bottom,
                    alpha,
                    accent,
                );
            }
        }

        // Bottom Pagination Indicator Dots
        let center_x = (left + right) * 0.5;
        let dot_y = bottom - 11.0;
        for i in 0..4 {
            let active = state.page.index() == i;
            let dx = center_x - 21.0 + (i as f32) * 14.0;
            let dot_w = if active { 10.0 } else { 5.0 };
            let dot_c = if active {
                color(accent.0, accent.1, accent.2, alpha)
            } else {
                color(1.0, 1.0, 1.0, 0.25 * alpha)
            };
            if let Ok(b) = self.dc_target.CreateSolidColorBrush(&dot_c, None) {
                self.dc_target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: dx - dot_w * 0.5,
                            top: dot_y - 2.5,
                            right: dx + dot_w * 0.5,
                            bottom: dot_y + 2.5,
                        },
                        radiusX: 2.5,
                        radiusY: 2.5,
                    },
                    &b,
                );
            }
        }
    }

    unsafe fn draw_page_media(
        &self,
        state: &mut AppState,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        alpha: f32,
        accent: (f32, f32, f32),
        secondary: (f32, f32, f32),
    ) {
        // Album art / Visualizer card on the left
        let art_rect = D2D_RECT_F {
            left,
            top,
            right: left + 52.0,
            bottom: top + 52.0,
        };
        if let Ok(card_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(accent.0, accent.1, accent.2, 0.16 * alpha), None)
        {
            self.dc_target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: art_rect,
                    radiusX: 12.0,
                    radiusY: 12.0,
                },
                &card_brush,
            );
        }

        // Animated spectrum inside the album tile
        let mid_art_y = top + 26.0;
        if let Ok(spec_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(accent.0, accent.1, accent.2, alpha), None)
        {
            for (i, &amp) in state.audio_spectrum.iter().enumerate() {
                let bx = left + 10.0 + (i as f32) * 5.6;
                let bh = if state.is_playing {
                    6.0 + amp * 22.0
                } else {
                    6.0
                };
                self.dc_target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: bx,
                            top: mid_art_y - bh * 0.5,
                            right: bx + 3.4,
                            bottom: mid_art_y + bh * 0.5,
                        },
                        radiusX: 1.7,
                        radiusY: 1.7,
                    },
                    &spec_brush,
                );
            }
        }

        // Song Title & Artist
        let title_str = if state.song_title.is_empty() {
            "No Active Media Session"
        } else {
            &state.song_title
        };
        let artist_str = if state.artist_name.is_empty() {
            "Spotify • YouTube • System Audio"
        } else {
            &state.artist_name
        };

        self.draw_text(
            title_str,
            &self.font_bold_13,
            D2D_RECT_F {
                left: left + 64.0,
                top: top + 4.0,
                right: right - 118.0,
                bottom: top + 26.0,
            },
            color(1.0, 1.0, 1.0, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        self.draw_text(
            artist_str,
            &self.font_medium_11,
            D2D_RECT_F {
                left: left + 64.0,
                top: top + 26.0,
                right: right - 118.0,
                bottom: top + 48.0,
            },
            color(0.68, 0.70, 0.76, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        // Transport Buttons: [⏮] [⏯] [⏭]
        let btn_y1 = top + 11.0;
        let btn_y2 = top + 41.0;
        let transport = [
            (InteractiveControl::MediaPrev, "⏮", right - 106.0, right - 74.0),
            (
                InteractiveControl::MediaPlayPause,
                if state.is_playing { "⏸" } else { "▶" },
                right - 70.0,
                right - 36.0,
            ),
            (InteractiveControl::MediaNext, "⏭", right - 32.0, right),
        ];
        for (ctrl, icon, x1, x2) in transport {
            let r = D2D_RECT_F {
                left: x1,
                top: btn_y1,
                right: x2,
                bottom: btn_y2,
            };
            let hov = state.hovered_control == Some(ctrl);
            let primary = ctrl == InteractiveControl::MediaPlayPause && state.is_playing;
            self.draw_pill_button(r, icon, primary, hov, alpha, accent);
            state.hit_zones.push(HitZone {
                left: r.left,
                top: r.top,
                right: r.right,
                bottom: r.bottom,
                control: ctrl,
            });
        }

        // Synced Lyric / Live Audio Banner
        let lyric_rect = D2D_RECT_F {
            left,
            top: top + 60.0,
            right,
            bottom: top + 86.0,
        };
        if let Ok(l_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.06 * alpha), None)
        {
            self.dc_target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: lyric_rect,
                    radiusX: 8.0,
                    radiusY: 8.0,
                },
                &l_brush,
            );
        }
        self.draw_text(
            &state.synced_lyric_line,
            &self.font_medium_11,
            D2D_RECT_F {
                left: left + 10.0,
                top: lyric_rect.top,
                right: right - 10.0,
                bottom: lyric_rect.bottom,
            },
            color(accent.0, accent.1, accent.2, alpha),
            DWRITE_TEXT_ALIGNMENT_CENTER,
        );

        // Bottom Row: Telemetry + Focus Mode Chips
        let row_y1 = bottom - 28.0;
        let row_y2 = bottom - 2.0;
        let net_bat = format!(
            "📶 {}   •   ⚡ {}%   •   🔊 {}%",
            state.wifi_ssid, state.battery_percent, state.volume_percent
        );
        self.draw_text(
            &net_bat,
            &self.font_regular_10,
            D2D_RECT_F {
                left,
                top: row_y1,
                right: right - 165.0,
                bottom: row_y2,
            },
            color(secondary.0, secondary.1, secondary.2, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let chips = [
            (FocusMode::None, "Normal", right - 160.0, right - 110.0),
            (FocusMode::Gaming, "🎮 Game", right - 106.0, right - 54.0),
            (FocusMode::Movie, "🎬 Movie", right - 50.0, right),
        ];
        for (fmode, label, x1, x2) in chips {
            let ctrl = InteractiveControl::FocusChip(fmode);
            let r = D2D_RECT_F {
                left: x1,
                top: row_y1,
                right: x2,
                bottom: row_y2,
            };
            let active = state.focus_mode == fmode;
            let hov = state.hovered_control == Some(ctrl);
            self.draw_pill_button(r, label, active, hov, alpha, accent);
            state.hit_zones.push(HitZone {
                left: r.left,
                top: r.top,
                right: r.right,
                bottom: r.bottom,
                control: ctrl,
            });
        }
    }

    unsafe fn draw_page_shelf(
        &self,
        state: &mut AppState,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        alpha: f32,
        accent: (f32, f32, f32),
    ) {
        if state.shelf_items.is_empty() {
            let box_rect = D2D_RECT_F {
                left,
                top: top + 4.0,
                right,
                bottom: bottom - 6.0,
            };
            if let Ok(fill) = self
                .dc_target
                .CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.05 * alpha), None)
            {
                self.dc_target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: box_rect,
                        radiusX: 12.0,
                        radiusY: 12.0,
                    },
                    &fill,
                );
            }
            if let Ok(stroke) = self
                .dc_target
                .CreateSolidColorBrush(&color(accent.0, accent.1, accent.2, 0.35 * alpha), None)
            {
                self.dc_target.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: box_rect,
                        radiusX: 12.0,
                        radiusY: 12.0,
                    },
                    &stroke,
                    1.2,
                    None,
                );
            }

            self.draw_text(
                "📥 Drag & Drop Files or Folders Into Pillify",
                &self.font_bold_13,
                D2D_RECT_F {
                    left,
                    top: top + 28.0,
                    right,
                    bottom: top + 56.0,
                },
                color(1.0, 1.0, 1.0, alpha),
                DWRITE_TEXT_ALIGNMENT_CENTER,
            );
            self.draw_text(
                "Pin frequent files here for instant 1-click launch",
                &self.font_medium_11,
                D2D_RECT_F {
                    left,
                    top: top + 56.0,
                    right,
                    bottom: top + 82.0,
                },
                color(0.65, 0.68, 0.74, alpha),
                DWRITE_TEXT_ALIGNMENT_CENTER,
            );
            return;
        }

        // Render up to 4 cards horizontally
        let card_w = 94.0;
        let card_h = 84.0;
        let gap = 8.0;
        for (idx, item) in state.shelf_items.iter().take(4).enumerate() {
            let cx = left + (idx as f32) * (card_w + gap);
            let card_rect = D2D_RECT_F {
                left: cx,
                top: top + 2.0,
                right: cx + card_w,
                bottom: top + 2.0 + card_h,
            };
            let open_ctrl = InteractiveControl::ShelfOpenItem(idx);
            let rem_ctrl = InteractiveControl::ShelfRemoveItem(idx);
            let hov = state.hovered_control == Some(open_ctrl);

            let bg_a = if hov { 0.16 } else { 0.08 };
            if let Ok(b) = self
                .dc_target
                .CreateSolidColorBrush(&color(1.0, 1.0, 1.0, bg_a * alpha), None)
            {
                self.dc_target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: card_rect,
                        radiusX: 10.0,
                        radiusY: 10.0,
                    },
                    &b,
                );
            }

            // Extension badge
            self.draw_text(
                &item.extension_badge,
                &self.font_bold_13,
                D2D_RECT_F {
                    left: cx + 8.0,
                    top: card_rect.top + 10.0,
                    right: cx + card_w - 24.0,
                    bottom: card_rect.top + 34.0,
                },
                color(accent.0, accent.1, accent.2, alpha),
                DWRITE_TEXT_ALIGNMENT_LEADING,
            );

            // Remove [✕] mini button
            let rem_rect = D2D_RECT_F {
                left: card_rect.right - 22.0,
                top: card_rect.top + 4.0,
                right: card_rect.right - 4.0,
                bottom: card_rect.top + 22.0,
            };
            self.draw_text(
                "✕",
                &self.font_regular_10,
                rem_rect,
                color(0.75, 0.75, 0.80, alpha),
                DWRITE_TEXT_ALIGNMENT_CENTER,
            );
            state.hit_zones.push(HitZone {
                left: rem_rect.left,
                top: rem_rect.top,
                right: rem_rect.right,
                bottom: rem_rect.bottom,
                control: rem_ctrl,
            });

            // File name truncated
            let short_name: String = item.display_name.chars().take(12).collect();
            self.draw_text(
                &short_name,
                &self.font_medium_11,
                D2D_RECT_F {
                    left: cx + 6.0,
                    top: card_rect.top + 48.0,
                    right: card_rect.right - 6.0,
                    bottom: card_rect.bottom - 6.0,
                },
                color(0.95, 0.95, 0.98, alpha),
                DWRITE_TEXT_ALIGNMENT_LEADING,
            );

            state.hit_zones.push(HitZone {
                left: card_rect.left,
                top: card_rect.top + 22.0,
                right: card_rect.right,
                bottom: card_rect.bottom,
                control: open_ctrl,
            });
        }

        // Footer: Item count + Clear All button
        let footer_y1 = bottom - 26.0;
        let footer_y2 = bottom - 2.0;
        let count_str = format!(
            "{} item(s) in Shelf • Click card to open",
            state.shelf_items.len()
        );
        self.draw_text(
            &count_str,
            &self.font_regular_10,
            D2D_RECT_F {
                left,
                top: footer_y1,
                right: right - 90.0,
                bottom: footer_y2,
            },
            color(0.68, 0.70, 0.76, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let clear_rect = D2D_RECT_F {
            left: right - 82.0,
            top: footer_y1,
            right,
            bottom: footer_y2,
        };
        let hov_clear = state.hovered_control == Some(InteractiveControl::ShelfClearAll);
        self.draw_pill_button(clear_rect, "Clear All", false, hov_clear, alpha, accent);
        state.hit_zones.push(HitZone {
            left: clear_rect.left,
            top: clear_rect.top,
            right: clear_rect.right,
            bottom: clear_rect.bottom,
            control: InteractiveControl::ShelfClearAll,
        });
    }

    unsafe fn draw_page_productivity(
        &self,
        state: &mut AppState,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        alpha: f32,
        accent: (f32, f32, f32),
        secondary: (f32, f32, f32),
    ) {
        // Left Panel: Pomodoro Timer + Presets
        let mid_x = (left + right) * 0.5 - 6.0;

        self.draw_text(
            &state.timer_mode_label,
            &self.font_semibold_12,
            D2D_RECT_F {
                left,
                top,
                right: mid_x,
                bottom: top + 20.0,
            },
            color(accent.0, accent.1, accent.2, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let time_str = state.timer_formatted();
        self.draw_text(
            &time_str,
            &self.font_timer_24,
            D2D_RECT_F {
                left,
                top: top + 20.0,
                right: left + 95.0,
                bottom: top + 56.0,
            },
            color(1.0, 1.0, 1.0, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        // Start/Pause & Reset buttons
        let toggle_rect = D2D_RECT_F {
            left: left + 98.0,
            top: top + 24.0,
            right: left + 150.0,
            bottom: top + 50.0,
        };
        let hov_tog = state.hovered_control == Some(InteractiveControl::PomodoroToggle);
        self.draw_pill_button(
            toggle_rect,
            if state.timer_running { "Pause" } else { "Start" },
            state.timer_running,
            hov_tog,
            alpha,
            accent,
        );
        state.hit_zones.push(HitZone {
            left: toggle_rect.left,
            top: toggle_rect.top,
            right: toggle_rect.right,
            bottom: toggle_rect.bottom,
            control: InteractiveControl::PomodoroToggle,
        });

        let rst_rect = D2D_RECT_F {
            left: left + 154.0,
            top: top + 24.0,
            right: mid_x,
            bottom: top + 50.0,
        };
        let hov_rst = state.hovered_control == Some(InteractiveControl::PomodoroReset);
        self.draw_pill_button(rst_rect, "↺", false, hov_rst, alpha, accent);
        state.hit_zones.push(HitZone {
            left: rst_rect.left,
            top: rst_rect.top,
            right: rst_rect.right,
            bottom: rst_rect.bottom,
            control: InteractiveControl::PomodoroReset,
        });

        // Presets: [25m] [5m] [15m]
        let presets = [(25u32, "25m"), (5u32, "5m"), (15u32, "15m")];
        for (i, (mins, lbl)) in presets.iter().enumerate() {
            let px = left + (i as f32) * 62.0;
            let pre_rect = D2D_RECT_F {
                left: px,
                top: top + 60.0,
                right: px + 56.0,
                bottom: top + 84.0,
            };
            let ctrl = InteractiveControl::PomodoroPreset(*mins);
            let active = state.timer_total_secs == mins * 60;
            let hov = state.hovered_control == Some(ctrl);
            self.draw_pill_button(pre_rect, lbl, active, hov, alpha, accent);
            state.hit_zones.push(HitZone {
                left: pre_rect.left,
                top: pre_rect.top,
                right: pre_rect.right,
                bottom: pre_rect.bottom,
                control: ctrl,
            });
        }

        // Hydration Tracker Row (bottom-left)
        let water_str = format!(
            "💧 Water: {}/{} glasses",
            state.config.water_glasses_today, state.config.water_daily_goal
        );
        self.draw_text(
            &water_str,
            &self.font_medium_11,
            D2D_RECT_F {
                left,
                top: bottom - 28.0,
                right: left + 125.0,
                bottom: bottom - 2.0,
            },
            color(secondary.0, secondary.1, secondary.2, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let add_w_rect = D2D_RECT_F {
            left: left + 128.0,
            top: bottom - 27.0,
            right: mid_x,
            bottom: bottom - 3.0,
        };
        let hov_w = state.hovered_control == Some(InteractiveControl::WaterAddGlass);
        self.draw_pill_button(add_w_rect, "+1 Cup", false, hov_w, alpha, accent);
        state.hit_zones.push(HitZone {
            left: add_w_rect.left,
            top: add_w_rect.top,
            right: add_w_rect.right,
            bottom: add_w_rect.bottom,
            control: InteractiveControl::WaterAddGlass,
        });

        // Right Panel: Interactive To-Do Checklist
        let rx = mid_x + 16.0;
        self.draw_text(
            "✓ Quick Tasks (Click to Toggle)",
            &self.font_semibold_12,
            D2D_RECT_F {
                left: rx,
                top,
                right,
                bottom: top + 20.0,
            },
            color(1.0, 1.0, 1.0, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        for (idx, todo) in state.config.todos.iter().take(3).enumerate() {
            let ty = top + 24.0 + (idx as f32) * 29.0;
            let row_rect = D2D_RECT_F {
                left: rx,
                top: ty,
                right,
                bottom: ty + 25.0,
            };
            let ctrl = InteractiveControl::TodoToggle(idx);
            let hov = state.hovered_control == Some(ctrl);
            let label = if todo.done {
                format!("☑ {}", todo.text)
            } else {
                format!("☐ {}", todo.text)
            };
            self.draw_pill_button(row_rect, &label, todo.done, hov, alpha, accent);
            state.hit_zones.push(HitZone {
                left: row_rect.left,
                top: row_rect.top,
                right: row_rect.right,
                bottom: row_rect.bottom,
                control: ctrl,
            });
        }
    }

    unsafe fn draw_page_customize(
        &self,
        state: &mut AppState,
        left: f32,
        top: f32,
        right: f32,
        _bottom: f32,
        alpha: f32,
        accent: (f32, f32, f32),
    ) {
        // Row 1: Style Mode, Spring Physics Preset, Accent Theme
        let style_lbl = match state.config.style {
            IslandStyle::HybridMorph => "Mode: Hybrid Morph",
            IslandStyle::AlwaysNotch => "Mode: Always Notch",
            IslandStyle::AlwaysFloatingPill => "Mode: Floating Pill",
        };
        let spring_lbl = format!("Spring: {}", state.config.spring_preset.label());
        let theme_lbl = format!("Theme: {}", state.config.accent_theme.label());

        let top_btns = [
            (
                InteractiveControl::CustomizeStyleCycle,
                style_lbl,
                left,
                left + 136.0,
            ),
            (
                InteractiveControl::CustomizeSpringCycle,
                &spring_lbl,
                left + 142.0,
                left + 272.0,
            ),
            (
                InteractiveControl::CustomizeThemeCycle,
                &theme_lbl,
                left + 278.0,
                right,
            ),
        ];

        for (ctrl, text, x1, x2) in top_btns {
            let r = D2D_RECT_F {
                left: x1,
                top: top + 2.0,
                right: x2,
                bottom: top + 30.0,
            };
            let hov = state.hovered_control == Some(ctrl);
            self.draw_pill_button(r, text, true, hov, alpha, accent);
            state.hit_zones.push(HitZone {
                left: r.left,
                top: r.top,
                right: r.right,
                bottom: r.bottom,
                control: ctrl,
            });
        }

        // Row 2 & Row 3: Live Geometry Sliders (- / + steppers)
        let adjusters = [
            (
                format!("Compact Width: {:.0}px", state.config.compact_width),
                InteractiveControl::CustomizeWidthMinus,
                InteractiveControl::CustomizeWidthPlus,
                left,
                top + 40.0,
            ),
            (
                format!("Corner Radius: {:.0}px", state.config.corner_radius),
                InteractiveControl::CustomizeRadiusMinus,
                InteractiveControl::CustomizeRadiusPlus,
                left + 210.0,
                top + 40.0,
            ),
            (
                format!("Notch Ear Flare: {:.0}px", state.config.ear_flare_radius),
                InteractiveControl::CustomizeFlareMinus,
                InteractiveControl::CustomizeFlarePlus,
                left,
                top + 76.0,
            ),
            (
                format!(
                    "Border Glow: {:.0}%",
                    state.config.border_glow_opacity * 100.0
                ),
                InteractiveControl::CustomizeGlowMinus,
                InteractiveControl::CustomizeGlowPlus,
                left + 210.0,
                top + 76.0,
            ),
        ];

        for (title, minus_ctrl, plus_ctrl, bx, by) in adjusters {
            self.draw_text(
                &title,
                &self.font_medium_11,
                D2D_RECT_F {
                    left: bx,
                    top: by,
                    right: bx + 132.0,
                    bottom: by + 26.0,
                },
                color(0.92, 0.94, 0.98, alpha),
                DWRITE_TEXT_ALIGNMENT_LEADING,
            );

            let m_rect = D2D_RECT_F {
                left: bx + 134.0,
                top: by,
                right: bx + 162.0,
                bottom: by + 26.0,
            };
            let p_rect = D2D_RECT_F {
                left: bx + 166.0,
                top: by,
                right: bx + 194.0,
                bottom: by + 26.0,
            };

            self.draw_pill_button(
                m_rect,
                "−",
                false,
                state.hovered_control == Some(minus_ctrl),
                alpha,
                accent,
            );
            self.draw_pill_button(
                p_rect,
                "+",
                false,
                state.hovered_control == Some(plus_ctrl),
                alpha,
                accent,
            );

            state.hit_zones.push(HitZone {
                left: m_rect.left,
                top: m_rect.top,
                right: m_rect.right,
                bottom: m_rect.bottom,
                control: minus_ctrl,
            });
            state.hit_zones.push(HitZone {
                left: p_rect.left,
                top: p_rect.top,
                right: p_rect.right,
                bottom: p_rect.bottom,
                control: plus_ctrl,
            });
        }
    }

    unsafe fn draw_pill_button(
        &self,
        rect: D2D_RECT_F,
        text: &str,
        active: bool,
        hovered: bool,
        alpha: f32,
        accent: (f32, f32, f32),
    ) {
        let bg_color = if active {
            color(
                accent.0 * 0.28,
                accent.1 * 0.28,
                accent.2 * 0.28,
                (if hovered { 0.95 } else { 0.85 }) * alpha,
            )
        } else if hovered {
            color(0.24, 0.25, 0.29, 0.92 * alpha)
        } else {
            color(0.14, 0.15, 0.18, 0.82 * alpha)
        };

        let r = ((rect.bottom - rect.top) * 0.5).min(13.0);
        if let Ok(b) = self.dc_target.CreateSolidColorBrush(&bg_color, None) {
            self.dc_target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: r,
                    radiusY: r,
                },
                &b,
            );
        }

        let border_c = if active {
            color(accent.0, accent.1, accent.2, 0.70 * alpha)
        } else {
            color(1.0, 1.0, 1.0, 0.14 * alpha)
        };
        if let Ok(sb) = self.dc_target.CreateSolidColorBrush(&border_c, None) {
            self.dc_target.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: r,
                    radiusY: r,
                },
                &sb,
                1.0,
                None,
            );
        }

        let txt_c = if active {
            color(1.0, 1.0, 1.0, alpha)
        } else {
            color(0.88, 0.89, 0.93, alpha)
        };
        self.draw_text(
            text,
            &self.font_medium_11,
            rect,
            txt_c,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        );
    }

    unsafe fn draw_text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: D2D_RECT_F,
        col: D2D1_COLOR_F,
        align: windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT,
    ) {
        let _ = format.SetTextAlignment(align);
        let utf16: Vec<u16> = text.encode_utf16().collect();
        if let Ok(brush) = self.dc_target.CreateSolidColorBrush(&col, None) {
            self.dc_target.DrawText(
                &utf16,
                format,
                &rect,
                &brush,
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                Default::default(),
            );
        }
    }
}

impl Drop for D2DRenderer {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.mem_dc, self.old_obj);
            let _ = DeleteObject(self.dib_bitmap);
            let _ = DeleteDC(self.mem_dc);
        }
    }
}

