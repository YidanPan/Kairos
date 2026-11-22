use std::{collections::VecDeque, path::Path};

use tauri::AppHandle;
use windows::{
    core::{IInspectable, Interface, Ref, HSTRING},
    Data::Xml::Dom::XmlDocument,
    Foundation::TypedEventHandler,
    Win32::{
        Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT},
        System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
        UI::Shell::SetCurrentProcessExplicitAppUserModelID,
    },
    UI::Notifications::{
        NotificationSetting, ToastActivatedEventArgs, ToastNotification, ToastNotificationManager,
    },
};

use super::{activated, Notice, Result};

pub(super) fn set_identity(identifier: &str) -> Result<()> {
    let identity = HSTRING::from(identifier);
    // This sets the Shell identity for this process only.
    unsafe { SetCurrentProcessExplicitAppUserModelID(&identity) }.map_err(|error| error.to_string())
}

pub(super) struct Toasts {
    initialized: bool,
    live: VecDeque<(ToastNotification, i64)>,
}

impl Toasts {
    pub(super) fn new() -> Self {
        // The reminder worker owns its own WinRT apartment.
        let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
        Self {
            initialized,
            live: VecDeque::new(),
        }
    }

    pub(super) fn show(&mut self, app: &AppHandle, notice: Notice, custom: bool) -> Result<()> {
        if !self.initialized {
            return Err("Windows notification runtime could not initialize.".into());
        }
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
            &app.config().identifier,
        ))
        .map_err(|error| format!("Create Windows notifier: {error}"))?;
        if let Ok(setting) = notifier.Setting() {
            if setting != NotificationSetting::Enabled {
                return Err(format!(
                    "Windows notification setting rejected delivery ({})",
                    setting.0
                ));
            }
        }
        let document = XmlDocument::new().map_err(|error| error.to_string())?;
        let payload = toast_xml(&notice, custom);
        document
            .LoadXml(&HSTRING::from(payload))
            .map_err(|error| error.to_string())?;
        let toast = ToastNotification::CreateToastNotification(&document)
            .map_err(|error| error.to_string())?;
        let app = app.clone();
        let listener = TypedEventHandler::<ToastNotification, IInspectable>::new(
            move |_, event: Ref<IInspectable>| {
                if let Ok(event) = event
                    .ok()
                    .and_then(|event| event.cast::<ToastActivatedEventArgs>())
                {
                    let action = event
                        .Arguments()
                        .map(|value| value.to_string())
                        .unwrap_or_default();
                    activated(app.clone(), notice.clone(), &action);
                }
                Ok(())
            },
        );
        let token = toast
            .Activated(&listener)
            .map_err(|error| format!("Subscribe to Windows toast activation: {error}"))?;
        if let Err(error) = notifier.Show(&toast) {
            let _ = toast.RemoveActivated(token);
            return Err(format!("Show Windows toast: {error}"));
        }
        self.live.push_back((toast, token));
        if self.live.len() > 128 {
            if let Some((toast, token)) = self.live.pop_front() {
                let _ = toast.RemoveActivated(token);
            }
        }
        Ok(())
    }
}

impl Drop for Toasts {
    fn drop(&mut self) {
        for (toast, token) in self.live.drain(..) {
            let _ = toast.RemoveActivated(token);
        }
        if self.initialized {
            unsafe {
                RoUninitialize();
            }
        }
    }
}

fn escape(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(*character, '\n' | '\r' | '\t'))
        .fold(String::new(), |mut xml, character| {
            match character {
                '&' => xml.push_str("&amp;"),
                '<' => xml.push_str("&lt;"),
                '>' => xml.push_str("&gt;"),
                _ => xml.push(character),
            }
            xml
        })
}

fn toast_xml(notice: &Notice, custom: bool) -> String {
    let body = if notice.count > 1 {
        format!("{} reminders due", notice.count)
    } else {
        "Task reminder".into()
    };
    let deadline = notice
        .deadline
        .as_deref()
        .map(|value| format!("<text>Due {}</text>", escape(value)))
        .unwrap_or_default();
    let audio = if notice.silent || custom {
        "<audio silent=\"true\"/>"
    } else {
        ""
    };
    format!(
        "<toast launch=\"open\"><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text>{deadline}</binding></visual><actions>\
         <action content=\"Complete\" arguments=\"complete\"/><action content=\"Snooze\" arguments=\"snooze\"/><action content=\"Open Task\" arguments=\"open\"/></actions>{audio}</toast>",
        escape(&notice.title),body
    )
}

pub(super) fn play(path: &Path) -> Result<()> {
    if !path.is_file() {
        return Err("Choose a WAV file first.".into());
    }
    let path = HSTRING::from(path.as_os_str());
    let playing = unsafe { PlaySoundW(&path, None, SND_FILENAME | SND_ASYNC | SND_NODEFAULT) };
    if playing.as_bool() {
        Ok(())
    } else {
        Err("Windows could not play this WAV file.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_payload_escapes_titles_and_includes_all_actions() {
        let notice = Notice {
            id: 1,
            task_id: "a".into(),
            title: "A < B & C\u{0001}".into(),
            deadline: Some("2026-11-22 10:00".into()),
            created_at: 0,
            first_due_at: 0,
            count: 2,
            silent: true,
            native_status: "pending".into(),
        };
        let xml = toast_xml(&notice, false);
        assert!(xml.contains("A &lt; B &amp; C"));
        assert!(!xml.contains('\u{0001}'));
        assert!(xml.contains("2 reminders due"));
        assert!(xml.contains("silent=\"true\""));
        for target in ["complete", "snooze", "open"] {
            assert!(xml.contains(&format!("arguments=\"{target}\"")));
        }
    }
}
