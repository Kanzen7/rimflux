use super::traits::WindowAdapter;
use crate::{core::state::DisplayInfo, message::Message, ui::notch};
use iced::{Point, Size, Task, window};
use raw_window_handle::RawWindowHandle;
use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::{
        CreatePolygonRgn, DeleteObject, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO,
        MonitorFromWindow, SetWindowRgn, WINDING,
    },
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, SWP_NOACTIVATE, SetWindowLongPtrW,
            SetWindowPos, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
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
            platform_specific: window::settings::PlatformSpecific {
                skip_taskbar: !self.inspect,
                drag_and_drop: false,
                corner_preference: window::settings::platform::CornerPreference::DoNotRound,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn synchronize(&self, id: window::Id, size: Size) -> Task<Message> {
        let inspect = self.inspect;
        window::run(id, move |window| {
            let Ok(handle) = window.window_handle() else {
                return DisplayInfo::default();
            };
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return DisplayInfo::default();
            };
            let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);
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
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
                if let Err(error) = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    x,
                    info.rcMonitor.top,
                    width,
                    height,
                    SWP_NOACTIVATE,
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
                DisplayInfo {
                    width: (info.rcMonitor.right - info.rcMonitor.left) as f32 / scale,
                    height: (info.rcMonitor.bottom - info.rcMonitor.top) as f32 / scale,
                    scale,
                }
            }
        })
        .map(Message::Display)
    }
}
