use rgb_automation::{
    AutomationEvent, AutomationWatcher, RunningAppInfo, list_running_windowed_apps,
};
use rgb_capture::{AMBILIGHT_H, AMBILIGHT_W, AudioSpectrumSampler, ScreenSampler};
use rgb_core::{
    AppConfig, AutomationConfig, DevicePlacement, DeviceSnapshot, EffectKind, EffectParams,
    HubImportReport, Locale, import_local_hub_presets,
};
use rgb_drivers::DeviceManager;
use rgb_engine::{EngineStatus, LightingEngine};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_positioner::{Position, WindowExt};

const POPOVER_LABEL: &str = "tray-popover";
const STUDIO_LABEL: &str = "studio";

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapState {
    pub config: AppConfig,
    pub engine_status: EngineStatus,
    pub devices: Vec<DeviceSnapshot>,
    pub canvas_width: usize,
    pub canvas_height: usize,
    pub canvas_pixels: Vec<u8>,
    pub active_rule_app: Option<String>,
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
    preview_channel: Option<Channel<Vec<u8>>>,
    pre_lock_power: Option<bool>,
    pre_idle_power: Option<bool>,
    pre_rule_profile_id: Option<String>,
    active_rule_app: Option<String>,
    night_mode_active: bool,
    battery_alert_levels: HashMap<String, BatteryAlertLevel>,
}

impl CoreRuntime {
    fn snapshot(&self) -> BootstrapState {
        BootstrapState {
            config: self.config.clone(),
            engine_status: LightingEngine::status_for(&self.config),
            devices: self.devices.snapshots(),
            canvas_width: self.engine.canvas.width,
            canvas_height: self.engine.canvas.height,
            canvas_pixels: self.engine.canvas.pixels.clone(),
            active_rule_app: self.active_rule_app.clone(),
        }
    }

    fn save_config(&self) {
        let _ = self.config.save_atomic(&self.config_path);
    }

    fn effective_config(&self) -> AppConfig {
        let mut eff = self.config.clone();
        if self.night_mode_active && eff.automation.night_mode_enabled {
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
        let placements = eff_cfg.placements.clone();
        for placement in &placements {
            let local_pts = self.devices.local_points_for(&placement.device_id);
            if let Some(colors) =
                self.engine
                    .sample_for_device(&eff_cfg, placement, &local_pts, now, force_hardware)
            {
                self.devices.apply_device_frame(&placement.device_id, &colors);
            }
        }
        if let Some(ch) = &self.preview_channel
            && ch.send(self.engine.canvas.pixels.clone()).is_err()
        {
            self.preview_channel = None;
        }
    }
}

struct SharedState {
    runtime: Arc<Mutex<CoreRuntime>>,
    watcher: AutomationWatcher,
    wake_notify: Arc<tokio::sync::Notify>,
}

impl SharedState {
    fn mutate_and_wake<F>(&self, f: F) -> Result<BootstrapState, String>
    where
        F: FnOnce(&mut CoreRuntime),
    {
        let snap = {
            let mut rt = self.runtime.lock().map_err(|_| "runtime lock poisoned")?;
            f(&mut rt);
            rt.engine.mark_needs_apply();
            rt.apply_tick(true);
            rt.save_config();
            rt.snapshot()
        };
        self.wake_notify.notify_one();
        Ok(snap)
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
    .inner_size(360.0, 480.0)
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
    .inner_size(1160.0, 740.0)
    .min_inner_size(940.0, 620.0)
    .decorations(false)
    .transparent(true)
    .center()
    .visible(false)
    .build()
    .map_err(|e| e.to_string())?;

    apply_win11_mica(&win);
    let _ = win.show();
    let _ = win.set_focus();

    let win_clone = win.clone();
    win.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = win_clone.destroy();
        }
    });

    Ok(())
}

#[tauri::command]
fn get_bootstrap_state(state: State<'_, SharedState>) -> Result<BootstrapState, String> {
    let mut rt = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    rt.devices.poll_telemetry();
    Ok(rt.snapshot())
}

#[tauri::command]
fn set_power(on: bool, state: State<'_, SharedState>) -> Result<BootstrapState, String> {
    state.mutate_and_wake(|rt| {
        rt.config.power_on = on;
        rt.pre_lock_power = None;
        rt.pre_idle_power = None;
    })
}

#[tauri::command]
fn set_brightness(brightness: u8, state: State<'_, SharedState>) -> Result<BootstrapState, String> {
    state.mutate_and_wake(|rt| {
        rt.config.global_brightness = brightness.min(100);
    })
}

#[tauri::command]
fn select_profile(
    profile_id: String,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    state.mutate_and_wake(|rt| {
        if rt.config.profiles.iter().any(|p| p.id == profile_id) {
            rt.config.active_profile_id = profile_id.clone();
            rt.config.power_on = profile_id != "off";
            rt.pre_rule_profile_id = None;
            rt.active_rule_app = None;
        }
    })
}

#[tauri::command]
fn update_effect(
    effect: EffectParams,
    profile_brightness: Option<u8>,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    state.mutate_and_wake(|rt| {
        if let Some(profile) = rt.config.active_profile_mut() {
            profile.effect = effect;
            if let Some(b) = profile_brightness {
                profile.brightness = b.min(100);
            }
        }
        rt.config.power_on = true;
    })
}

#[tauri::command]
fn update_placement(
    placement: DevicePlacement,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    state.mutate_and_wake(|rt| {
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

#[tauri::command]
fn update_aura_led_counts(
    counts: [usize; 2],
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    state.mutate_and_wake(|rt| {
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

#[tauri::command]
fn set_locale(locale: Locale, state: State<'_, SharedState>) -> Result<BootstrapState, String> {
    let mut guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    guard.config.locale = locale;
    guard.save_config();
    Ok(guard.snapshot())
}

#[tauri::command]
fn update_automation(
    app: AppHandle,
    automation: AutomationConfig,
    state: State<'_, SharedState>,
) -> Result<BootstrapState, String> {
    let autolaunch = app.autolaunch();
    if automation.start_with_windows {
        let _ = autolaunch.enable();
    } else {
        let _ = autolaunch.disable();
    }

    state.watcher.update_config(automation.clone());
    let mut guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    guard.config.automation = automation;
    guard.save_config();
    Ok(guard.snapshot())
}

#[tauri::command]
fn list_running_apps() -> Vec<RunningAppInfo> {
    list_running_windowed_apps()
}

#[tauri::command]
fn import_hub_presets(state: State<'_, SharedState>) -> Result<HubImportResponse, String> {
    let report = import_local_hub_presets();
    let mut guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    guard.config.imported_hub_presets = report.items.clone();
    guard.save_config();
    Ok(HubImportResponse {
        state: guard.snapshot(),
        report,
    })
}

#[tauri::command]
fn refresh_devices(state: State<'_, SharedState>) -> Result<BootstrapState, String> {
    state.mutate_and_wake(|rt| {
        rt.devices.reconnect_all();
    })
}

#[tauri::command]
fn subscribe_canvas_preview(
    channel: Channel<Vec<u8>>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut guard = state.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let _ = channel.send(guard.engine.canvas.pixels.clone());
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
    win.destroy().map_err(|e| e.to_string())
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
    wake_notify: Arc<tokio::sync::Notify>,
) {
    tauri::async_runtime::spawn(async move {
        let mut last_telemetry = Instant::now();
        loop {
            let needs_dynamic_loop = {
                let mut rt = match runtime.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                if last_telemetry.elapsed() >= Duration::from_secs(60) {
                    rt.devices.poll_telemetry();
                    update_tray_and_notifications(&app, &tray, &mut rt);
                    last_telemetry = Instant::now();
                }
                if rt.engine.should_tick(&rt.config) {
                    rt.apply_tick(false);
                }
                !matches!(
                    LightingEngine::status_for(&rt.config),
                    EngineStatus::Off | EngineStatus::StaticZeroCpu
                )
            };

            if needs_dynamic_loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(16)) => {}
                    _ = wake_notify.notified() => {}
                }
            } else {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                    _ = wake_notify.notified() => {}
                }
            }
        }
    });
}

fn spawn_automation_loop(
    runtime: Arc<Mutex<CoreRuntime>>,
    wake_notify: Arc<tokio::sync::Notify>,
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
                    rt.apply_tick(true);
                }
            }
            if changed {
                wake_notify.notify_one();
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
        pre_lock_power: None,
        pre_idle_power: None,
        pre_rule_profile_id: None,
        active_rule_app: None,
        night_mode_active: false,
        battery_alert_levels: HashMap::new(),
    }));
    let wake_notify = Arc::new(tokio::sync::Notify::new());
    let (watcher, auto_rx) = AutomationWatcher::start(initial_automation);

    if let Ok(mut rt) = runtime.lock() {
        rt.apply_tick(true);
    }

    let loop_runtime = Arc::clone(&runtime);
    let loop_wake = Arc::clone(&wake_notify);
    let auto_runtime = Arc::clone(&runtime);
    let auto_wake = Arc::clone(&wake_notify);

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
            wake_notify,
        })
        .setup(move |app| {
            let open_item =
                MenuItem::with_id(app, "open_studio", "RGBStuff Studio", true, None::<&str>)?;
            let toggle_item =
                MenuItem::with_id(app, "toggle_power", "Toggle RGB", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &toggle_item, &quit_item])?;

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("RGBStuff")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open_studio" => {
                        let _ = open_or_focus_studio(app);
                    }
                    "toggle_power" => {
                        let state = app.state::<SharedState>();
                        let current = state
                            .runtime
                            .lock()
                            .map(|rt| rt.config.power_on)
                            .unwrap_or(true);
                        let _ = state.mutate_and_wake(|rt| {
                            rt.config.power_on = !current;
                        });
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
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

            spawn_engine_loop(app.handle().clone(), tray, loop_runtime, loop_wake);
            spawn_automation_loop(auto_runtime, auto_wake, auto_rx);

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
            update_automation,
            list_running_apps,
            import_hub_presets,
            refresh_devices,
            subscribe_canvas_preview,
            open_studio,
            window_minimize,
            window_toggle_maximize,
            window_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running RGBStuff");
}
