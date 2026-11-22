use std::{mem::size_of, path::Path, thread, time::Duration};

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::{CloseHandle, HWND, RECT},
        Graphics::{
            Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
            Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST},
        },
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::WindowsAndMessaging::{
            GetClassNameW, GetForegroundWindow, GetShellWindow, GetWindowRect,
            GetWindowThreadProcessId,
        },
    },
};

use super::{set_click_through, show, IntegrationState, WindowMode};

const SHORTCUTS: [(&str, &str, Option<&str>); 5] = [
    ("Ctrl+Alt+Shift+T", "Show or hide", None),
    ("Ctrl+Alt+Shift+N", "New task", Some("kairos-new-task")),
    ("Ctrl+Alt+Shift+F", "Search", Some("kairos-search")),
    ("Ctrl+Alt+Shift+S", "Settings", Some("kairos-open-settings")),
    ("Ctrl+Alt+Shift+R", "Restore mouse interaction", None),
];

pub(super) fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    app.plugin(
        tauri_plugin_autostart::Builder::new()
            .app_name(app.config().identifier.clone())
            .build(),
    )?;
    let state = app.state::<IntegrationState>();
    for (binding, label, event) in SHORTCUTS {
        let registration = app
            .global_shortcut()
            .on_shortcut(binding, move |app, _, shortcut| {
                if shortcut.state == ShortcutState::Released {
                    let result = match label {
                        "Show or hide" => toggle_window(app),
                        "Restore mouse interaction" => {
                            set_click_through(app.clone(), false).map(|_| ())
                        }
                        _ => show(app, event),
                    };
                    if let Err(error) = result {
                        eprintln!("Kairos shortcut {binding}: {error}");
                    }
                }
            });
        if let Err(error) = registration {
            state.unavailable_shortcuts.lock().unwrap().push(label);
            eprintln!("Could not register Kairos shortcut {binding}: {error}");
        }
    }
    if let Some(window) = app.get_webview_window("main") {
        let app = app.clone();
        thread::spawn(move || {
            let mut applied = None;
            loop {
                let state = app.state::<IntegrationState>();
                let desired = {
                    let preferences = state.preferences.lock().unwrap();
                    match preferences.window_mode {
                        WindowMode::Desktop => false,
                        WindowMode::AlwaysOnTop => true,
                        WindowMode::Smart => {
                            smart_top(&preferences.whitelist, applied.unwrap_or(false))
                        }
                    }
                };
                if Some(desired) != applied && window.set_always_on_top(desired).is_ok() {
                    applied = Some(desired);
                }
                thread::sleep(Duration::from_millis(600));
            }
        });
    }
    Ok(())
}

pub(super) fn startup_enabled(app: &AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())
}

pub(super) fn set_launch_at_login(app: &AppHandle, enabled: bool) -> Result<bool, String> {
    let startup = app.autolaunch();
    if startup_enabled(app)? != enabled {
        if enabled {
            startup.enable()
        } else {
            startup.disable()
        }
        .map_err(|error| error.to_string())?;
    }
    startup_enabled(app)
}

fn toggle_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Kairos window is unavailable.")?;
    let state = app.state::<IntegrationState>();
    let click_through = *state
        .click_through
        .lock()
        .map_err(|error| error.to_string())?;
    let visible = window.is_visible().map_err(|error| error.to_string())?;
    let minimized = window.is_minimized().map_err(|error| error.to_string())?;
    if visible && !minimized && !click_through {
        window.hide().map_err(|error| error.to_string())
    } else {
        show(app, None)
    }
}

fn smart_top(whitelist: &[String], previous: bool) -> bool {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return previous;
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == std::process::id() {
        return previous || whitelist.is_empty();
    }
    if is_desktop(hwnd) {
        return true;
    }
    !is_fullscreen(hwnd)
        && (whitelist.is_empty()
            || executable_name(pid).is_some_and(|name| whitelist.contains(&name)))
}

fn is_desktop(hwnd: HWND) -> bool {
    if hwnd == unsafe { GetShellWindow() } {
        return true;
    }
    let mut class = [0u16; 64];
    let length = unsafe { GetClassNameW(hwnd, &mut class) } as usize;
    matches!(
        String::from_utf16_lossy(&class[..length]).as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

fn is_fullscreen(hwnd: HWND) -> bool {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return false;
    }
    let mut bounds = RECT::default();
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut bounds as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
    };
    if result.is_err() && unsafe { GetWindowRect(hwnd, &mut bounds) }.is_err() {
        return false;
    }
    let screen = info.rcMonitor;
    bounds.left <= screen.left
        && bounds.top <= screen.top
        && bounds.right >= screen.right
        && bounds.bottom >= screen.bottom
}

fn executable_name(pid: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut path = [0u16; 32768];
    let mut length = path.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        )
    };
    let _ = unsafe { CloseHandle(handle) };
    result.ok()?;
    let path = String::from_utf16_lossy(&path[..length as usize]);
    Path::new(&path)
        .file_name()?
        .to_str()
        .map(str::to_lowercase)
}
