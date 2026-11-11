use super::traits::WindowAdapter;
use crate::{core::state::DisplayInfo, message::Message, ui::notch};
use iced::{Point, Size, Task, window};
use raw_window_handle::RawWindowHandle;
use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DwmSetWindowAttribute},
    Graphics::Gdi::{
        CreatePolygonRgn, DeleteObject, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO,
        MonitorFromWindow, SetWindowRgn, WINDING,
    },
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, HWND_TOPMOST, SW_HIDE, SW_SHOWNOACTIVATE,
            SWP_FRAMECHANGED, SWP_NOACTIVATE, SetWindowLongPtrW, SetWindowPos, ShowWindow,
            WS_BORDER, WS_CAPTION, WS_EX_APPWINDOW, WS_EX_CLIENTEDGE, WS_EX_STATICEDGE,
            WS_EX_TOOLWINDOW, WS_EX_WINDOWEDGE, WS_THICKFRAME,
        },
    },
};

pub struct WindowsWindow {
    /// Development inspection keeps the native window discoverable to desktop tools.
    pub inspect: bool,
}

impl WindowAdapter for WindowsWindow {
    fn settings(&self, size: Size) -> window::Settings {
        window::Settings {
            size,
            position: window::Position::SpecificWith(|window, monitor| {
                Point::new((monitor.width - window.width) * 0.5, 0.0)
            }),
            decorations: false,
            transparent: true,
            resizable: false,
            level: window::Level::AlwaysOnTop,
            visible: false,
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                skip_taskbar: !self.inspect,
                drag_and_drop: false,
                undecorated_shadow: false,
                corner_preference: window::settings::platform::CornerPreference::DoNotRound,
            },
            ..Default::default()
        }
    }

    fn synchronize(&self, id: window::Id, size: Size, reveal: bool) -> Task<Message> {
        let inspect = self.inspect;
        window::run(id, move |window| {
            let Ok(handle) = window.window_handle() else {
                return DisplayInfo::default();
            };
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return DisplayInfo::default();
            };
            let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);
            super::windows_desktop::set_notch(hwnd);
            // SAFETY: iced provides a live HWND on its event-loop thread. No pointer is retained.
            // GDI owns the region only after successful SetWindowRgn; failure frees it here.
            unsafe {
                let scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                let mut info = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                    tracing::warn!(
                        "GetMonitorInfoW failed; keeping bounded window in its current location"
                    );
                    if reveal {
                        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    }
                    return DisplayInfo::default();
                }
                let width = (size.width * scale).ceil() as i32;
                let height = (size.height * scale).ceil() as i32;
                let x =
                    info.rcMonitor.left + (info.rcMonitor.right - info.rcMonitor.left - width) / 2;
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                let style = if inspect {
                    (style | WS_EX_APPWINDOW.0 as isize) & !(WS_EX_TOOLWINDOW.0 as isize)
                } else {
                    (style | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize)
                };
                let style =
                    style & !((WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE | WS_EX_STATICEDGE).0 as isize);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
                // winit retains some non-client styles even for undecorated windows.
                // Remove their border metrics so the client pixels meet the monitor edge.
                let native_style = GetWindowLongPtrW(hwnd, GWL_STYLE);
                let borderless =
                    native_style & !((WS_BORDER | WS_CAPTION | WS_THICKFRAME).0 as isize);
                let frame_flags = if native_style != borderless {
                    SetWindowLongPtrW(hwnd, GWL_STYLE, borderless);
                    SWP_FRAMECHANGED
                } else {
                    Default::default()
                };
                // Windows 11's DWM border is independent of the Canvas silhouette.
                // Older Windows versions may not support this attribute.
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_BORDER_COLOR,
                    (&DWMWA_COLOR_NONE as *const u32).cast(),
                    std::mem::size_of::<u32>() as u32,
                );
                if let Err(error) = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    x,
                    info.rcMonitor.top,
                    width,
                    height,
                    SWP_NOACTIVATE | frame_flags,
                ) {
                    tracing::warn!(%error, "Native top-edge positioning failed");
                }
                let points: Vec<POINT> = notch::outline(size)
                    .into_iter()
                    .map(|p| POINT {
                        x: (p.x * scale).round() as i32,
                        y: (p.y * scale).round() as i32,
                    })
                    .collect();
                let region = CreatePolygonRgn(&points, WINDING);
                if region.is_invalid() {
                    tracing::warn!(
                        "Cannot create notch region; bounded rectangular hit area remains"
                    );
                } else if SetWindowRgn(hwnd, Some(region), true) == 0 {
                    let _ = DeleteObject(region.into());
                    tracing::warn!(
                        "Cannot apply notch region; bounded rectangular hit area remains"
                    );
                }
                if reveal {
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                }
                DisplayInfo {
                    width: (info.rcMonitor.right - info.rcMonitor.left) as f32 / scale,
                    height: (info.rcMonitor.bottom - info.rcMonitor.top) as f32 / scale,
                    scale,
                }
            }
        })
        .map(Message::Display)
    }

    fn visibility(&self, id: window::Id, visible: bool) -> Task<Message> {
        window::run(id, move |window| {
            if let Ok(handle) = window.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                // iced supplies the live window on its own event-loop thread.
                unsafe {
                    let _ = ShowWindow(
                        HWND(handle.hwnd.get() as *mut _),
                        if visible { SW_SHOWNOACTIVATE } else { SW_HIDE },
                    );
                }
            }
        })
        .discard()
    }
}
