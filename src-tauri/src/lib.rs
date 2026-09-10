mod codex;
#[cfg(not(target_os = "linux"))]
mod codex_host;
mod models;
#[cfg(target_os = "linux")]
mod ubuntu_host;

#[cfg(not(target_os = "linux"))]
use codex_host as platform_host;
#[cfg(target_os = "linux")]
use ubuntu_host as platform_host;

use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

use models::{ProviderSnapshot, WidgetPreferences};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Listener, Manager, Runtime, State, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

pub(crate) struct AppState {
    client: reqwest::Client,
    pub(crate) preferences: Mutex<WidgetPreferences>,
    pub(crate) layout_lock: Mutex<()>,
    preferences_path: PathBuf,
    fetch_lock: tokio::sync::Mutex<()>,
    snapshot_cache: Mutex<Option<(Instant, Vec<ProviderSnapshot>)>>,
}

const TRAY_LANGUAGE_CHANGED_EVENT: &str = "tray-language-changed";
const TRAY_PANEL_VISIBILITY_CHANGED_EVENT: &str = "tray-panel-visibility-changed";

#[cfg(any(target_os = "linux", test))]
fn should_skip_taskbar(tray_available: bool) -> bool {
    tray_available
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrayLabels {
    show_panel: &'static str,
    five_hour: &'static str,
    weekly: &'static str,
    refresh: &'static str,
    language: &'static str,
    autostart: &'static str,
    quit: &'static str,
}

struct TrayMenuItems<R: Runtime> {
    show_panel: CheckMenuItem<R>,
    five_hour: CheckMenuItem<R>,
    weekly: CheckMenuItem<R>,
    refresh: MenuItem<R>,
    language: MenuItem<R>,
    autostart: CheckMenuItem<R>,
    quit: MenuItem<R>,
}

fn tray_labels(language: &str) -> TrayLabels {
    if language == "en" {
        TrayLabels {
            show_panel: "Show quota panel",
            five_hour: "5-hour quota",
            weekly: "Weekly quota",
            refresh: "Refresh now",
            language: "Switch to Chinese",
            autostart: "Start at login",
            quit: "Quit",
        }
    } else {
        TrayLabels {
            show_panel: "显示额度面板",
            five_hour: "5 小时额度",
            weekly: "一周额度",
            refresh: "立即刷新",
            language: "切换到英文",
            autostart: "开机启动",
            quit: "退出",
        }
    }
}

fn apply_tray_labels<R: Runtime>(
    labels: TrayLabels,
    items: &TrayMenuItems<R>,
) -> tauri::Result<()> {
    items.show_panel.set_text(labels.show_panel)?;
    items.five_hour.set_text(labels.five_hour)?;
    items.weekly.set_text(labels.weekly)?;
    items.refresh.set_text(labels.refresh)?;
    items.language.set_text(labels.language)?;
    items.autostart.set_text(labels.autostart)?;
    items.quit.set_text(labels.quit)?;
    Ok(())
}

async fn fetch_snapshots_uncached(state: &State<'_, AppState>) -> Vec<ProviderSnapshot> {
    let _guard = state.fetch_lock.lock().await;
    let values = vec![codex::fetch_snapshot(&state.client).await];
    if let Ok(mut cache) = state.snapshot_cache.lock() {
        *cache = Some((Instant::now(), values.clone()));
    }
    values
}

fn load_preferences(path: &PathBuf) -> WidgetPreferences {
    let parse = |candidate: &PathBuf| {
        fs::read_to_string(candidate)
            .ok()
            .and_then(|raw| serde_json::from_str::<WidgetPreferences>(&raw).ok())
    };
    if let Some(value) = parse(path) {
        return value.normalized();
    }
    let backup = path.with_extension("json.bak");
    if let Some(value) = parse(&backup) {
        eprintln!("preferences recovered from backup");
        return value.normalized();
    }
    WidgetPreferences::default()
}

fn persist_preferences(path: &PathBuf, value: &WidgetPreferences) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|_| "failed to create settings directory".to_string())?;
    }
    let serialized =
        serde_json::to_vec_pretty(value).map_err(|_| "failed to serialize settings".to_string())?;
    let temporary = path.with_extension("json.tmp");
    let backup = path.with_extension("json.bak");
    let mut file = fs::File::create(&temporary)
        .map_err(|_| "failed to create temporary settings file".to_string())?;
    file.write_all(&serialized)
        .and_then(|_| file.sync_all())
        .map_err(|_| "failed to write settings".to_string())?;
    if path.exists() {
        let _ = fs::remove_file(&backup);
        fs::rename(path, &backup).map_err(|_| "failed to back up settings".to_string())?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::rename(&backup, path);
        return Err(format!("failed to commit settings: {error}"));
    }
    Ok(())
}

#[tauri::command]
async fn get_snapshots(state: State<'_, AppState>) -> Result<Vec<ProviderSnapshot>, String> {
    const CACHE_TTL: Duration = Duration::from_secs(30);
    if let Ok(cache) = state.snapshot_cache.lock() {
        if let Some((time, values)) = &*cache {
            if time.elapsed() < CACHE_TTL {
                return Ok(values.clone());
            }
        }
    }
    let _guard = match state.fetch_lock.try_lock() {
        Ok(guard) => guard,
        Err(_) => {
            if let Ok(cache) = state.snapshot_cache.lock() {
                if let Some((_, values)) = &*cache {
                    return Ok(values.clone());
                }
            }
            return Ok(vec![ProviderSnapshot::failure(
                "unavailable",
                "Quota refresh is already running.",
            )]);
        }
    };
    if let Ok(cache) = state.snapshot_cache.lock() {
        if let Some((time, values)) = &*cache {
            if time.elapsed() < CACHE_TTL {
                return Ok(values.clone());
            }
        }
    }
    let values = vec![codex::fetch_snapshot(&state.client).await];
    if let Ok(mut cache) = state.snapshot_cache.lock() {
        *cache = Some((Instant::now(), values.clone()));
    }
    Ok(values)
}

#[tauri::command]
async fn refresh_snapshots(state: State<'_, AppState>) -> Result<Vec<ProviderSnapshot>, String> {
    Ok(fetch_snapshots_uncached(&state).await)
}

#[tauri::command]
fn get_preferences(state: State<'_, AppState>) -> Result<WidgetPreferences, String> {
    state
        .preferences
        .lock()
        .map(|value| value.clone())
        .map_err(|_| "settings unavailable".into())
}

#[tauri::command]
fn set_preferences(
    preferences: WidgetPreferences,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut current = state
        .preferences
        .lock()
        .map_err(|_| "settings unavailable".to_string())?;
    let mut preferences = preferences.normalized();
    // Panel size and quota selection are native transactions. Generic settings
    // saves must not overwrite a concurrent native toggle.
    preferences.expanded = current.expanded;
    preferences.quota_window = current.quota_window.clone();
    let language_changed = preferences.language != current.language;
    persist_preferences(&state.preferences_path, &preferences)?;
    *current = preferences.clone();
    drop(current);
    let _ = app.emit_to("widget", "preferences-changed", preferences.clone());
    if language_changed {
        let _ = app.emit(TRAY_LANGUAGE_CHANGED_EVENT, preferences.language);
    }
    Ok(())
}

#[tauri::command]
fn set_widget_expanded(
    expanded: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WidgetPreferences, String> {
    let _layout_guard = state
        .layout_lock
        .lock()
        .map_err(|_| "window layout unavailable".to_string())?;
    // Do not keep the preferences mutex while sending GTK a size request.
    // On Wayland, configure notifications can re-enter application code; the
    // previous implementation held this lock across resize + move and could
    // leave the one-shot resize command waiting indefinitely.
    let previous = state
        .preferences
        .lock()
        .map_err(|_| "settings unavailable".to_string())?
        .clone();
    if previous.expanded == expanded {
        platform_host::apply_expanded(&app, expanded)?;
        return Ok(previous);
    }

    platform_host::apply_expanded(&app, expanded)?;
    let mut preferences = state
        .preferences
        .lock()
        .map_err(|_| "settings unavailable".to_string())?;
    let mut next = preferences.clone();
    next.expanded = expanded;
    if persist_preferences(&state.preferences_path, &next).is_err() {
        return match platform_host::apply_expanded(&app, previous.expanded) {
            Ok(()) => Err("failed to save panel size; previous layout restored".to_string()),
            Err(_) => Err(
                "failed to save panel size and restore the previous layout; reopen the widget"
                    .to_string(),
            ),
        };
    }

    *preferences = next.clone();
    drop(preferences);
    let _ = app.emit_to("widget", "preferences-changed", next.clone());
    Ok(next)
}

fn apply_panel_visibility(app: &AppHandle, visible: bool) {
    if let Some(window) = app.get_webview_window("widget") {
        if visible {
            #[cfg(not(windows))]
            let _ = window.show();
        } else {
            let _ = window.hide();
        }
    }
}

fn update_panel_visibility(app: &AppHandle, visible: bool) -> Result<WidgetPreferences, String> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "settings unavailable".to_string())?;
    let mut preferences = state
        .preferences
        .lock()
        .map_err(|_| "settings unavailable".to_string())?;
    let previous = preferences.clone();
    let mut next = previous.clone();
    next.panel_visible = visible;
    persist_preferences(&state.preferences_path, &next)?;
    *preferences = next.clone();
    drop(preferences);
    apply_panel_visibility(app, visible);
    let _ = app.emit_to("widget", "preferences-changed", next.clone());
    let _ = app.emit(TRAY_PANEL_VISIBILITY_CHANGED_EVENT, visible);
    Ok(next)
}

#[tauri::command]
fn set_panel_visible(visible: bool, app: AppHandle) -> Result<WidgetPreferences, String> {
    update_panel_visibility(&app, visible)
}

fn persist_quota_window(
    path: &PathBuf,
    preferences: &mut WidgetPreferences,
    quota_window: &str,
) -> Result<WidgetPreferences, String> {
    if !matches!(quota_window, "fiveHour" | "weekly") {
        return Err("unsupported quota window".into());
    }
    if preferences.quota_window == quota_window {
        return Ok(preferences.clone());
    }
    let mut next = preferences.clone();
    next.quota_window = quota_window.into();
    persist_preferences(path, &next)?;
    *preferences = next.clone();
    Ok(next)
}

fn update_quota_window(app: &AppHandle, quota_window: &str) -> Result<WidgetPreferences, String> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "settings unavailable".to_string())?;
    let mut preferences = state
        .preferences
        .lock()
        .map_err(|_| "settings unavailable".to_string())?;
    let next = persist_quota_window(&state.preferences_path, &mut preferences, quota_window)?;
    drop(preferences);
    let _ = app.emit_to("widget", "preferences-changed", next.clone());
    Ok(next)
}

#[tauri::command]
fn set_quota_window(quota_window: String, app: AppHandle) -> Result<WidgetPreferences, String> {
    update_quota_window(&app, &quota_window)
}

#[tauri::command]
fn set_widget_always_on_top(
    always_on_top: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WidgetPreferences, String> {
    let previous = state
        .preferences
        .lock()
        .map_err(|_| "settings unavailable".to_string())?
        .clone();
    let mut next = previous.clone();
    next.always_on_top = always_on_top;
    persist_preferences(&state.preferences_path, &next)?;
    let window = app
        .get_webview_window("widget")
        .ok_or_else(|| "widget window missing".to_string())?;
    if let Err(error) = window.set_always_on_top(always_on_top) {
        let _ = persist_preferences(&state.preferences_path, &previous);
        return Err(format!("failed to toggle always-on-top: {error}"));
    }
    *state
        .preferences
        .lock()
        .map_err(|_| "settings unavailable".to_string())? = next.clone();
    let _ = app.emit_to("widget", "preferences-changed", next.clone());
    Ok(next)
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let (panel_visible, initial_language, initial_quota_window) = app
        .state::<AppState>()
        .preferences
        .lock()
        .map(|prefs| {
            (
                prefs.panel_visible,
                prefs.language.clone(),
                prefs.quota_window.clone(),
            )
        })
        .unwrap_or_else(|_| (true, "zh-CN".into(), "weekly".into()));
    let labels = tray_labels(&initial_language);
    let show_panel = CheckMenuItem::with_id(
        app,
        "show_panel",
        labels.show_panel,
        true,
        panel_visible,
        None::<&str>,
    )?;
    let five_hour = CheckMenuItem::with_id(
        app,
        "quota_five_hour",
        labels.five_hour,
        true,
        initial_quota_window == "fiveHour",
        None::<&str>,
    )?;
    let weekly = CheckMenuItem::with_id(
        app,
        "quota_weekly",
        labels.weekly,
        true,
        initial_quota_window == "weekly",
        None::<&str>,
    )?;
    let refresh = MenuItem::with_id(app, "refresh", labels.refresh, true, None::<&str>)?;
    let language = MenuItem::with_id(app, "language", labels.language, true, None::<&str>)?;
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        labels.autostart,
        true,
        autostart_enabled,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show_panel,
            &five_hour,
            &weekly,
            &refresh,
            &language,
            &autostart,
            &quit,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("Quota Float");
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let autostart_menu = autostart.clone();
    let show_panel_menu = show_panel.clone();
    let show_panel_click_menu = show_panel.clone();
    let five_hour_menu = five_hour.clone();
    let weekly_menu = weekly.clone();
    let label_items = TrayMenuItems {
        show_panel: show_panel.clone(),
        five_hour: five_hour.clone(),
        weekly: weekly.clone(),
        refresh: refresh.clone(),
        language: language.clone(),
        autostart: autostart.clone(),
        quit: quit.clone(),
    };
    app.listen(TRAY_LANGUAGE_CHANGED_EVENT, move |event| {
        let Ok(language) = serde_json::from_str::<String>(event.payload()) else {
            return;
        };
        if apply_tray_labels(tray_labels(&language), &label_items).is_err() {
            eprintln!("tray language update failed");
        }
    });
    let show_panel_event_menu = show_panel.clone();
    app.listen(TRAY_PANEL_VISIBILITY_CHANGED_EVENT, move |event| {
        let Ok(visible) = serde_json::from_str::<bool>(event.payload()) else {
            return;
        };
        if show_panel_event_menu.set_checked(visible).is_err() {
            eprintln!("tray panel visibility update failed");
        }
    });
    builder
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show_panel" => {
                let visible = app
                    .try_state::<AppState>()
                    .and_then(|state| {
                        state
                            .preferences
                            .lock()
                            .ok()
                            .map(|prefs| !prefs.panel_visible)
                    })
                    .unwrap_or(true);
                match update_panel_visibility(app, visible) {
                    Ok(_) => {
                        let _ = show_panel_menu.set_checked(visible);
                    }
                    Err(_) => eprintln!("panel visibility update failed"),
                }
            }
            "quota_five_hour" | "quota_weekly" => {
                let selected = if event.id.as_ref() == "quota_five_hour" {
                    "fiveHour"
                } else {
                    "weekly"
                };
                let quota_window = match update_quota_window(app, selected) {
                    Ok(updated) => updated.quota_window,
                    Err(_) => {
                        eprintln!("quota window update failed");
                        app.try_state::<AppState>()
                            .and_then(|state| {
                                state
                                    .preferences
                                    .lock()
                                    .ok()
                                    .map(|prefs| prefs.quota_window.clone())
                            })
                            .unwrap_or_else(|| "weekly".into())
                    }
                };
                // Native check items toggle themselves. Restore the saved
                // selection even after a repeated click or a failed save.
                let _ = five_hour_menu.set_checked(quota_window == "fiveHour");
                let _ = weekly_menu.set_checked(quota_window == "weekly");
            }
            "refresh" => {
                let _ = app.emit_to("widget", "refresh-requested", ());
            }
            "language" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let updated = state.preferences.lock().ok().and_then(|mut prefs| {
                        let mut next = prefs.clone();
                        next.language = if next.language == "en" {
                            "zh-CN".into()
                        } else {
                            "en".into()
                        };
                        let next = next.normalized();
                        persist_preferences(&state.preferences_path, &next).ok()?;
                        *prefs = next.clone();
                        Some(next)
                    });
                    if let Some(updated) = updated {
                        let _ = app.emit(TRAY_LANGUAGE_CHANGED_EVENT, updated.language.clone());
                        let _ = app.emit_to("widget", "preferences-changed", updated);
                    } else {
                        eprintln!("language update failed");
                    }
                }
            }
            "autostart" => {
                let manager = app.autolaunch();
                let enabled = manager.is_enabled().unwrap_or(false);
                let result = if enabled {
                    manager.disable()
                } else {
                    manager.enable()
                };
                match result {
                    Ok(()) => {
                        let _ = autostart_menu.set_checked(!enabled);
                    }
                    Err(_) => eprintln!("autostart update failed"),
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if update_panel_visibility(tray.app_handle(), true).is_ok() {
                    let _ = show_panel_click_menu.set_checked(true);
                }
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let _ = update_panel_visibility(app, true);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let data_dir = app.path().app_config_dir()?;
            let preferences_path = data_dir.join("preferences.json");
            let preferences = load_preferences(&preferences_path);
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(12))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent(concat!("QuotaFloat/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("static HTTP client configuration must be valid");
            app.manage(AppState {
                client,
                preferences: Mutex::new(preferences.clone()),
                layout_lock: Mutex::new(()),
                preferences_path,
                fetch_lock: tokio::sync::Mutex::new(()),
                snapshot_cache: Mutex::new(None),
            });
            let tray_available = setup_tray(app).is_ok();
            if !tray_available {
                eprintln!("tray setup failed; enabling taskbar fallback");
                if let Some(window) = app.get_webview_window("widget") {
                    let _ = window.set_skip_taskbar(false);
                }
            }
            if let Some(window) = app.get_webview_window("widget") {
                let _ = window.set_always_on_top(false);
                #[cfg(target_os = "linux")]
                {
                    // The Shell extension owns stacking and workspace affinity so
                    // the widget stays with ChatGPT instead of becoming a desktop
                    // overlay when another application is focused.
                    let _ = window.set_skip_taskbar(should_skip_taskbar(tray_available));
                    let _ = window.set_visible_on_all_workspaces(false);
                }
            }
            if platform_host::apply_expanded(app.handle(), preferences.expanded).is_err() {
                eprintln!("initial widget layout failed");
            }
            apply_panel_visibility(app.handle(), preferences.panel_visible);
            platform_host::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshots,
            refresh_snapshots,
            get_preferences,
            set_preferences,
            set_widget_expanded,
            set_quota_window,
            set_panel_visible,
            set_widget_always_on_top
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if update_panel_visibility(window.app_handle(), false).is_err() {
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build Quota Float");
    app.run(|app_handle, event| match event {
        tauri::RunEvent::Resumed => {
            let _ = app_handle.emit_to("widget", "refresh-requested", ());
        }
        #[cfg(target_os = "linux")]
        tauri::RunEvent::Exit => ubuntu_host::stop(),
        _ => {}
    });
}

#[cfg(test)]
mod tray_tests {
    use super::{
        load_preferences, persist_quota_window, should_skip_taskbar, tray_labels, TrayLabels,
    };
    use crate::models::WidgetPreferences;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn quota_selection_survives_restart_and_repeated_clicks() {
        let directory = std::env::temp_dir().join(format!(
            "quota-float-selection-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("preferences.json");
        let mut preferences = WidgetPreferences {
            expanded: false,
            language: "en".into(),
            ..WidgetPreferences::default()
        };
        persist_quota_window(&path, &mut preferences, "fiveHour").unwrap();
        let mut restored = load_preferences(&path);
        assert_eq!(restored.quota_window, "fiveHour");
        assert!(!restored.expanded);
        assert_eq!(restored.language, "en");

        persist_quota_window(&path, &mut restored, "fiveHour").unwrap();
        assert_eq!(restored.quota_window, "fiveHour");
        persist_quota_window(&path, &mut restored, "weekly").unwrap();
        assert_eq!(load_preferences(&path).quota_window, "weekly");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn failed_quota_selection_keeps_previous_preferences() {
        let path = std::env::temp_dir().join(format!(
            "quota-float-selection-blocked-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, "blocks the settings directory").unwrap();
        let mut preferences = WidgetPreferences::default();
        assert!(
            persist_quota_window(&path.join("preferences.json"), &mut preferences, "fiveHour")
                .is_err()
        );
        assert_eq!(preferences.quota_window, "weekly");
        assert!(persist_quota_window(&path, &mut preferences, "invalid").is_err());
        assert_eq!(preferences.quota_window, "weekly");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "blocks the settings directory"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn keeps_taskbar_entry_when_tray_setup_fails() {
        assert!(!should_skip_taskbar(false));
        assert!(should_skip_taskbar(true));
    }

    #[test]
    fn returns_english_tray_labels() {
        assert_eq!(
            tray_labels("en"),
            TrayLabels {
                show_panel: "Show quota panel",
                five_hour: "5-hour quota",
                weekly: "Weekly quota",
                refresh: "Refresh now",
                language: "Switch to Chinese",
                autostart: "Start at login",
                quit: "Quit",
            }
        );
    }

    #[test]
    fn returns_chinese_tray_labels_and_uses_them_as_the_fallback() {
        let expected = TrayLabels {
            show_panel: "显示额度面板",
            five_hour: "5 小时额度",
            weekly: "一周额度",
            refresh: "立即刷新",
            language: "切换到英文",
            autostart: "开机启动",
            quit: "退出",
        };
        assert_eq!(tray_labels("zh-CN"), expected);
        assert_eq!(tray_labels("unsupported"), expected);
    }
}
