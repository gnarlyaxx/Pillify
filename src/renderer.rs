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
    font_bold_12: IDWriteTextFormat,
    font_semibold_11: IDWriteTextFormat,
    font_medium_11: IDWriteTextFormat,
    font_regular_10: IDWriteTextFormat,
    font_mono_11: IDWriteTextFormat,
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
                    biHeight: -CANVAS_H,
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

            let make_ui_fmt = |weight, size: f32| -> windows::core::Result<IDWriteTextFormat> {
                let fmt = dwrite.CreateTextFormat(
                    w!("Segoe UI Variable Text"),
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

            let make_mono_fmt = |weight, size: f32| -> windows::core::Result<IDWriteTextFormat> {
                let fmt = dwrite.CreateTextFormat(
                    w!("Consolas"),
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

            let font_bold_12 = make_ui_fmt(DWRITE_FONT_WEIGHT_BOLD, 12.5)?;
            let font_semibold_11 = make_ui_fmt(DWRITE_FONT_WEIGHT_SEMI_BOLD, 11.0)?;
            let font_medium_11 = make_ui_fmt(DWRITE_FONT_WEIGHT_MEDIUM, 11.0)?;
            let font_regular_10 = make_ui_fmt(DWRITE_FONT_WEIGHT_REGULAR, 10.0)?;
            let font_mono_11 = make_mono_fmt(DWRITE_FONT_WEIGHT_SEMI_BOLD, 11.0)?;
            let font_timer_24 = make_mono_fmt(DWRITE_FONT_WEIGHT_BOLD, 22.0)?;

            Ok(Self {
                d2d_factory,
                dc_target,
                mem_dc,
                dib_bitmap,
                old_obj,
                font_bold_12,
                font_semibold_11,
                font_medium_11,
                font_regular_10,
                font_mono_11,
                font_timer_24,
            })
        }
    }

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
        let ear = ear_r * flare_clamped;
        let top_r = r * (1.0 - flare_clamped);
        let k = 0.55228475;

        sink.BeginFigure(
            D2D_POINT_2F {
                x: left - ear + top_r,
                y: top,
            },
            D2D1_FIGURE_BEGIN_FILLED,
        );

        sink.AddLine(D2D_POINT_2F {
            x: right + ear - top_r,
            y: top,
        });

        if flare_clamped > 0.02 {
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

        sink.AddLine(D2D_POINT_2F {
            x: right,
            y: bottom - r,
        });

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

        sink.AddLine(D2D_POINT_2F {
            x: left + r,
            y: bottom,
        });

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

        // Solid pitch-black matte surface (#09090B)
        let Ok(bg_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(0.035, 0.035, 0.043, 0.99), None)
        else {
            return;
        };
        self.dc_target.FillGeometry(&geo, &bg_brush, None);

        // Crisp 1px structural hairline border
        let border_alpha = if state.is_hovered {
            (state.config.border_glow_opacity + 0.08).min(0.45)
        } else {
            state.config.border_glow_opacity
        };
        if let Ok(border_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(1.0, 1.0, 1.0, border_alpha), None)
        {
            self.dc_target.DrawGeometry(&geo, &border_brush, 1.0, None);
        }

        let expand = state.animator.expand_progress.value.clamp(0.0, 1.0);
        let osd_blend = state.animator.osd_blend.value.clamp(0.0, 1.0);
        let alert_blend = state.animator.compact_alert_blend.value.clamp(0.0, 1.0);

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

        let left_label = if state.timer_running {
            state.timer_formatted()
        } else {
            state.current_time_str.clone()
        };
        self.draw_text(
            &left_label,
            &self.font_mono_11,
            D2D_RECT_F {
                left: left + pad_x,
                top,
                right: left + 90.0,
                bottom,
            },
            color(0.96, 0.96, 0.97, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        if state.is_music_active() {
            let spec_x = left + 64.0;
            if let Ok(bar_brush) = self
                .dc_target
                .CreateSolidColorBrush(&color(accent.0, accent.1, accent.2, alpha), None)
            {
                for (idx, &amp) in state.audio_spectrum.iter().enumerate() {
                    let bx = spec_x + (idx as f32) * 4.0;
                    let bh = (3.0 + amp * 12.0).clamp(3.0, 15.0);
                    let rrect = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: bx,
                            top: mid_y - bh * 0.5,
                            right: bx + 2.2,
                            bottom: mid_y + bh * 0.5,
                        },
                        radiusX: 1.1,
                        radiusY: 1.1,
                    };
                    self.dc_target.FillRoundedRectangle(&rrect, &bar_brush);
                }
            }

            let song_label = if state.artist_name.is_empty() {
                state.song_title.clone()
            } else {
                format!("{} — {}", state.song_title, state.artist_name)
            };
            let chars: String = song_label.chars().take(24).collect();
            self.draw_text(
                &chars,
                &self.font_medium_11,
                D2D_RECT_F {
                    left: spec_x + 28.0,
                    top,
                    right: right - 74.0,
                    bottom,
                },
                color(0.92, 0.92, 0.94, alpha),
                DWRITE_TEXT_ALIGNMENT_LEADING,
            );
        }

        let status_dot_color = if state.network_connected {
            color(0.20, 0.84, 0.48, alpha)
        } else {
            color(0.92, 0.28, 0.28, alpha)
        };
        if let Ok(dot_brush) = self.dc_target.CreateSolidColorBrush(&status_dot_color, None) {
            let rrect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: right - 58.0,
                    top: mid_y - 2.5,
                    right: right - 53.0,
                    bottom: mid_y + 2.5,
                },
                radiusX: 2.5,
                radiusY: 2.5,
            };
            self.dc_target.FillRoundedRectangle(&rrect, &dot_brush);
        }

        let bat_label = if state.is_charging {
            format!("{}+", state.battery_percent)
        } else {
            format!("{}%", state.battery_percent)
        };
        self.draw_text(
            &bat_label,
            &self.font_mono_11,
            D2D_RECT_F {
                left: right - 50.0,
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
            "MUTE".to_string()
        } else {
            format!("VOL {:02}", state.volume_percent)
        };
        self.draw_text(
            &label,
            &self.font_mono_11,
            D2D_RECT_F {
                left: left + 16.0,
                top,
                right: left + 82.0,
                bottom,
            },
            color(0.96, 0.96, 0.97, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let bar_l = left + 84.0;
        let bar_r = right - 16.0;
        let bar_w = (bar_r - bar_l).max(20.0);

        if let Ok(track_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.14 * alpha), None)
        {
            self.dc_target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: bar_l,
                        top: mid_y - 2.0,
                        right: bar_r,
                        bottom: mid_y + 2.0,
                    },
                    radiusX: 2.0,
                    radiusY: 2.0,
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
                            top: mid_y - 2.0,
                            right: bar_l + bar_w * fill_ratio,
                            bottom: mid_y + 2.0,
                        },
                        radiusX: 2.0,
                        radiusY: 2.0,
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
                        left: left + 16.0,
                        top: mid_y - 3.0,
                        right: left + 22.0,
                        bottom: mid_y + 3.0,
                    },
                    radiusX: 3.0,
                    radiusY: 3.0,
                },
                &badge_brush,
            );
        }

        let summary = format!("{}  /  {}", state.toast_app.to_uppercase(), state.toast_title);
        self.draw_text(
            &summary,
            &self.font_semibold_11,
            D2D_RECT_F {
                left: left + 30.0,
                top,
                right: right - 16.0,
                bottom,
            },
            color(0.96, 0.96, 0.97, alpha),
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
        let header_bottom = header_top + 24.0;

        // Clean typographic navigation bar without emojis
        let tabs = [
            (ActivePage::Media, "MEDIA", 64.0),
            (ActivePage::Shelf, "SHELF", 64.0),
            (ActivePage::Productivity, "FOCUS", 64.0),
            (ActivePage::Customize, "CONFIG", 68.0),
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
            self.draw_action_chip(rect, label, active, hovered, alpha, accent);
            state.hit_zones.push(HitZone {
                left: rect.left,
                top: rect.top,
                right: rect.right,
                bottom: rect.bottom,
                control: ctrl,
            });
            cur_x += tab_w + 6.0;
        }

        let close_rect = D2D_RECT_F {
            left: right - pad_x - 42.0,
            top: header_top,
            right: right - pad_x,
            bottom: header_bottom,
        };
        let close_hovered = state.hovered_control == Some(InteractiveControl::CollapseButton);
        self.draw_action_chip(close_rect, "HIDE", false, close_hovered, alpha, accent);
        state.hit_zones.push(HitZone {
            left: close_rect.left,
            top: close_rect.top,
            right: close_rect.right,
            bottom: close_rect.bottom,
            control: InteractiveControl::CollapseButton,
        });

        // 1px structural divider below header
        let divider_y = header_bottom + 7.0;
        if let Ok(div_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.08 * alpha), None)
        {
            self.dc_target.DrawLine(
                D2D_POINT_2F {
                    x: left + pad_x,
                    y: divider_y,
                },
                D2D_POINT_2F {
                    x: right - pad_x,
                    y: divider_y,
                },
                &div_brush,
                1.0,
                None,
            );
        }

        let content_top = divider_y + 9.0;
        let content_bottom = bottom - 16.0;

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
        // Minimal equalizer block
        let art_rect = D2D_RECT_F {
            left,
            top,
            right: left + 46.0,
            bottom: top + 46.0,
        };
        if let Ok(card_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.04 * alpha), None)
        {
            self.dc_target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: art_rect,
                    radiusX: 6.0,
                    radiusY: 6.0,
                },
                &card_brush,
            );
        }

        let mid_art_y = top + 23.0;
        if let Ok(spec_brush) = self
            .dc_target
            .CreateSolidColorBrush(&color(accent.0, accent.1, accent.2, alpha), None)
        {
            for (idx, &amp) in state.audio_spectrum.iter().enumerate() {
                let bx = left + 8.0 + (idx as f32) * 5.2;
                let bh = if state.is_playing {
                    4.0 + amp * 20.0
                } else {
                    4.0
                };
                self.dc_target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: bx,
                            top: mid_art_y - bh * 0.5,
                            right: bx + 2.8,
                            bottom: mid_art_y + bh * 0.5,
                        },
                        radiusX: 1.4,
                        radiusY: 1.4,
                    },
                    &spec_brush,
                );
            }
        }

        let title_str = if state.song_title.is_empty() {
            "Idle Audio Session"
        } else {
            &state.song_title
        };
        let artist_str = if state.artist_name.is_empty() {
            "System Media Transport"
        } else {
            &state.artist_name
        };

        self.draw_text(
            title_str,
            &self.font_bold_12,
            D2D_RECT_F {
                left: left + 56.0,
                top: top + 2.0,
                right: right - 134.0,
                bottom: top + 24.0,
            },
            color(0.96, 0.96, 0.97, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        self.draw_text(
            artist_str,
            &self.font_regular_10,
            D2D_RECT_F {
                left: left + 56.0,
                top: top + 24.0,
                right: right - 134.0,
                bottom: top + 44.0,
            },
            color(0.58, 0.58, 0.62, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let btn_y1 = top + 10.0;
        let btn_y2 = top + 36.0;
        let transport = [
            (InteractiveControl::MediaPrev, "PREV", right - 128.0, right - 88.0),
            (
                InteractiveControl::MediaPlayPause,
                if state.is_playing { "PAUSE" } else { "PLAY" },
                right - 84.0,
                right - 42.0,
            ),
            (InteractiveControl::MediaNext, "NEXT", right - 38.0, right),
        ];
        for (ctrl, label, x1, x2) in transport {
            let r = D2D_RECT_F {
                left: x1,
                top: btn_y1,
                right: x2,
                bottom: btn_y2,
            };
            let hov = state.hovered_control == Some(ctrl);
            let primary = ctrl == InteractiveControl::MediaPlayPause && state.is_playing;
            self.draw_action_chip(r, label, primary, hov, alpha, accent);
            state.hit_zones.push(HitZone {
                left: r.left,
                top: r.top,
                right: r.right,
                bottom: r.bottom,
                control: ctrl,
            });
        }

        // Clean technical readout line
        let readout_rect = D2D_RECT_F {
            left,
            top: top + 56.0,
            right,
            bottom: top + 80.0,
        };
        if let Ok(line_bg) = self
            .dc_target
            .CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.03 * alpha), None)
        {
            self.dc_target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: readout_rect,
                    radiusX: 4.0,
                    radiusY: 4.0,
                },
                &line_bg,
            );
        }
        self.draw_text(
            &state.synced_lyric_line,
            &self.font_mono_11,
            D2D_RECT_F {
                left: left + 10.0,
                top: readout_rect.top,
                right: right - 10.0,
                bottom: readout_rect.bottom,
            },
            color(0.76, 0.76, 0.80, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let row_y1 = bottom - 24.0;
        let row_y2 = bottom;
        let telemetry = format!(
            "NET {}   //   BAT {}%   //   VOL {}%",
            state.wifi_ssid.to_uppercase(),
            state.battery_percent,
            state.volume_percent
        );
        self.draw_text(
            &telemetry,
            &self.font_mono_11,
            D2D_RECT_F {
                left,
                top: row_y1,
                right: right - 166.0,
                bottom: row_y2,
            },
            color(secondary.0, secondary.1, secondary.2, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let chips = [
            (FocusMode::None, "STD", right - 156.0, right - 108.0),
            (FocusMode::Gaming, "DND", right - 104.0, right - 54.0),
            (FocusMode::Movie, "MUTE", right - 50.0, right),
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
            self.draw_action_chip(r, label, active, hov, alpha, accent);
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
                top: top + 2.0,
                right,
                bottom: bottom - 4.0,
            };
            if let Ok(stroke) = self
                .dc_target
                .CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.14 * alpha), None)
            {
                self.dc_target.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: box_rect,
                        radiusX: 6.0,
                        radiusY: 6.0,
                    },
                    &stroke,
                    1.0,
                    None,
                );
            }

            self.draw_text(
                "DROP FILES OR DIRECTORIES HERE",
                &self.font_mono_11,
                D2D_RECT_F {
                    left,
                    top: top + 28.0,
                    right,
                    bottom: top + 52.0,
                },
                color(0.92, 0.92, 0.95, alpha),
                DWRITE_TEXT_ALIGNMENT_CENTER,
            );
            self.draw_text(
                "Staged paths persist locally for instant launch",
                &self.font_regular_10,
                D2D_RECT_F {
                    left,
                    top: top + 52.0,
                    right,
                    bottom: top + 74.0,
                },
                color(0.55, 0.55, 0.60, alpha),
                DWRITE_TEXT_ALIGNMENT_CENTER,
            );
            return;
        }

        // Structured list rows instead of generic card grid
        let row_h = 24.0;
        for (idx, entry) in state.shelf_items.iter().take(3).enumerate() {
            let ry = top + (idx as f32) * (row_h + 4.0);
            let row_rect = D2D_RECT_F {
                left,
                top: ry,
                right: right - 32.0,
                bottom: ry + row_h,
            };
            let open_ctrl = InteractiveControl::ShelfOpenItem(idx);
            let rem_ctrl = InteractiveControl::ShelfRemoveItem(idx);
            let hov = state.hovered_control == Some(open_ctrl);

            let row_label = format!("[{}]  {}", entry.extension_badge, entry.display_name);
            self.draw_action_chip(row_rect, &row_label, false, hov, alpha, accent);
            state.hit_zones.push(HitZone {
                left: row_rect.left,
                top: row_rect.top,
                right: row_rect.right,
                bottom: row_rect.bottom,
                control: open_ctrl,
            });

            let rem_rect = D2D_RECT_F {
                left: right - 28.0,
                top: ry,
                right,
                bottom: ry + row_h,
            };
            let rem_hov = state.hovered_control == Some(rem_ctrl);
            self.draw_action_chip(rem_rect, "DEL", false, rem_hov, alpha, accent);
            state.hit_zones.push(HitZone {
                left: rem_rect.left,
                top: rem_rect.top,
                right: rem_rect.right,
                bottom: rem_rect.bottom,
                control: rem_ctrl,
            });
        }

        let footer_y1 = bottom - 24.0;
        let footer_y2 = bottom;
        let count_str = format!("{} STAGED ITEMS", state.shelf_items.len());
        self.draw_text(
            &count_str,
            &self.font_mono_11,
            D2D_RECT_F {
                left,
                top: footer_y1,
                right: right - 90.0,
                bottom: footer_y2,
            },
            color(0.58, 0.58, 0.62, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let clear_rect = D2D_RECT_F {
            left: right - 82.0,
            top: footer_y1,
            right,
            bottom: footer_y2,
        };
        let hov_clear = state.hovered_control == Some(InteractiveControl::ShelfClearAll);
        self.draw_action_chip(clear_rect, "CLEAR ALL", false, hov_clear, alpha, accent);
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
        let mid_x = (left + right) * 0.5 - 6.0;

        self.draw_text(
            &state.timer_mode_label,
            &self.font_mono_11,
            D2D_RECT_F {
                left,
                top,
                right: mid_x,
                bottom: top + 18.0,
            },
            color(secondary.0, secondary.1, secondary.2, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let time_str = state.timer_formatted();
        self.draw_text(
            &time_str,
            &self.font_timer_24,
            D2D_RECT_F {
                left,
                top: top + 18.0,
                right: left + 92.0,
                bottom: top + 52.0,
            },
            color(0.96, 0.96, 0.97, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let toggle_rect = D2D_RECT_F {
            left: left + 96.0,
            top: top + 22.0,
            right: left + 148.0,
            bottom: top + 46.0,
        };
        let hov_tog = state.hovered_control == Some(InteractiveControl::PomodoroToggle);
        self.draw_action_chip(
            toggle_rect,
            if state.timer_running { "PAUSE" } else { "START" },
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
            left: left + 152.0,
            top: top + 22.0,
            right: mid_x,
            bottom: top + 46.0,
        };
        let hov_rst = state.hovered_control == Some(InteractiveControl::PomodoroReset);
        self.draw_action_chip(rst_rect, "RST", false, hov_rst, alpha, accent);
        state.hit_zones.push(HitZone {
            left: rst_rect.left,
            top: rst_rect.top,
            right: rst_rect.right,
            bottom: rst_rect.bottom,
            control: InteractiveControl::PomodoroReset,
        });

        let presets = [(25u32, "25M"), (5u32, "05M"), (15u32, "15M")];
        for (idx, (mins, lbl)) in presets.iter().enumerate() {
            let px = left + (idx as f32) * 62.0;
            let pre_rect = D2D_RECT_F {
                left: px,
                top: top + 56.0,
                right: px + 56.0,
                bottom: top + 78.0,
            };
            let ctrl = InteractiveControl::PomodoroPreset(*mins);
            let active = state.timer_total_secs == mins * 60;
            let hov = state.hovered_control == Some(ctrl);
            self.draw_action_chip(pre_rect, lbl, active, hov, alpha, accent);
            state.hit_zones.push(HitZone {
                left: pre_rect.left,
                top: pre_rect.top,
                right: pre_rect.right,
                bottom: pre_rect.bottom,
                control: ctrl,
            });
        }

        let water_str = format!(
            "HYDRATION  {}/{}",
            state.config.water_glasses_today, state.config.water_daily_goal
        );
        self.draw_text(
            &water_str,
            &self.font_mono_11,
            D2D_RECT_F {
                left,
                top: bottom - 24.0,
                right: left + 125.0,
                bottom,
            },
            color(secondary.0, secondary.1, secondary.2, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        let add_w_rect = D2D_RECT_F {
            left: left + 128.0,
            top: bottom - 24.0,
            right: mid_x,
            bottom,
        };
        let hov_w = state.hovered_control == Some(InteractiveControl::WaterAddGlass);
        self.draw_action_chip(add_w_rect, "+1 UNIT", false, hov_w, alpha, accent);
        state.hit_zones.push(HitZone {
            left: add_w_rect.left,
            top: add_w_rect.top,
            right: add_w_rect.right,
            bottom: add_w_rect.bottom,
            control: InteractiveControl::WaterAddGlass,
        });

        let rx = mid_x + 14.0;
        self.draw_text(
            "TASKS",
            &self.font_mono_11,
            D2D_RECT_F {
                left: rx,
                top,
                right,
                bottom: top + 18.0,
            },
            color(secondary.0, secondary.1, secondary.2, alpha),
            DWRITE_TEXT_ALIGNMENT_LEADING,
        );

        for (idx, todo) in state.config.todos.iter().take(3).enumerate() {
            let ty = top + 22.0 + (idx as f32) * 28.0;
            let row_rect = D2D_RECT_F {
                left: rx,
                top: ty,
                right,
                bottom: ty + 24.0,
            };
            let ctrl = InteractiveControl::TodoToggle(idx);
            let hov = state.hovered_control == Some(ctrl);
            let label = if todo.done {
                format!("[X] {}", todo.text)
            } else {
                format!("[ ] {}", todo.text)
            };
            self.draw_action_chip(row_rect, &label, todo.done, hov, alpha, accent);
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
        let style_lbl = match state.config.style {
            IslandStyle::HybridMorph => "GEOMETRY: HYBRID",
            IslandStyle::AlwaysNotch => "GEOMETRY: NOTCH",
            IslandStyle::AlwaysFloatingPill => "GEOMETRY: PILL",
        };
        let spring_lbl = format!("SPRING: {}", state.config.spring_preset.label().to_uppercase());
        let theme_lbl = format!("ACCENT: {}", state.config.accent_theme.label().to_uppercase());

        let top_btns = [
            (
                InteractiveControl::CustomizeStyleCycle,
                style_lbl,
                left,
                left + 132.0,
            ),
            (
                InteractiveControl::CustomizeSpringCycle,
                &spring_lbl,
                left + 138.0,
                left + 266.0,
            ),
            (
                InteractiveControl::CustomizeThemeCycle,
                &theme_lbl,
                left + 272.0,
                right,
            ),
        ];

        for (ctrl, text, x1, x2) in top_btns {
            let r = D2D_RECT_F {
                left: x1,
                top,
                right: x2,
                bottom: top + 24.0,
            };
            let hov = state.hovered_control == Some(ctrl);
            self.draw_action_chip(r, text, false, hov, alpha, accent);
            state.hit_zones.push(HitZone {
                left: r.left,
                top: r.top,
                right: r.right,
                bottom: r.bottom,
                control: ctrl,
            });
        }

        let adjusters = [
            (
                format!("WIDTH   {:.0}PX", state.config.compact_width),
                InteractiveControl::CustomizeWidthMinus,
                InteractiveControl::CustomizeWidthPlus,
                left,
                top + 36.0,
            ),
            (
                format!("RADIUS  {:.0}PX", state.config.corner_radius),
                InteractiveControl::CustomizeRadiusMinus,
                InteractiveControl::CustomizeRadiusPlus,
                left + 206.0,
                top + 36.0,
            ),
            (
                format!("FLARE   {:.0}PX", state.config.ear_flare_radius),
                InteractiveControl::CustomizeFlareMinus,
                InteractiveControl::CustomizeFlarePlus,
                left,
                top + 70.0,
            ),
            (
                format!(
                    "BORDER  {:.0}%",
                    state.config.border_glow_opacity * 100.0
                ),
                InteractiveControl::CustomizeGlowMinus,
                InteractiveControl::CustomizeGlowPlus,
                left + 206.0,
                top + 70.0,
            ),
        ];

        for (title, minus_ctrl, plus_ctrl, bx, by) in adjusters {
            self.draw_text(
                &title,
                &self.font_mono_11,
                D2D_RECT_F {
                    left: bx,
                    top: by,
                    right: bx + 126.0,
                    bottom: by + 24.0,
                },
                color(0.84, 0.84, 0.88, alpha),
                DWRITE_TEXT_ALIGNMENT_LEADING,
            );

            let m_rect = D2D_RECT_F {
                left: bx + 130.0,
                top: by,
                right: bx + 158.0,
                bottom: by + 24.0,
            };
            let p_rect = D2D_RECT_F {
                left: bx + 162.0,
                top: by,
                right: bx + 190.0,
                bottom: by + 24.0,
            };

            self.draw_action_chip(
                m_rect,
                "-",
                false,
                state.hovered_control == Some(minus_ctrl),
                alpha,
                accent,
            );
            self.draw_action_chip(
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

    unsafe fn draw_action_chip(
        &self,
        rect: D2D_RECT_F,
        text: &str,
        active: bool,
        hovered: bool,
        alpha: f32,
        accent: (f32, f32, f32),
    ) {
        let bg_color = if active {
            color(1.0, 1.0, 1.0, (if hovered { 0.16 } else { 0.12 }) * alpha)
        } else if hovered {
            color(1.0, 1.0, 1.0, 0.08 * alpha)
        } else {
            color(1.0, 1.0, 1.0, 0.03 * alpha)
        };

        let r = 5.0;
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
            color(accent.0, accent.1, accent.2, 0.55 * alpha)
        } else if hovered {
            color(1.0, 1.0, 1.0, 0.18 * alpha)
        } else {
            color(1.0, 1.0, 1.0, 0.07 * alpha)
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
            color(0.98, 0.98, 0.99, alpha)
        } else {
            color(0.74, 0.74, 0.78, alpha)
        };
        self.draw_text(
            text,
            &self.font_semibold_11,
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
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
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
