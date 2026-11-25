use rgb_automation::{
    AutomationEvent, AutomationWatcher, RunningAppInfo, list_running_windowed_apps,
};
use rgb_capture::{AMBILIGHT_H, AMBILIGHT_W, AudioSpectrumSampler, ScreenSampler};
use rgb_core::{
    AppConfig, AutomationConfig, CloseAction, DevicePlacement, DeviceSnapshot, EffectKind,
    EffectParams, HubImportReport, Locale, UiPrefs, import_local_hub_presets,
};
use rgb_drivers::DeviceManager;
use rgb_engine::{EngineStatus, LightingEngine};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, EventTarget, Manager, RunEvent, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent, Wry,
};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_positioner::{Position, WindowExt};

const POPOVER_LABEL: &str = "tray-popover";
const STUDIO_LABEL: &str = "studio";
/// Config writes are coalesced so slider/drag spam does not hit the disk on every event.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);
/// The UI preview only needs ~30 fps; hardware keeps its own per-bus pacing.
const PREVIEW_INTERVAL: Duration = Duration::from_millis(33);
const DYNAMIC_TICK: Duration = Duration::from_millis(16);
/// Pushed to every open window whenever state changes outside that window
/// (tray menu, automations, the other window), so the UI never goes stale.
const STATE_EVENT: &str = "rgb://state";
const TRAY_ID: &str = "rgbstuff-tray";

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapState {
    pub config: AppConfig,
    pub engine_status: EngineStatus,
    pub devices: Vec<DeviceSnapshot>,
    pub canvas_width: usize,
    pub canvas_height: usize,
    pub canvas_pixels: Vec<u8>,
    pub active_rule_app: Option<String>,
    /// Night mode is enabled *and* the clock is inside its window right now.
    pub night_mode_active: bool,
    /// What actually reaches the LEDs: global x profile, capped by night mode.
    pub output_brightness: u8,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HubImportResponse {
    pub state: BootstrapState,
    pub report: HubImportReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BatteryAlertLevel {
    Normal,
    Warn,
    Critical,
}

struct CoreRuntime {
    config_path: PathBuf,
    config: AppConfig,
    engine: LightingEngine,
    devices: DeviceManager,
    screen_sampler: ScreenSampler,
    audio_sampler: AudioSpectrumSampler,
    preview_channel: Option<Channel<InvokeResponseBody>>,
    last_preview: Instant,
    config_dirty_since: Option<Instant>,
    pre_lock_power: Option<bool>,
    pre_idle_power: Option<bool>,
    pre_rule_profile_id: Option<String>,
    active_rule_app: Option<String>,
    night_mode_active: bool,
    battery_alert_levels: HashMap<String, BatteryAlertLevel>,
}

impl CoreRuntime {
    /// `with_pixels` is only needed for the initial bootstrap; afterwards the
    /// preview arrives through the raw-bytes channel.
    fn snapshot(&self, with_pixels: bool) -> BootstrapState {
        BootstrapState {
            config: self.config.clone(),
            engine_status: LightingEngine::status_for(&self.config),
            devices: self.devices.snapshots(),
            canvas_width: self.engine.canvas.width,
            canvas_height: self.engine.canvas.height,
            canvas_pixels: if with_pixels {
                self.engine.canvas.pixels.clone()
            } else {
                Vec::new()
            },
            active_rule_app: self.active_rule_app.clone(),
            night_mode_active: self.night_dimming(),
            output_brightness: LightingEngine::effective_brightness(&self.effective_config()),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    fn night_dimming(&self) -> bool {
        self.night_mode_active && self.config.automation.night_mode_enabled
    }

    fn save_config(&mut self) {
        let _ = self.config.save_atomic(&self.config_path);
        self.config_dirty_since = None;
    }

    fn mark_config_dirty(&mut self) {
        self.config_dirty_since = Some(Instant::now());
    }

    fn effective_config(&self) -> AppConfig {
        let mut eff = self.config.clone();
        if self.night_dimming() {
            eff.global_brightness = eff.global_brightness.min(eff.automation.night_brightness);
        }
        eff
    }

    fn sync_capture_providers(&mut self) {
        let active_effect = self
            .config
            .active_profile()
            .map(|p| p.effect.kind)
            .unwrap_or(EffectKind::Solid);
        let want_ambilight = self.config.power_on && active_effect.needs_screen_capture();
        let want_audio = self.config.power_on && active_effect.needs_audio_capture();

        if want_ambilight {
            if !self.screen_sampler.is_running() {
                self.screen_sampler.start();
            }
        } else if self.screen_sampler.is_running() {
            self.screen_sampler.stop();
        }

        if want_audio {
            if !self.audio_sampler.is_running() {
                self.audio_sampler.start();
            }
        } else if self.audio_sampler.is_running() {
            self.audio_sampler.stop();
        }
    }

    fn apply_tick(&mut self, force_hardware: bool) {
        self.sync_capture_providers();
        let eff_cfg = self.effective_config();

        let screen_buf = if self.screen_sampler.is_running() {
            self.screen_sampler.latest_frame()
        } else {
            None
        };
        let screen_arg = screen_buf
            .as_deref()
            .map(|buf| (AMBILIGHT_W, AMBILIGHT_H, buf));

        let audio_bands = if self.audio_sampler.is_running() {
            self.audio_sampler.latest_bands()
        } else {
            Vec::new()
        };

        self.engine.render_step(&eff_cfg, screen_arg, &audio_bands);

        let now = Instant::now();
        let mut deferred = false;
        for placement in &eff_cfg.placements {
            // A bus that is still inside its pacing window must not be skipped for good
            // on static effects: keep the engine ticking until every device got the frame.
            if !force_hardware && !self.engine.pacer.should_send(&placement.device_id, now) {
                deferred = true;
                continue;
            }
            let local_pts = self.devices.local_points_for(&placement.device_id);
            if let Some(colors) =
                self.engine
                    .sample_for_device(&eff_cfg, placement, &local_pts, now, force_hardware)
            {
                self.devices.apply_device_frame(&placement.device_id, &colors);
            }
        }
        if deferred {
            self.engine.defer_static_apply();
        }

        if self.preview_channel.is_some()
            && (force_hardware || self.last_preview.elapsed() >= PREVIEW_INTERVAL)
        {
            self.last_preview = Instant::now();
            let frame = InvokeResponseBody::Raw(self.engine.canvas.pixels.clone());
            if let Some(ch) = &self.preview_channel
                && ch.send(frame).is_err()
            {
                self.preview_channel = None;
            }
        }
    }
}

struct SharedState {
    runtime: Arc<Mutex<CoreRuntime>>,
    watcher: AutomationWatcher,
    wake: Sender<()>,
}

impl SharedState {
    /// Mutates config and lets the engine thread do the hardware I/O. Never touches
    /// devices or disk here, so callers (UI commands, tray menu) return immediately.
    /// Every other open window gets the new state; `caller` already has it.
    fn mutate_and_wake<F>(
        &self,
        app: &AppHandle,
        caller: Option<&str>,
        f: F,
    ) -> Result<BootstrapState, String>
    where
        F: FnOnce(&mut CoreRuntime),
    {
        let snap = {
            let mut rt = self.runtime.lock().map_err(|_| "runtime lock poisoned")?;
            f(&mut rt);
            rt.engine.mark_needs_apply();
            rt.mark_config_dirty();
            rt.snapshot(false)
        };
        let _ = self.wake.send(());
        broadcast_state(app, &snap, caller);
        Ok(snap)
    }

    /// Config-only change (no lighting re-apply), saved right away.
    fn mutate_and_save<F>(
        &self,
        app: &AppHandle,
        caller: Option<&str>,
        f: F,
    ) -> Result<BootstrapState, String>
    where
        F: FnOnce(&mut CoreRuntime),
    {
        let snap = {
            let mut rt = self.runtime.lock().map_err(|_| "runtime lock poisoned")?;
            f(&mut rt);
            rt.save_config();
            rt.snapshot(false)
        };
        broadcast_state(app, &snap, caller);
        Ok(snap)
    }
}

/// Listeners register per window (`getCurrentWebviewWindow().listen`), so the
/// filter can skip the window that already got the state as a command result.
fn broadcast_state(app: &AppHandle, snap: &BootstrapState, except: Option<&str>) {
    let _ = app.emit_filter(STATE_EVENT, snap, |target| match target {
        EventTarget::WebviewWindow { label }
        | EventTarget::Webview { label }
        | EventTarget::Window { label }
        | EventTarget::AnyLabel { label } => Some(label.as_str()) != except,
        EventTarget::Any | EventTarget::App => except.is_none(),
        _ => false,
    });
}

fn broadcast_from_runtime(app: &AppHandle) {
    let state = app.state::<SharedState>();
    let snap = match state.runtime.lock() {
        Ok(rt) => rt.snapshot(false),
        Err(_) => return,
    };
    broadcast_state(app, &snap, None);
}

fn quit_app(app: &AppHandle) {
    if let Ok(mut rt) = app.state::<SharedState>().runtime.lock()
        && rt.config_dirty_since.is_some()
    {
        rt.save_config();
    }
    app.exit(0);
}

/// Studio close button / Alt+F4: tray (destroying the webview to free WebView2
/// memory) or quit, as chosen in Settings.
fn handle_studio_close(app: &AppHandle) {
    let state = app.state::<SharedState>();
    let (action, show_hint, locale) = match state.runtime.lock() {
        Ok(mut rt) => {
            let action = rt.config.ui.close_action;
            let hint = action == CloseAction::Tray && !rt.config.ui.tray_hint_shown;
            if hint {
                rt.config.ui.tray_hint_shown = true;
                rt.save_config();
            }
            (action, hint, rt.config.locale)
        }
        Err(_) => (CloseAction::Tray, false, Locale::PtBr),
    };

    match action {
        CloseAction::Quit => quit_app(app),
        CloseAction::Tray => {
            if let Some(win) = app.get_webview_window(STUDIO_LABEL) {
                let _ = win.destroy();
            }
            if show_hint {
                let body = match locale {
                    Locale::PtBr => "O RGBStuff continua rodando na bandeja. Clique no ícone para abrir; dá pra mudar isso em Configurações.",
                    Locale::EnUs => "RGBStuff keeps running in the tray. Click the icon to open it; you can change this in Settings.",
                };
                let _ = app.notification().builder().title("RGBStuff").body(body).show();
            }
        }
    }
}

fn apply_win11_mica(window: &WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        let _ = window_vibrancy::apply_mica(window, Some(true));
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

fn toggle_popover(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(POPOVER_LABEL) {
        let _ = win.destroy();
        return;
    }

    let builder = WebviewWindowBuilder::new(
        app,
        POPOVER_LABEL,
        WebviewUrl::App("index.html?window=popover".into()),
    )
    .title("RGBStuff")
    .inner_size(380.0, 520.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false);

    if let Ok(win) = builder.build() {
        apply_win11_mica(&win);
        let _ = win.move_window(Position::TrayCenter);
        let _ = win.show();
        let _ = win.set_focus();

        let win_clone = win.clone();
        win.on_window_event(move |event| {
            if let WindowEvent::Focused(false) = event {
                let _ = win_clone.destroy();
            }
        });
    }
}

fn open_or_focus_studio(app: &AppHandle) -> Result<(), String> {
    if let Some(popover) = app.get_webview_window(POPOVER_LABEL) {
        let _ = popover.destroy();
    }

    if let Some(win) = app.get_webview_window(STUDIO_LABEL) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    let win = WebviewWindowBuilder::new(
        app,
        STUDIO_LABEL,
        WebviewUrl::App("index.html?window=studio".into()),
    )
    .title("RGBStuff Studio")
    .inner_size(1240.0, 780.0)
    .min_inner_size(980.0, 640.0)
    .decorations(false)
    .transparent(true)
    .center()
    .visible(false)
    .build()
    .map_err(|e| e.to_string())?;

    apply_win11_mica(&win);
    let _ = win.show();
    let _ = win.set_focus();

    let handle = app.clone();
    win.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            handle_studio_close(&handle);
        }
    });

    Ok(())
}

#[tauri::command(async)]
fn get_bootstrap_state(state: State<'_, SharedState>) -> Result<BootstrapState, String> {
    let mut rt = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    rt.devices.poll_telemetry();
    Ok(rt.snapshot(true))
}

#[tauri::command(async)]
fn set_power(
    on: bool,
    window: WebviewWindow,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    state.mutate_and_wake(window.app_handle(), Some(window.label()), |rt| {
        rt.config.power_on = on;
        rt.pre_lock_power = None;
        rt.pre_idle_power = None;
    })
}

#[tauri::command(async)]
fn set_brightness(
    brightness: u8,
    window: WebviewWindow,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    state.mutate_and_wake(window.app_handle(), Some(window.label()), |rt| {
        rt.config.global_brightness = brightness.min(100);
    })
}

#[tauri::command(async)]
fn select_profile(
    profile_id: String,
    window: WebviewWindow,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    state.mutate_and_wake(window.app_handle(), Some(window.label()), |rt| {
        apply_profile_selection(rt, &profile_id);
    })
}

fn apply_profile_selection(rt: &mut CoreRuntime, profile_id: &str) {
    if rt.config.profiles.iter().any(|p| p.id == profile_id) {
        rt.config.active_profile_id = profile_id.to_string();
        rt.config.power_on = profile_id != "off";
        rt.pre_rule_profile_id = None;
        rt.active_rule_app = None;
        rt.pre_lock_power = None;
        rt.pre_idle_power = None;
    }
}

#[tauri::command(async)]
fn update_effect(
    effect: EffectParams,
    profile_brightness: Option<u8>,
    window: WebviewWindow,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    state.mutate_and_wake(window.app_handle(), Some(window.label()), |rt| {
        // Never paint over the built-in "Off" profile: picking an effect while
        // it is active switches to the first real profile instead.
        if rt.config.active_profile_id == "off"
            && effect.kind != EffectKind::Off
            && let Some(first) = rt.config.profiles.iter().find(|p| p.id != "off")
        {
            rt.config.active_profile_id = first.id.clone();
        }
        if let Some(profile) = rt.config.active_profile_mut() {
            profile.effect = effect;
            if let Some(b) = profile_brightness {
                profile.brightness = b.min(100);
            }
        }
        rt.config.power_on = true;
    })
}

#[tauri::command(async)]
fn update_placement(
    mut placement: DevicePlacement,
    window: WebviewWindow,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    placement.brightness = placement.brightness.min(200);
    state.mutate_and_wake(window.app_handle(), Some(window.label()), |rt| {
        if let Some(existing) = rt
            .config
            .placements
            .iter_mut()
            .find(|p| p.device_id == placement.device_id)
        {
            *existing = placement;
        } else {
            rt.config.placements.push(placement);
        }
    })
}

#[tauri::command(async)]
fn update_aura_led_counts(
    counts: [usize; 2],
    window: WebviewWindow,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    state.mutate_and_wake(window.app_handle(), Some(window.label()), |rt| {
        let c0 = counts[0].clamp(1, 120);
        let c1 = counts[1].clamp(1, 120);
        rt.config.aura_argb_led_counts = [c0, c1];
        rt.devices.update_aura_led_counts([c0, c1]);
        if let Some(pl) = rt
            .config
            .placements
            .iter_mut()
            .find(|p| p.device_id == "asus-aura-0b05-19af")
        {
            pl.led_count = c0 + c1;
        }
    })
}

#[tauri::command(async)]
fn set_locale(
    locale: Locale,
    window: WebviewWindow,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    let snap = state.mutate_and_save(window.app_handle(), Some(window.label()), |rt| {
        rt.config.locale = locale;
    })?;
    // Tray menu labels follow the UI language.
    if let Some(tray) = window.app_handle().tray_by_id(TRAY_ID)
        && let Ok(menu) = build_tray_menu(window.app_handle(), &snap.config)
    {
        let _ = tray.set_menu(Some(menu));
    }
    Ok(snap)
}

#[tauri::command(async)]
fn update_ui_prefs(
    prefs: UiPrefs,
    window: WebviewWindow,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    state.mutate_and_save(window.app_handle(), Some(window.label()), |rt| {
        rt.config.ui = prefs;
    })
}

#[tauri::command(async)]
fn update_automation(
    automation: AutomationConfig,
    window: WebviewWindow,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    let app = window.app_handle();
    let mut automation = automation;
    let autostart_before = state
        .runtime
        .lock()
        .map(|rt| rt.config.automation.start_with_windows)
        .unwrap_or(!automation.start_with_windows);
    if automation.start_with_windows != autostart_before {
        automation.start_with_windows = sync_autostart(app, automation.start_with_windows);
    }

    state.watcher.update_config(automation.clone());
    // Night mode / brightness caps may change what reaches the LEDs.
    state.mutate_and_wake(app, Some(window.label()), |rt| {
        rt.config.automation = automation;
    })
}

/// Makes the Windows "Run" entry match `want`. Returns what actually stuck, so
/// the toggle never shows "on" when the registry write failed.
fn sync_autostart(app: &AppHandle, want: bool) -> bool {
    let autolaunch = app.autolaunch();
    // Re-enabling also refreshes the path after the installer moved the exe.
    let _ = if want { autolaunch.enable() } else { autolaunch.disable() };
    autolaunch.is_enabled().unwrap_or(want)
}

#[tauri::command(async)]
fn list_running_apps() -> Vec<RunningAppInfo> {
    list_running_windowed_apps()
}

#[tauri::command(async)]
fn import_hub_presets(
    window: WebviewWindow,
    state: State<'_, SharedState>,
) -> Result<HubImportResponse, String> {
    let report = import_local_hub_presets();
    let items = report.items.clone();
    let snap = state.mutate_and_save(window.app_handle(), Some(window.label()), |rt| {
        rt.config.imported_hub_presets = items;
    })?;
    Ok(HubImportResponse { state: snap, report })
}

#[tauri::command(async)]
fn refresh_devices(
    window: WebviewWindow,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    state.mutate_and_wake(window.app_handle(), Some(window.label()), |rt| {
        rt.devices.reconnect_all();
    })
}

#[tauri::command(async)]
fn subscribe_canvas_preview(
    channel: Channel<InvokeResponseBody>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let _ = channel.send(InvokeResponseBody::Raw(guard.engine.canvas.pixels.clone()));
    guard.preview_channel = Some(channel);
    Ok(())
}

#[tauri::command]
fn open_studio(app: AppHandle) -> Result<(), String> {
    open_or_focus_studio(&app)
}

#[tauri::command]
fn window_minimize(win: WebviewWindow) -> Result<(), String> {
    win.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
fn window_toggle_maximize(win: WebviewWindow) -> Result<(), String> {
    if win.is_maximized().unwrap_or(false) {
        win.unmaximize().map_err(|e| e.to_string())
    } else {
        win.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn window_close(win: WebviewWindow) -> Result<(), String> {
    if win.label() == STUDIO_LABEL {
        handle_studio_close(win.app_handle());
        Ok(())
    } else {
        win.destroy().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn quit_app_command(app: AppHandle) {
    quit_app(&app);
}

fn profile_label(locale: Locale, id: &str, fallback: &str) -> String {
    let pt = matches!(locale, Locale::PtBr);
    match id {
        "work" => if pt { "Trabalho" } else { "Work" },
        "gaming" => "Gaming",
        "ambilight" => "Ambilight",
        "audio" => if pt { "Áudio" } else { "Audio" },
        "night" => if pt { "Noturno" } else { "Night" },
        "off" => if pt { "Apagar" } else { "Off" },
        _ => fallback,
    }
    .to_string()
}

fn build_tray_menu(app: &AppHandle, config: &AppConfig) -> tauri::Result<Menu<Wry>> {
    let pt = matches!(config.locale, Locale::PtBr);
    let open_item = MenuItem::with_id(
        app,
        "open_studio",
        if pt { "Abrir RGBStuff Studio" } else { "Open RGBStuff Studio" },
        true,
        None::<&str>,
    )?;
    let toggle_item = MenuItem::with_id(
        app,
        "toggle_power",
        if pt { "Ligar / desligar RGB" } else { "Toggle RGB" },
        true,
        None::<&str>,
    )?;

    let profile_items = config
        .profiles
        .iter()
        .map(|p| {
            MenuItem::with_id(
                app,
                format!("profile:{}", p.id),
                profile_label(config.locale, &p.id, &p.name),
                true,
                None::<&str>,
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let profile_refs: Vec<&dyn IsMenuItem<Wry>> =
        profile_items.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
    let profiles = Submenu::with_items(app, if pt { "Perfis" } else { "Profiles" }, true, &profile_refs)?;

    let brightness_items = [25u8, 50, 75, 100]
        .iter()
        .map(|b| MenuItem::with_id(app, format!("brightness:{b}"), format!("{b}%"), true, None::<&str>))
        .collect::<tauri::Result<Vec<_>>>()?;
    let brightness_refs: Vec<&dyn IsMenuItem<Wry>> =
        brightness_items.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
    let brightness = Submenu::with_items(app, if pt { "Brilho" } else { "Brightness" }, true, &brightness_refs)?;

    let quit_item = MenuItem::with_id(app, "quit", if pt { "Sair" } else { "Quit" }, true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    Menu::with_items(
        app,
        &[&open_item, &sep1, &toggle_item, &profiles, &brightness, &sep2, &quit_item],
    )
}

fn handle_tray_menu(app: &AppHandle, id: &str) {
    let state = app.state::<SharedState>();
    match id {
        "open_studio" => {
            let _ = open_or_focus_studio(app);
        }
        "toggle_power" => {
            let _ = state.mutate_and_wake(app, None, |rt| {
                rt.config.power_on = !rt.config.power_on;
                rt.pre_lock_power = None;
                rt.pre_idle_power = None;
            });
        }
        "quit" => quit_app(app),
        other => {
            if let Some(pid) = other.strip_prefix("profile:") {
                let pid = pid.to_string();
                let _ = state.mutate_and_wake(app, None, |rt| apply_profile_selection(rt, &pid));
            } else if let Some(b) = other
                .strip_prefix("brightness:")
                .and_then(|b| b.parse::<u8>().ok())
            {
                let _ = state.mutate_and_wake(app, None, |rt| {
                    rt.config.global_brightness = b.min(100);
                    rt.config.power_on = true;
                });
            }
        }
    }
}

fn normalize_exe_match(rule_exe: &str, active_exe: &str) -> bool {
    let r = rule_exe.trim().to_ascii_lowercase();
    let a = active_exe.trim().to_ascii_lowercase();
    if r.is_empty() || a.is_empty() {
        return false;
    }
    r == a || format!("{r}.exe") == a
}

fn update_tray_and_notifications(
    app: &AppHandle,
    tray: &TrayIcon,
    rt: &mut CoreRuntime,
) {
    let snapshots = rt.devices.snapshots();
    let mut tooltip_parts = vec!["RGBStuff".to_string()];

    for dev in snapshots {
        if let Some(tel) = dev.telemetry {
            if let Some(pct) = tel.percent {
                tooltip_parts.push(format!("{}: {}%", dev.name, pct));
                let warn = rt.config.automation.low_battery_warn_pct;
                let crit = rt.config.automation.low_battery_crit_pct;
                let new_level = if tel.charging || pct > warn {
                    BatteryAlertLevel::Normal
                } else if pct <= crit {
                    BatteryAlertLevel::Critical
                } else {
                    BatteryAlertLevel::Warn
                };

                let prev_level = rt
                    .battery_alert_levels
                    .get(&dev.id)
                    .copied()
                    .unwrap_or(BatteryAlertLevel::Normal);

                if new_level > prev_level {
                    let body = match rt.config.locale {
                        Locale::PtBr => format!("{} está com {}% de bateria.", dev.name, pct),
                        Locale::EnUs => format!("{} battery is at {}%.", dev.name, pct),
                    };
                    let _ = app.notification().builder().title("RGBStuff").body(body).show();
                }
                rt.battery_alert_levels.insert(dev.id, new_level);
            }
        }
    }

    let _ = tray.set_tooltip(Some(tooltip_parts.join(" · ")));
}

fn spawn_engine_loop(
    app: AppHandle,
    tray: TrayIcon,
    runtime: Arc<Mutex<CoreRuntime>>,
    wake_rx: Receiver<()>,
) {
    // Dedicated OS thread: HID/SMBus writes block, so they must not run on the
    // tokio workers that also serve IPC commands.
    let spawned = std::thread::Builder::new()
        .name("rgb-engine".into())
        .spawn(move || {
            let mut last_telemetry = Instant::now();
            loop {
                let wait = {
                    let mut rt = match runtime.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    if last_telemetry.elapsed() >= Duration::from_secs(60) {
                        rt.devices.poll_telemetry();
                        update_tray_and_notifications(&app, &tray, &mut rt);
                        broadcast_state(&app, &rt.snapshot(false), None);
                        last_telemetry = Instant::now();
                    }
                    if rt.engine.should_tick(&rt.config) {
                        rt.apply_tick(false);
                    }
                    if rt
                        .config_dirty_since
                        .is_some_and(|t| t.elapsed() >= SAVE_DEBOUNCE)
                    {
                        rt.save_config();
                    }

                    let mut wait = if rt.engine.should_tick(&rt.config) {
                        DYNAMIC_TICK
                    } else {
                        Duration::from_secs(60)
                    };
                    if let Some(t) = rt.config_dirty_since {
                        wait = wait.min(SAVE_DEBOUNCE.saturating_sub(t.elapsed()));
                    }
                    wait
                };

                match wake_rx.recv_timeout(wait) {
                    Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
                while wake_rx.try_recv().is_ok() {}
            }
        });
    if let Err(e) = spawned {
        eprintln!("failed to spawn engine thread: {e}");
    }
}

fn spawn_automation_loop(
    app: AppHandle,
    runtime: Arc<Mutex<CoreRuntime>>,
    wake: Sender<()>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<AutomationEvent>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let mut changed = false;
            if let Ok(mut rt) = runtime.lock() {
                match ev {
                    AutomationEvent::SessionLocked => {
                        if rt.config.automation.auto_off_on_lock && rt.pre_lock_power.is_none() {
                            rt.pre_lock_power = Some(rt.config.power_on);
                            rt.config.power_on = false;
                            changed = true;
                        }
                    }
                    AutomationEvent::SessionUnlocked => {
                        if let Some(prev) = rt.pre_lock_power.take() {
                            rt.config.power_on = prev;
                            changed = true;
                        }
                    }
                    AutomationEvent::SystemResumed => {
                        rt.devices.reconnect_all();
                        changed = true;
                    }
                    AutomationEvent::IdleChanged(idle) => {
                        if idle && rt.config.automation.idle_timeout_mins > 0 {
                            if rt.pre_idle_power.is_none() {
                                rt.pre_idle_power = Some(rt.config.power_on);
                                rt.config.power_on = false;
                                changed = true;
                            }
                        } else if !idle {
                            if let Some(prev) = rt.pre_idle_power.take() {
                                rt.config.power_on = prev;
                                changed = true;
                            }
                        }
                    }
                    AutomationEvent::ForegroundAppChanged(exe) => {
                        let matched_rule = rt
                            .config
                            .automation
                            .app_rules
                            .iter()
                            .find(|r| r.enabled && normalize_exe_match(&r.executable, &exe))
                            .cloned();

                        if let Some(rule) = matched_rule {
                            if rt.config.profiles.iter().any(|p| p.id == rule.profile_id) {
                                if rt.pre_rule_profile_id.is_none() {
                                    rt.pre_rule_profile_id = Some(rt.config.active_profile_id.clone());
                                }
                                if rt.config.active_profile_id != rule.profile_id {
                                    rt.config.active_profile_id = rule.profile_id;
                                    changed = true;
                                }
                                rt.active_rule_app = Some(exe);
                            }
                        } else if let Some(prev_profile) = rt.pre_rule_profile_id.take() {
                            rt.active_rule_app = None;
                            if rt.config.active_profile_id != prev_profile {
                                rt.config.active_profile_id = prev_profile;
                                changed = true;
                            }
                        }
                    }
                    AutomationEvent::NightModeChanged(active) => {
                        if rt.night_mode_active != active {
                            rt.night_mode_active = active;
                            changed = true;
                        }
                    }
                }

                if changed {
                    rt.engine.mark_needs_apply();
                }
            }
            if changed {
                let _ = wake.send(());
                broadcast_from_runtime(&app);
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_path =
        AppConfig::default_config_path().unwrap_or_else(|_| PathBuf::from("rgbstuff.toml"));
    let config = AppConfig::load_or_default(&config_path).unwrap_or_default();
    let initial_automation = config.automation.clone();
    let devices = DeviceManager::new(config.aura_argb_led_counts);
    let mut engine = LightingEngine::new();
    engine.render_step(&config, None, &[]);

    let runtime = Arc::new(Mutex::new(CoreRuntime {
        config_path,
        config,
        engine,
        devices,
        screen_sampler: ScreenSampler::new(),
        audio_sampler: AudioSpectrumSampler::new(),
        preview_channel: None,
        last_preview: Instant::now(),
        config_dirty_since: None,
        pre_lock_power: None,
        pre_idle_power: None,
        pre_rule_profile_id: None,
        active_rule_app: None,
        night_mode_active: false,
        battery_alert_levels: HashMap::new(),
    }));
    let (wake_tx, wake_rx) = std::sync::mpsc::channel::<()>();
    let (watcher, auto_rx) = AutomationWatcher::start(initial_automation);

    if let Ok(mut rt) = runtime.lock() {
        rt.apply_tick(true);
    }

    let loop_runtime = Arc::clone(&runtime);
    let auto_runtime = Arc::clone(&runtime);
    let auto_wake = wake_tx.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = open_or_focus_studio(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_positioner::init())
        .manage(SharedState {
            runtime,
            watcher,
            wake: wake_tx,
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            let config_now = {
                let state = handle.state::<SharedState>();
                let mut rt = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
                // The config is the source of truth for "start with Windows".
                let want = rt.config.automation.start_with_windows;
                let actual = sync_autostart(&handle, want);
                if actual != want {
                    rt.config.automation.start_with_windows = actual;
                    rt.save_config();
                }
                rt.config.clone()
            };
            let menu = build_tray_menu(&handle, &config_now)?;

            let tray = TrayIconBuilder::with_id(TRAY_ID)
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("RGBStuff")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| handle_tray_menu(app, event.id.as_ref()))
                .on_tray_icon_event(|tray, event| {
                    let app = tray.app_handle();
                    tauri_plugin_positioner::on_tray_event(app, &event);
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_popover(app);
                    }
                })
                .build(app)?;

            spawn_engine_loop(app.handle().clone(), tray, loop_runtime, wake_rx);
            spawn_automation_loop(app.handle().clone(), auto_runtime, auto_wake, auto_rx);

            let start_minimized = std::env::args().any(|a| a == "--minimized");
            if !start_minimized {
                let _ = open_or_focus_studio(app.handle());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_bootstrap_state,
            set_power,
            set_brightness,
            select_profile,
            update_effect,
            update_placement,
            update_aura_led_counts,
            set_locale,
            update_ui_prefs,
            update_automation,
            list_running_apps,
            import_hub_presets,
            refresh_devices,
            subscribe_canvas_preview,
            open_studio,
            window_minimize,
            window_toggle_maximize,
            window_close,
            quit_app_command,
        ])
        .build(tauri::generate_context!())
        .expect("error while building RGBStuff")
        .run(|_app, event| {
            // Closing the last window must not end the process: RGBStuff lives
            // in the tray. Only an explicit exit (tray "Quit", app.exit) has a code.
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}
