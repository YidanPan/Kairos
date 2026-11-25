use std::{sync::mpsc, thread, time::Duration};

use objc2::{runtime::NSObjectProtocol, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSApplicationPresentationOptions, NSRunningApplication, NSWorkspace,
};
use tauri::{AppHandle, Manager};

use super::{IntegrationState, WindowMode};

pub(super) fn setup(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        let mut applied = None;
        loop {
            let handle = app.clone();
            let (send, recv) = mpsc::channel();
            let scheduled = app.run_on_main_thread(move || {
                let _ = send.send(update_window_level(&handle, applied));
            });
            if scheduled.is_err() {
                break;
            }
            let Ok(level) = recv.recv() else { break };
            applied = level;
            thread::sleep(Duration::from_millis(600));
        }
    });
}

fn update_window_level(app: &AppHandle, applied: Option<bool>) -> Option<bool> {
    let state = app.state::<IntegrationState>();
    let Ok(preferences) = state.preferences.try_lock().map(|value| value.clone()) else {
        return applied;
    };
    let desired = match preferences.window_mode {
        WindowMode::Desktop => false,
        WindowMode::AlwaysOnTop => true,
        WindowMode::Smart => smart_top(&preferences.whitelist, applied.unwrap_or(false)),
    };
    if Some(desired) != applied {
        let window = app.get_webview_window("main")?;
        if window.set_always_on_top(desired).is_ok() {
            return Some(desired);
        }
    }
    applied
}

fn smart_top(whitelist: &[String], previous: bool) -> bool {
    let Some(main_thread) = MainThreadMarker::new() else {
        return previous;
    };
    let options = NSApplication::sharedApplication(main_thread).currentSystemPresentationOptions();
    if fullscreen(options) {
        return false;
    }
    if whitelist.is_empty() {
        return true;
    }
    let Some(frontmost) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
        return previous;
    };
    if frontmost.isEqual(Some(&NSRunningApplication::currentApplication())) {
        return previous;
    }
    let name = frontmost.localizedName().map(|value| value.to_string());
    let bundle_id = frontmost.bundleIdentifier().map(|value| value.to_string());
    is_allowed(whitelist, name.as_deref(), bundle_id.as_deref())
}

fn fullscreen(options: NSApplicationPresentationOptions) -> bool {
    options.contains(NSApplicationPresentationOptions::FullScreen)
        || options.contains(
            NSApplicationPresentationOptions::HideDock
                | NSApplicationPresentationOptions::HideMenuBar,
        )
}

fn is_allowed(whitelist: &[String], name: Option<&str>, bundle_id: Option<&str>) -> bool {
    [name, bundle_id]
        .into_iter()
        .flatten()
        .any(|candidate| whitelist.contains(&candidate.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::{normalize_whitelist, Behavior};

    #[test]
    fn fullscreen_and_kiosk_yield_but_autohide_remains_eligible() {
        use NSApplicationPresentationOptions as Options;

        assert!(fullscreen(Options::FullScreen | Options::AutoHideMenuBar));
        assert!(fullscreen(Options::HideDock | Options::HideMenuBar));
        assert!(!fullscreen(
            Options::AutoHideDock | Options::AutoHideMenuBar
        ));
        assert!(!fullscreen(Options::empty()));
    }

    #[test]
    fn smart_mode_accepts_app_names_and_bundle_ids_and_survives_serialization() {
        let mut preferences: Behavior = serde_json::from_str(
            r#"{"windowMode":"smart","whitelist":[" Visual Studio Code ","com.apple.Safari","COM.APPLE.SAFARI"]}"#,
        )
        .unwrap();
        preferences.whitelist = normalize_whitelist(preferences.whitelist).unwrap();
        let saved = serde_json::to_string(&preferences).unwrap();
        let loaded: Behavior = serde_json::from_str(&saved).unwrap();
        assert!(matches!(loaded.window_mode, WindowMode::Smart));
        assert!(matches!(
            Behavior::default().window_mode,
            WindowMode::Desktop
        ));
        assert_eq!(loaded.whitelist, ["visual studio code", "com.apple.safari"]);
        assert!(is_allowed(
            &loaded.whitelist,
            Some("Visual Studio Code"),
            None
        ));
        assert!(is_allowed(
            &loaded.whitelist,
            None,
            Some("com.apple.Safari")
        ));
        assert!(!is_allowed(
            &loaded.whitelist,
            Some("Finder"),
            Some("com.apple.finder")
        ));
        assert!(normalize_whitelist(vec!["/Applications/Safari.app".into()]).is_err());
    }
}
