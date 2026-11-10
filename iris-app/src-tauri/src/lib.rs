use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};
use iris_engine::{DisplayEngine, ScheduleConfig};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppStateDto {
    pub is_active: bool,
    pub kelvin: u32,
    pub brightness: f64,
    pub schedule_enabled: bool,
}

pub struct AppState {
    pub engine: Mutex<DisplayEngine>,
    pub schedule: Mutex<ScheduleConfig>,
    pub schedule_enabled: Mutex<bool>,
}

#[tauri::command]
fn get_state(state: State<'_, Arc<AppState>>) -> Result<AppStateDto, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    let schedule_enabled = *state.schedule_enabled.lock().map_err(|e| e.to_string())?;

    Ok(AppStateDto {
        is_active: engine.is_active(),
        kelvin: engine.target_kelvin(),
        brightness: engine.target_brightness(),
        schedule_enabled,
    })
}

#[tauri::command]
fn set_temperature(kelvin: u32, smooth: Option<bool>, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.set_temperature(kelvin, smooth.unwrap_or(true))
}

#[tauri::command]
fn set_brightness(brightness: f64, smooth: Option<bool>, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.set_brightness(brightness, smooth.unwrap_or(true))
}

#[tauri::command]
fn toggle_active(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let mut engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.toggle_active(true)
}

#[tauri::command]
fn set_schedule_mode(enabled: bool, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut sched_guard = state.schedule_enabled.lock().map_err(|e| e.to_string())?;
    *sched_guard = enabled;
    Ok(())
}

#[tauri::command]
fn apply_preset(name: String, state: State<'_, Arc<AppState>>) -> Result<AppStateDto, String> {
    let mut engine = state.engine.lock().map_err(|e| e.to_string())?;
    
    let (kelvin, brightness) = match name.to_lowercase().as_str() {
        "day" => (6500, 1.0),
        "reading" => (4500, 0.90),
        "sunset" => (3400, 0.85),
        "bedtime" => (2200, 0.65),
        "candlelight" => (1800, 0.50),
        _ => (engine.target_kelvin(), engine.target_brightness()),
    };

    engine.set_target(kelvin, brightness, true)?;
    let schedule_enabled = *state.schedule_enabled.lock().map_err(|e| e.to_string())?;

    Ok(AppStateDto {
        is_active: engine.is_active(),
        kelvin,
        brightness,
        schedule_enabled,
    })
}

fn broadcast_state(app: &AppHandle, state: &AppState) {
    if let (Ok(engine), Ok(sched_guard)) = (state.engine.lock(), state.schedule_enabled.lock()) {
        let dto = AppStateDto {
            is_active: engine.is_active(),
            kelvin: engine.target_kelvin(),
            brightness: engine.target_brightness(),
            schedule_enabled: *sched_guard,
        };
        let _ = app.emit("state-changed", &dto);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (hour, minute) = get_local_time();
    let schedule = ScheduleConfig::default();
    let (init_temp, init_bright) = schedule.calculate_targets(hour, minute);

    let mut engine = DisplayEngine::new().expect("Failed to initialize display engine");
    let _ = engine.set_target(init_temp, init_bright, false);

    let app_state = Arc::new(AppState {
        engine: Mutex::new(engine),
        schedule: Mutex::new(schedule),
        schedule_enabled: Mutex::new(true),
    });

    let background_state = Arc::clone(&app_state);

    tauri::Builder::default()
        .manage(Arc::clone(&app_state))
        .setup(move |app| {
            setup_tray(app.handle())?;

            if let Some(window) = app.get_webview_window("main") {
                let win = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win.hide();
                        trim_working_set();
                    }
                });
            }

            let handle_clone = app.handle().clone();
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_secs(30));

                    let schedule_active = match background_state.schedule_enabled.lock() {
                        Ok(g) => *g,
                        Err(_) => continue,
                    };

                    if !schedule_active {
                        continue;
                    }

                    let (h, m) = get_local_time();

                    if let (Ok(mut eng), Ok(sched)) = (background_state.engine.lock(), background_state.schedule.lock()) {
                        if eng.is_active() {
                            let (target_temp, target_bright) = sched.calculate_targets(h, m);
                            if (eng.target_kelvin() as i32 - target_temp as i32).abs() > 50 
                                || (eng.target_brightness() - target_bright).abs() > 0.03 {
                                let _ = eng.set_target(target_temp, target_bright, true);
                                drop(eng);
                                broadcast_state(&handle_clone, &background_state);
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_temperature,
            set_brightness,
            toggle_active,
            set_schedule_mode,
            apply_preset
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Ok(mut engine) = app.state::<Arc<AppState>>().engine.lock() {
                    let _ = engine.restore_initial();
                }
            }
        });
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle_i = MenuItem::with_id(app, "toggle", "Toggle Filter", true, None::<&str>)?;
    let day_i = MenuItem::with_id(app, "day", "Day Mode (6500K)", true, None::<&str>)?;
    let sunset_i = MenuItem::with_id(app, "sunset", "Sunset Mode (3400K)", true, None::<&str>)?;
    let bedtime_i = MenuItem::with_id(app, "bedtime", "Bedtime Mode (2200K)", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit Amber", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&toggle_i, &day_i, &sunset_i, &bedtime_i, &quit_i])?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let state_opt = app.try_state::<Arc<AppState>>();
            match event.id.as_ref() {
                "toggle" => {
                    if let Some(ref state) = state_opt {
                        if let Ok(mut engine) = state.engine.lock() {
                            let _ = engine.toggle_active(true);
                        }
                        broadcast_state(app, state);
                    }
                }
                "day" => {
                    if let Some(ref state) = state_opt {
                        if let Ok(mut engine) = state.engine.lock() {
                            let _ = engine.set_target(6500, 1.0, true);
                        }
                        broadcast_state(app, state);
                    }
                }
                "sunset" => {
                    if let Some(ref state) = state_opt {
                        if let Ok(mut engine) = state.engine.lock() {
                            let _ = engine.set_target(3400, 0.85, true);
                        }
                        broadcast_state(app, state);
                    }
                }
                "bedtime" => {
                    if let Some(ref state) = state_opt {
                        if let Ok(mut engine) = state.engine.lock() {
                            let _ = engine.set_target(2200, 0.65, true);
                        }
                        broadcast_state(app, state);
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn get_local_time() -> (u8, u8) {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct SystemTime {
            year: u16,
            month: u16,
            day_of_week: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            milliseconds: u16,
        }

        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetLocalTime(lpSystemTime: *mut SystemTime);
        }

        let mut st = SystemTime {
            year: 0,
            month: 0,
            day_of_week: 0,
            day: 0,
            hour: 0,
            minute: 0,
            second: 0,
            milliseconds: 0,
        };

        unsafe {
            GetLocalTime(&mut st);
        }

        (st.hour as u8, st.minute as u8)
    }

    #[cfg(not(target_os = "windows"))]
    {
        (12, 0)
    }
}

fn trim_working_set() {
    #[cfg(target_os = "windows")]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn SetProcessWorkingSetSize(
                h_process: *mut std::ffi::c_void,
                min_size: usize,
                max_size: usize,
            ) -> i32;
        }

        unsafe {
            SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
        }
    }
}
