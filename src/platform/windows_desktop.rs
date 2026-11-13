//! Owns the Windows message pump, recovery controls, and foreground observation.
use crate::{
    core::{
        desktop::{
            Foreground, HideFor, ModeSelection, Shortcut, VisibilityPolicy, classify_window,
            covers_monitor,
        },
        state::{Action, AppState, NotchMode, Panel},
    },
    message::Message,
};
use iced::{Subscription, futures::SinkExt};
use std::{
    cell::Cell,
    ffi::c_void,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicIsize, Ordering},
    },
};
use tokio::sync::mpsc;
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, GetLastError, HWND, LPARAM,
            LRESULT, POINT, RECT, WPARAM,
        },
        Graphics::{
            Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
            Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
        },
        System::{
            LibraryLoader::GetModuleHandleW,
            Registry::{HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW},
            Threading::{
                CreateMutexW, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
        UI::{
            Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
            Input::KeyboardAndMouse::{
                MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey,
            },
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION,
                NIN_SELECT, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
            },
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, PWSTR, w},
};

const TRAY_MESSAGE: u32 = WM_APP + 1;
const REFRESH: u32 = WM_APP + 2;
const CONFIGURE: u32 = WM_APP + 3;
const RECONCILE_TIMER: usize = 1;
const GEOMETRY_TIMER: usize = 2;
const CHECK_FOREGROUND: u8 = 1;
const CHECK_OPTIONS: u8 = 2;
const RECREATE_TRAY: u8 = 4;
const OPEN_MENU: u8 = 8;
const RESTORE: u8 = 16;
const HOTKEY: u8 = 32;
const STOP: u8 = 64;
static NOTCH: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    static WINDOW: Cell<HWND> = const { Cell::new(HWND(std::ptr::null_mut())) };
    static SIGNALS: Cell<u8> = const { Cell::new(0) };
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
    static MENU_POSITION: Cell<Option<POINT>> = const { Cell::new(None) };
}

pub fn set_notch(hwnd: HWND) {
    NOTCH.store(hwnd.0 as isize, Ordering::Release);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Options {
    shortcut: String,
    tray: bool,
    mode: NotchMode,
    policy: VisibilityPolicy,
    selection: ModeSelection,
    visible: bool,
}

impl From<&AppState> for Options {
    fn from(state: &AppState) -> Self {
        Self {
            shortcut: state.config.interaction.global_shortcut.clone(),
            tray: state.config.general.keep_running_in_tray,
            mode: state.mode,
            policy: state.config.visibility.policy,
            selection: state.selection,
            visible: state.visible(),
        }
    }
}

impl Options {
    fn matches(&self, state: &AppState) -> bool {
        self.shortcut == state.config.interaction.global_shortcut
            && self.tray == state.config.general.keep_running_in_tray
            && self.mode == state.mode
            && self.policy == state.config.visibility.policy
            && self.selection == state.selection
            && self.visible == state.visible()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopStatus {
    pub foreground: Foreground,
    pub tray: bool,
    pub hotkey: bool,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
struct Shared {
    hwnd: AtomicIsize,
    stopping: AtomicBool,
    options: Mutex<Option<Options>>,
}

#[derive(Debug, Clone)]
pub struct DesktopHandle(Arc<Shared>);

impl DesktopHandle {
    pub fn configure(&self, state: &AppState) {
        let Ok(mut current) = self.0.options.lock() else {
            return;
        };
        if current
            .as_ref()
            .is_some_and(|options| options.matches(state))
        {
            return;
        }
        *current = Some(state.into());
        drop(current);
        self.post(CONFIGURE);
    }

    pub fn stop(&self) {
        self.0.stopping.store(true, Ordering::Release);
        self.post(WM_CLOSE);
    }

    fn post(&self, message: u32) {
        let hwnd = self.0.hwnd.load(Ordering::Acquire);
        if hwnd != 0 {
            // A disappearing window simply rejects the queued notification.
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(hwnd as *mut c_void)),
                    message,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }
}

struct StopOnDrop(DesktopHandle);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.stop();
    }
}

pub fn subscription(initial: Options) -> Subscription<Message> {
    // The identity is stable across settings edits. The live worker is reconfigured
    // through its handle instead of replacing the message window or tray icon.
    Subscription::run_with(initial, |initial| {
        let initial = initial.clone();
        iced::stream::channel::<Message>(16, async move |mut output| {
            let (tx, mut rx) = mpsc::channel(16);
            let handle = DesktopHandle(Arc::new(Shared {
                options: Mutex::new(Some(initial)),
                ..Shared::default()
            }));
            let _stop = StopOnDrop(handle.clone());
            let errors = tx.clone();
            let worker = std::thread::Builder::new()
                .name("rimflux-desktop".into())
                .spawn(move || {
                    if let Err(error) = run(&handle, &tx) {
                        if error.code() == ERROR_ALREADY_EXISTS.to_hresult() {
                            tracing::info!("Rimflux desktop controls are already running");
                            let _ = tx.blocking_send(Message::Quit);
                        } else {
                            let _ = tx.blocking_send(Message::DesktopStatus(DesktopStatus {
                                error: Some(error.to_string()),
                                ..DesktopStatus::default()
                            }));
                        }
                    }
                    let _ = tx.blocking_send(Message::DesktopStopped);
                });
            if let Err(error) = worker {
                let _ = errors
                    .send(Message::DesktopStatus(DesktopStatus {
                        error: Some(error.to_string()),
                        ..DesktopStatus::default()
                    }))
                    .await;
            }
            drop(errors);
            while let Some(message) = rx.recv().await {
                if output.send(message).await.is_err() {
                    break;
                }
            }
        })
    })
}

// Only called on the owning Win32 thread, including out-of-context callbacks.
fn signal(bit: u8) {
    SIGNALS.set(SIGNALS.get() | bit);
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CLOSE => {
                signal(STOP);
                let _ = EndMenu();
            }
            REFRESH => signal(CHECK_FOREGROUND),
            CONFIGURE => signal(CHECK_OPTIONS),
            WM_TIMER if w.0 == RECONCILE_TIMER => signal(CHECK_FOREGROUND | CHECK_OPTIONS),
            WM_TIMER if w.0 == GEOMETRY_TIMER => {
                let _ = KillTimer(Some(hwnd), GEOMETRY_TIMER);
                signal(CHECK_FOREGROUND);
            }
            WM_HOTKEY => signal(HOTKEY),
            TRAY_MESSAGE => match (l.0 as u32) & 0xffff {
                WM_CONTEXTMENU => {
                    MENU_POSITION.set(Some(POINT {
                        x: (w.0 as u16 as i16).into(),
                        y: ((w.0 >> 16) as u16 as i16).into(),
                    }));
                    signal(OPEN_MENU);
                }
                WM_RBUTTONUP => {
                    MENU_POSITION.set(None);
                    signal(OPEN_MENU);
                }
                NIN_SELECT | WM_LBUTTONUP => signal(RESTORE),
                // NIN_KEYSELECT is defined as NIN_SELECT | NINF_KEY in the SDK.
                value if value == NIN_SELECT + 1 => signal(RESTORE),
                _ => {}
            },
            value if value != 0 && value == TASKBAR_CREATED.get() => signal(RECREATE_TRAY),
            _ => return DefWindowProcW(hwnd, msg, w, l),
        }
    }
    LRESULT(0)
}

unsafe extern "system" fn window_event(
    _: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _: u32,
    _: u32,
) {
    let receiver = WINDOW.get();
    if receiver.is_invalid() {
        return;
    }
    unsafe {
        if event == EVENT_OBJECT_LOCATIONCHANGE {
            if object == 0 && child == 0 && hwnd == GetForegroundWindow() {
                SetTimer(Some(receiver), GEOMETRY_TIMER, 45, None);
            }
        } else {
            let _ = PostMessageW(Some(receiver), REFRESH, WPARAM(0), LPARAM(0));
        }
    }
}

struct Native {
    hwnd: HWND,
    instance: windows::Win32::Foundation::HINSTANCE,
    icon: Option<HICON>,
    tray: bool,
    hotkey: Option<(i32, Shortcut)>,
    hooks: Vec<HWINEVENTHOOK>,
    guard: windows::Win32::Foundation::HANDLE,
}

const CLASS: PCWSTR = w!("Rimflux.DesktopControls");

impl Native {
    fn open() -> windows::core::Result<Self> {
        // Named handle prevents two instances from adding recovery icons.
        unsafe {
            let guard = CreateMutexW(None, false, w!("Local\\Rimflux.DesktopControls"))?;
            let mut native = Self {
                guard,
                hwnd: HWND::default(),
                instance: Default::default(),
                icon: None,
                tray: false,
                hotkey: None,
                hooks: Vec::new(),
            };
            if GetLastError() == ERROR_ALREADY_EXISTS {
                return Err(windows::core::Error::new(
                    ERROR_ALREADY_EXISTS.to_hresult(),
                    "Rimflux desktop controls are already running",
                ));
            }
            native.instance = GetModuleHandleW(None)?.into();
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: native.instance,
                lpszClassName: CLASS,
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                return Err(windows::core::Error::from_thread());
            }
            native.hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                CLASS,
                w!("Rimflux"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(native.instance),
                None,
            )?;
            WINDOW.set(native.hwnd);
            TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));
            if SetTimer(Some(native.hwnd), RECONCILE_TIMER, 1000, None) == 0 {
                return Err(windows::core::Error::from_thread());
            }
            for event in [
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_MOVESIZEEND,
                EVENT_SYSTEM_MINIMIZESTART,
                EVENT_SYSTEM_MINIMIZEEND,
                EVENT_OBJECT_LOCATIONCHANGE,
            ] {
                let hook = SetWinEventHook(
                    event,
                    event,
                    None,
                    Some(window_event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                );
                if hook.is_invalid() {
                    tracing::warn!(
                        event,
                        "WinEvent hook unavailable; using reconciliation timer"
                    );
                } else {
                    native.hooks.push(hook);
                }
            }
            Ok(native)
        }
    }

    fn icon_data(&self) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: TRAY_MESSAGE,
            hIcon: self.icon.unwrap_or_default(),
            ..Default::default()
        };
        for (slot, unit) in data.szTip.iter_mut().zip("Rimflux".encode_utf16()) {
            *slot = unit;
        }
        data
    }

    fn tray(&mut self, enabled: bool) {
        if enabled == self.tray {
            return;
        }
        unsafe {
            if enabled {
                if self.icon.is_none() {
                    self.icon = create_icon(self.instance).ok();
                }
                if self.icon.is_none() {
                    return;
                }
                let mut data = self.icon_data();
                self.tray = Shell_NotifyIconW(NIM_ADD, &data).as_bool();
                if self.tray {
                    data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
                    let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
                }
            } else {
                let _ = Shell_NotifyIconW(NIM_DELETE, &self.icon_data());
                self.tray = false;
            }
        }
    }

    fn hotkey(&mut self, text: &str) -> windows::core::Result<()> {
        let shortcut = Shortcut::parse(text).ok_or_else(|| {
            windows::core::Error::new(
                windows::core::HRESULT(0x80070057_u32 as i32),
                "Invalid shortcut",
            )
        })?;
        if self.hotkey.is_some_and(|(_, active)| active == shortcut) {
            return Ok(());
        }
        let id = if self.hotkey.is_some_and(|(id, _)| id == 1) {
            2
        } else {
            1
        };
        let mut flags = MOD_NOREPEAT;
        for (enabled, flag) in [
            (shortcut.ctrl, MOD_CONTROL),
            (shortcut.alt, MOD_ALT),
            (shortcut.shift, MOD_SHIFT),
        ] {
            if enabled {
                flags |= flag;
            }
        }
        unsafe {
            RegisterHotKey(Some(self.hwnd), id, flags, shortcut.key)?;
            if let Some((previous, _)) = self.hotkey.replace((id, shortcut)) {
                let _ = UnregisterHotKey(Some(self.hwnd), previous);
            }
        }
        Ok(())
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        self.tray(false);
        unsafe {
            for hook in self.hooks.drain(..) {
                let _ = UnhookWinEvent(hook);
            }
            if let Some((id, _)) = self.hotkey {
                let _ = UnregisterHotKey(Some(self.hwnd), id);
            }
            if let Some(icon) = self.icon {
                let _ = DestroyIcon(icon);
            }
            if !self.hwnd.is_invalid() {
                let _ = KillTimer(Some(self.hwnd), RECONCILE_TIMER);
                let _ = KillTimer(Some(self.hwnd), GEOMETRY_TIMER);
                let _ = DestroyWindow(self.hwnd);
            }
            if !self.instance.is_invalid() {
                let _ = UnregisterClassW(CLASS, Some(self.instance));
            }
            let _ = CloseHandle(self.guard);
        }
    }
}

fn create_icon(instance: windows::Win32::Foundation::HINSTANCE) -> windows::core::Result<HICON> {
    let mut mask = [255_u8; 32];
    let mut bgra = [0_u8; 16 * 16 * 4];
    for y in 3..13 {
        for x in 2..14 {
            if y >= 11 && (x == 2 || x == 13) {
                continue;
            }
            mask[y * 2 + x / 8] &= !(0x80 >> (x % 8));
            let index = (y * 16 + x) * 4;
            let color = if y <= 4 {
                [245, 209, 114, 255]
            } else {
                [29, 24, 20, 255]
            };
            bgra[index..index + 4].copy_from_slice(&color);
        }
    }
    unsafe { CreateIcon(Some(instance), 16, 16, 1, 32, mask.as_ptr(), bgra.as_ptr()) }
}

fn run(handle: &DesktopHandle, tx: &mpsc::Sender<Message>) -> windows::core::Result<()> {
    let mut native = Native::open()?;
    handle
        .0
        .hwnd
        .store(native.hwnd.0 as isize, Ordering::Release);
    let Some(mut options) = handle
        .0
        .options
        .lock()
        .ok()
        .and_then(|options| options.clone())
    else {
        return Ok(());
    };
    if tx
        .blocking_send(Message::DesktopConnected(handle.clone()))
        .is_err()
    {
        return Ok(());
    }
    let mut last = None;
    let mut attempted_shortcut = String::new();
    let mut hotkey_error = None;
    let mut foreground = Foreground::Desktop;
    let mut flags = CHECK_FOREGROUND | CHECK_OPTIONS;
    while !handle.0.stopping.load(Ordering::Acquire) && !tx.is_closed() && flags & STOP == 0 {
        if flags & CHECK_OPTIONS != 0
            && let Some(updated) = handle
                .0
                .options
                .lock()
                .ok()
                .and_then(|options| options.clone())
        {
            options = updated;
            if options.shortcut != attempted_shortcut {
                hotkey_error = native.hotkey(&options.shortcut).err().map(|error| {
                    format!(
                        "Shortcut {} could not be registered ({error})",
                        options.shortcut
                    )
                });
                attempted_shortcut.clone_from(&options.shortcut);
            }
            native.tray(options.tray);
        }
        if flags & RECREATE_TRAY != 0 {
            native.tray = false;
            native.tray(options.tray);
        }
        if flags & CHECK_FOREGROUND != 0
            && let Some(current) = foreground_window()
        {
            foreground = current;
        }
        let status = DesktopStatus {
            foreground,
            tray: native.tray,
            hotkey: native.hotkey.is_some(),
            error: hotkey_error.clone().or_else(|| {
                (options.tray && !native.tray).then(|| "Notification-area icon unavailable".into())
            }),
        };
        if last.as_ref() != Some(&status)
            && tx.try_send(Message::DesktopStatus(status.clone())).is_ok()
        {
            last = Some(status);
        }
        for (signal, action) in [
            (RESTORE, Action::Restore),
            (HOTKEY, Action::ToggleVisibility),
        ] {
            if flags & signal != 0 {
                let _ = tx.blocking_send(Message::Action(action));
            }
        }
        if flags & OPEN_MENU != 0
            && let Some(message) = tray_menu(native.hwnd, &options)
            && !handle.0.stopping.load(Ordering::Acquire)
        {
            let _ = tx.blocking_send(message);
        }
        flags = SIGNALS.replace(0);
        if flags != 0 {
            continue;
        }
        let mut msg = MSG::default();
        unsafe {
            let result = GetMessageW(&mut msg, None, 0, 0).0;
            if result < 0 {
                handle.0.hwnd.store(0, Ordering::Release);
                return Err(windows::core::Error::from_thread());
            }
            if result == 0 {
                break;
            }
            DispatchMessageW(&msg);
        }
        flags = SIGNALS.replace(0);
    }
    handle.0.hwnd.store(0, Ordering::Release);
    Ok(())
}

fn tray_menu(hwnd: HWND, options: &Options) -> Option<Message> {
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let result = (|| -> windows::core::Result<u32> {
            AppendMenuW(menu, MF_STRING, 1, w!("&Show Rimflux"))?;
            AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
            for (id, mode, title) in [
                (2, NotchMode::Clock, w!("&Clock")),
                (3, NotchMode::Music, w!("&Music")),
                (4, NotchMode::Monitoring, w!("System M&onitor")),
            ] {
                AppendMenuW(
                    menu,
                    MF_STRING
                        | if options.mode == mode {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        },
                    id,
                    title,
                )?;
            }
            let policy_menu = CreatePopupMenu()?;
            if let Err(error) = AppendMenuW(
                menu,
                MF_POPUP,
                policy_menu.0 as usize,
                w!("&Visibility Mode"),
            ) {
                let _ = DestroyMenu(policy_menu);
                return Err(error);
            }
            for (id, policy, title) in [
                (5, VisibilityPolicy::Smart, w!("&Smart")),
                (6, VisibilityPolicy::AlwaysVisible, w!("&Always Visible")),
                (7, VisibilityPolicy::DesktopOnly, w!("&Desktop Only")),
            ] {
                AppendMenuW(
                    policy_menu,
                    MF_STRING
                        | if options.policy == policy {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        },
                    id,
                    title,
                )?;
            }
            AppendMenuW(menu, MF_STRING, 8, w!("Se&ttings"))?;
            AppendMenuW(
                menu,
                MF_STRING
                    | if options.selection == ModeSelection::Pinned {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
                11,
                w!("&Pin Current Mode"),
            )?;
            if options.visible {
                AppendMenuW(menu, MF_STRING, 10, w!("&Hide Rimflux"))?;
            }
            AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
            AppendMenuW(menu, MF_STRING, 9, w!("&Quit Rimflux"))?;
            let mut cursor = POINT::default();
            if let Some(position) = MENU_POSITION.take() {
                cursor = position;
            } else {
                GetCursorPos(&mut cursor)?;
            }
            let _ = SetForegroundWindow(hwnd);
            let selected = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                cursor.x,
                cursor.y,
                None,
                hwnd,
                None,
            )
            .0;
            let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            Ok(selected as u32)
        })();
        let _ = DestroyMenu(menu);
        let action = match result.ok()? {
            1 => Action::Restore,
            2 => Action::SwitchMode(NotchMode::Clock),
            3 => Action::SwitchMode(NotchMode::Music),
            4 => Action::SwitchMode(NotchMode::Monitoring),
            5 => Action::SetVisibility(VisibilityPolicy::Smart),
            6 => Action::SetVisibility(VisibilityPolicy::AlwaysVisible),
            7 => Action::SetVisibility(VisibilityPolicy::DesktopOnly),
            8 => Action::OpenPanel(Panel::Settings),
            9 => return Some(Message::Quit),
            10 => Action::Hide(HideFor::UntilShown),
            11 => Action::TogglePin,
            _ => return None,
        };
        Some(Message::DesktopAction(action))
    }
}

fn foreground_window() -> Option<Foreground> {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_invalid() {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(foreground, Some(&mut pid));
        if pid == std::process::id() {
            return None;
        }
        let root = GetAncestor(foreground, GA_ROOTOWNER);
        let hwnd = if root.is_invalid() { foreground } else { root };
        let mut class = [0_u16; 128];
        let count = GetClassNameW(hwnd, &mut class).max(0) as usize;
        let class = String::from_utf16_lossy(&class[..count]);
        if hwnd == GetShellWindow() || matches!(class.as_str(), "Progman" | "WorkerW") {
            return Some(Foreground::Desktop);
        }
        if matches!(
            class.as_str(),
            "Shell_TrayWnd"
                | "Shell_SecondaryTrayWnd"
                | "#32768"
                | "tooltips_class32"
                | "MultitaskingViewFrame"
                | "XamlExplorerHostIslandWindow"
        ) || !IsWindowVisible(hwnd).as_bool()
            || IsIconic(hwnd).as_bool()
        {
            return None;
        }
        let mut cloaked = 0_u32;
        let _ = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), 4);
        if cloaked != 0 {
            return None;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let notch = HWND(NOTCH.load(Ordering::Acquire) as *mut c_void);
        if !notch.is_invalid()
            && IsWindow(Some(notch)).as_bool()
            && MonitorFromWindow(notch, MONITOR_DEFAULTTONEAREST) != monitor
        {
            return Some(Foreground::Window);
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let mut bounds = RECT::default();
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut bounds as *mut RECT).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
        .is_err()
        {
            GetWindowRect(hwnd, &mut bounds).ok()?;
        }
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        let maximized = IsZoomed(hwnd).as_bool()
            || (GetWindowPlacement(hwnd, &mut placement).is_ok()
                && placement.showCmd == SW_SHOWMAXIMIZED.0 as u32);
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let decorated = style & WS_CAPTION.0 == WS_CAPTION.0;
        let edges = |r: RECT| [r.left, r.top, r.right, r.bottom];
        let fullscreen = covers_monitor(edges(bounds), edges(info.rcMonitor), maximized, decorated);
        if !fullscreen
            && !maximized
            && GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0
        {
            return None;
        }
        let executable = if maximized && !fullscreen {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            executable_name(pid).unwrap_or_default()
        } else {
            String::new()
        };
        Some(classify_window(&executable, maximized, fullscreen))
    }
}

fn executable_name(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut path = [0_u16; 1024];
        let mut length = path.len() as u32;
        let read = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        read.ok()?;
        let path = String::from_utf16_lossy(&path[..length as usize]);
        Some(path.rsplit('\\').next()?.to_owned())
    }
}

pub fn set_startup(enabled: bool) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let key = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let name = w!("Rimflux");
    unsafe {
        if !enabled {
            let result = RegDeleteKeyValueW(HKEY_CURRENT_USER, key, name);
            return if result == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                result.ok().map_err(|e| e.to_string())
            };
        }
        let path = std::env::current_exe().map_err(|error| error.to_string())?;
        let mut command = vec![u16::from(b'"')];
        command.extend(path.as_os_str().encode_wide());
        command.extend([u16::from(b'"'), 0]);
        if command.len() > 260 {
            return Err("Executable path exceeds the Windows startup command limit".into());
        }
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            REG_SZ.0,
            Some(command.as_ptr().cast()),
            (command.len() * 2) as u32,
        )
        .ok()
        .map_err(|error| error.to_string())
    }
}
