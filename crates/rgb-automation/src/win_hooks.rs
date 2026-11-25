use crate::schedule::{current_local_minutes, is_in_night_window};
use rgb_core::AutomationConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::sync::{Arc, RwLock, mpsc as std_mpsc};
use std::thread::JoinHandle;
use tokio::sync::mpsc;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    EnumWindows, GWLP_USERDATA, GetForegroundWindow, GetMessageW, GetWindowLongPtrW,
    GetWindowTextW, GetWindowThreadProcessId, HWND_MESSAGE, IsWindowVisible, KillTimer, MSG,
    PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PostMessageW, PostQuitMessage, RegisterClassW,
    SetTimer, SetWindowLongPtrW, TranslateMessage, UnregisterClassW, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_DESTROY, WM_NCCREATE, WM_NCDESTROY, WM_POWERBROADCAST, WM_TIMER,
    WM_WTSSESSION_CHANGE, WNDCLASSW, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
};
use windows::core::{BOOL, PCWSTR, PWSTR, w};

const TIMER_ID: usize = 1;
const POLL_INTERVAL_MS: u32 = 2_000;

/// Events emitted by the Windows automation watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationEvent {
    SessionLocked,
    SessionUnlocked,
    SystemResumed,
    IdleChanged(bool),
    ForegroundAppChanged(String),
    NightModeChanged(bool),
}

/// Watches Windows session, power, idle, foreground application, and schedule state
/// from a dedicated hidden message-only window thread.
#[derive(Debug)]
pub struct AutomationWatcher {
    hwnd: isize,
    config: Arc<RwLock<AutomationConfig>>,
    thread: Option<JoinHandle<()>>,
}

impl AutomationWatcher {
    /// Starts the background message-only window watcher with `initial_config`.
    pub fn start(
        initial_config: AutomationConfig,
    ) -> (Self, mpsc::UnboundedReceiver<AutomationEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let config = Arc::new(RwLock::new(initial_config));
        let config_worker = Arc::clone(&config);
        let (ready_tx, ready_rx) = std_mpsc::sync_channel::<isize>(1);

        let thread = std::thread::Builder::new()
            .name("rgb-automation-win".to_string())
            .spawn(move || run_message_loop(config_worker, tx, ready_tx))
            .expect("failed to spawn rgb-automation window thread");

        let hwnd = ready_rx.recv().unwrap_or(0);

        (
            Self {
                hwnd,
                config,
                thread: Some(thread),
            },
            rx,
        )
    }

    /// Updates the live automation configuration used by the watcher loop.
    pub fn update_config(&self, config: AutomationConfig) {
        let mut guard = self.config.write().unwrap_or_else(|e| e.into_inner());
        *guard = config;
    }

    /// Stops the background message loop and joins the watcher thread.
    pub fn stop(&mut self) {
        if self.hwnd != 0 {
            let hwnd = HWND(self.hwnd as *mut c_void);
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            self.hwnd = 0;
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AutomationWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Returns `true` for OS overlays and RGBStuff itself so transient popups
/// never override an active per-app profile rule (e.g. Feathertune or a game).
pub fn is_ignored_transient_exe(exe: &str) -> bool {
    matches!(
        exe.to_ascii_lowercase().as_str(),
        "rgbstuff-desktop.exe"
            | "shellexperiencehost.exe"
            | "searchhost.exe"
            | "startmenuexperiencehost.exe"
            | "lockapp.exe"
            | "textinputhost.exe"
            | "applicationframehost.exe"
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunningAppInfo {
    pub executable: String,
    pub title: String,
}

/// Lists visible top-level application executables currently open on Windows.
pub fn list_running_windowed_apps() -> Vec<RunningAppInfo> {
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    let ptr = &mut map as *mut BTreeMap<String, String>;

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return BOOL(1);
        }
        let mut title_buf = [0u16; 256];
        let title_len = unsafe { GetWindowTextW(hwnd, &mut title_buf) };
        if title_len <= 0 {
            return BOOL(1);
        }
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize])
            .trim()
            .to_string();
        if title.is_empty() {
            return BOOL(1);
        }

        if let Some(exe) = query_exe_for_hwnd(hwnd) {
            if !is_ignored_transient_exe(&exe) {
                let map = unsafe { &mut *(lparam.0 as *mut BTreeMap<String, String>) };
                map.entry(exe).or_insert(title);
            }
        }
        BOOL(1)
    }

    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(ptr as isize));
    }

    map.into_iter()
        .map(|(executable, title)| RunningAppInfo { executable, title })
        .collect()
}

/// Extracts the lowercase executable filename from a Windows or POSIX path.
pub fn extract_exe_basename(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('\0').trim();
    if trimmed.is_empty() {
        return None;
    }
    let name = trimmed.rsplit(['\\', '/']).next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_ascii_lowercase())
    }
}

struct WindowState {
    config: Arc<RwLock<AutomationConfig>>,
    tx: mpsc::UnboundedSender<AutomationEvent>,
    last_idle: bool,
    last_foreground_exe: Option<String>,
    last_night_mode: bool,
}

impl WindowState {
    fn poll(&mut self) {
        let cfg = self
            .config
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        let is_idle = is_user_idle(cfg.idle_timeout_mins);
        if is_idle != self.last_idle {
            self.last_idle = is_idle;
            let _ = self.tx.send(AutomationEvent::IdleChanged(is_idle));
        }

        if let Some(exe) = query_foreground_exe() {
            if !is_ignored_transient_exe(&exe)
                && self.last_foreground_exe.as_deref() != Some(exe.as_str())
            {
                self.last_foreground_exe = Some(exe.clone());
                let _ = self.tx.send(AutomationEvent::ForegroundAppChanged(exe));
            }
        }

        let is_night = cfg.night_mode_enabled
            && is_in_night_window(&cfg.night_start, &cfg.night_end, current_local_minutes());
        if is_night != self.last_night_mode {
            self.last_night_mode = is_night;
            let _ = self.tx.send(AutomationEvent::NightModeChanged(is_night));
        }
    }
}

struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn is_user_idle(idle_timeout_mins: u16) -> bool {
    if idle_timeout_mins == 0 {
        return false;
    }

    let mut lii = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    if !unsafe { GetLastInputInfo(&mut lii) }.as_bool() {
        return false;
    }

    let now_tick = unsafe { GetTickCount() };
    let elapsed_ms = now_tick.wrapping_sub(lii.dwTime);
    let threshold_ms = u32::from(idle_timeout_mins).saturating_mul(60_000);
    elapsed_ms >= threshold_ms
}

fn query_foreground_exe() -> Option<String> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return None;
    }
    query_exe_for_hwnd(hwnd)
}

fn query_exe_for_hwnd(hwnd: HWND) -> Option<String> {
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }

    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let guard = HandleGuard(process);

    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            guard.0,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    }
    .ok()?;

    let full_path = String::from_utf16_lossy(&buf[..len as usize]);
    extract_exe_basename(&full_path)
}

fn run_message_loop(
    config: Arc<RwLock<AutomationConfig>>,
    tx: mpsc::UnboundedSender<AutomationEvent>,
    ready_tx: std_mpsc::SyncSender<isize>,
) {
    let class_name = w!("RgbStuffAutomationWatcherWindow");
    let hmodule = unsafe { GetModuleHandleW(PCWSTR::null()) }.unwrap_or_default();
    let hinstance = HINSTANCE(hmodule.0);

    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        lpszClassName: class_name,
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&wc);
    }

    let state = Box::new(WindowState {
        config,
        tx,
        last_idle: false,
        last_foreground_exe: None,
        last_night_mode: false,
    });
    let state_ptr = Box::into_raw(state);

    let hwnd_res = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("RGBStuff Automation"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(hinstance),
            Some(state_ptr as *const c_void),
        )
    };

    let Ok(hwnd) = hwnd_res else {
        unsafe {
            drop(Box::from_raw(state_ptr));
        }
        let _ = ready_tx.send(0);
        return;
    };

    unsafe {
        let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
        SetTimer(Some(hwnd), TIMER_ID, POLL_INTERVAL_MS, None);
    }

    let _ = ready_tx.send(hwnd.0 as isize);

    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if ret.0 <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    unsafe {
        let _ = UnregisterClassW(class_name, Some(hinstance));
    }
}

unsafe fn window_state_mut<'a>(hwnd: HWND) -> Option<&'a mut WindowState> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowState;
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &mut *ptr })
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = lparam.0 as *const CREATESTRUCTW;
            if !cs.is_null() {
                let state_ptr = unsafe { (*cs).lpCreateParams } as isize;
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr) };
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_WTSSESSION_CHANGE => {
            if let Some(state) = unsafe { window_state_mut(hwnd) } {
                match wparam.0 as u32 {
                    WTS_SESSION_LOCK => {
                        let _ = state.tx.send(AutomationEvent::SessionLocked);
                    }
                    WTS_SESSION_UNLOCK => {
                        let _ = state.tx.send(AutomationEvent::SessionUnlocked);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            if let Some(state) = unsafe { window_state_mut(hwnd) } {
                match wparam.0 as u32 {
                    PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
                        let _ = state.tx.send(AutomationEvent::SystemResumed);
                    }
                    _ => {}
                }
            }
            LRESULT(1)
        }
        WM_TIMER => {
            if wparam.0 == TIMER_ID {
                if let Some(state) = unsafe { window_state_mut(hwnd) } {
                    state.poll();
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                let _ = KillTimer(Some(hwnd), TIMER_ID);
                let _ = WTSUnRegisterSessionNotification(hwnd);
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let ptr = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            if ptr != 0 {
                unsafe {
                    drop(Box::from_raw(ptr as *mut WindowState));
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::extract_exe_basename;

    #[test]
    fn extracts_lowercase_exe_basename() {
        assert_eq!(
            extract_exe_basename(r"C:\Games\CS2\cs2.exe"),
            Some("cs2.exe".into())
        );
        assert_eq!(
            extract_exe_basename(r"C:\Program Files\VideoLAN\VLC\VLC.EXE"),
            Some("vlc.exe".into())
        );
        assert_eq!(
            extract_exe_basename("notepad.exe"),
            Some("notepad.exe".into())
        );
        assert_eq!(extract_exe_basename(r"C:\Games\"), None);
        assert_eq!(extract_exe_basename(""), None);
    }
}
