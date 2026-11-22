use std::{
    collections::VecDeque,
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};

use tauri::AppHandle;
use windows::{
    core::{IInspectable, Interface, Ref, HSTRING, PCWSTR},
    Data::Xml::Dom::XmlDocument,
    Foundation::TypedEventHandler,
    Win32::{
        Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_FLAGS, SND_NODEFAULT},
        System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
        UI::Shell::SetCurrentProcessExplicitAppUserModelID,
    },
    UI::Notifications::{
        NotificationSetting, ToastActivatedEventArgs, ToastNotification, ToastNotificationManager,
    },
};

use super::{activated, activation, Notice, Result};
use activation::Action;

static CALLBACK_GENERATION: AtomicUsize = AtomicUsize::new(0);

pub(super) fn invalidate_callbacks() {
    CALLBACK_GENERATION.fetch_add(1, Ordering::Relaxed);
}

pub(super) fn set_identity(identifier: &str) -> Result<()> {
    let identity = HSTRING::from(identifier);
    // This sets the Shell identity for this process only.
    unsafe { SetCurrentProcessExplicitAppUserModelID(&identity) }.map_err(|error| error.to_string())
}

pub(super) struct Toasts {
    initialized: bool,
    protocol: Option<String>,
    live: VecDeque<(ToastNotification, i64)>,
}

impl Toasts {
    pub(super) fn new(app: &AppHandle) -> Self {
        // The reminder worker owns its own WinRT apartment.
        let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
        Self {
            initialized,
            protocol: registered_protocol(app),
            live: VecDeque::new(),
        }
    }

    pub(super) fn show(
        &mut self,
        app: &AppHandle,
        database: &rusqlite::Connection,
        notice: Notice,
        custom: bool,
    ) -> Result<()> {
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
        let link = self
            .protocol
            .as_ref()
            .map(|scheme| {
                activation::issue(database, notice.id, chrono::Local::now().timestamp()).map(
                    |token| activation::Link {
                        scheme: scheme.clone(),
                        token,
                    },
                )
            })
            .transpose()?;
        let payload = toast_xml(&notice, custom, link.as_ref());
        document
            .LoadXml(&HSTRING::from(payload))
            .map_err(|error| error.to_string())?;
        let toast = ToastNotification::CreateToastNotification(&document)
            .map_err(|error| error.to_string())?;
        if link.is_some() {
            return notifier
                .Show(&toast)
                .map_err(|error| format!("Show Windows toast: {error}"));
        }
        let app = app.clone();
        let generation = CALLBACK_GENERATION.load(Ordering::Relaxed);
        let listener = TypedEventHandler::<ToastNotification, IInspectable>::new(
            move |_, event: Ref<IInspectable>| {
                if generation != CALLBACK_GENERATION.load(Ordering::Relaxed) {
                    return Ok(());
                }
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

fn registered_protocol(app: &AppHandle) -> Option<String> {
    let scheme = activation::scheme(&app.config().identifier);
    let key = windows_registry::CURRENT_USER
        .open(format!(r"Software\Classes\{scheme}\shell\open\command"))
        .ok()?;
    let command = key.get_string("").ok()?;
    let expected = format!("\"{}\" \"%1\"", std::env::current_exe().ok()?.display());
    command.eq_ignore_ascii_case(&expected).then_some(scheme)
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

fn toast_xml(notice: &Notice, custom: bool, link: Option<&activation::Link>) -> String {
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
    let argument = |action: Action| {
        escape(&link.map_or_else(|| action.name().into(), |link| link.uri(action)))
    };
    let activation = if link.is_some() {
        " activationType=\"protocol\""
    } else {
        ""
    };
    let open = argument(Action::Open);
    let complete = argument(Action::Complete);
    let snooze = argument(Action::Snooze);
    format!("<toast launch=\"{snooze}\"{activation}><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{body}</text>{deadline}</binding></visual><actions>\
         <action content=\"Snooze\" arguments=\"{snooze}\"{activation}/><action content=\"Open Task\" arguments=\"{open}\"{activation}/><action content=\"Complete\" arguments=\"{complete}\"{activation}/></actions>{audio}</toast>", escape(&notice.title))
}

pub(super) fn play(directory: &Path, sound: &[u8]) -> Result<()> {
    static PLAYING: Mutex<()> = Mutex::new(());
    let _lock = PLAYING.lock().map_err(|error| error.to_string())?;
    let path = directory.join("playback-cache.wav");
    let mut pending =
        tempfile::NamedTempFile::new_in(directory).map_err(|error| error.to_string())?;
    pending
        .write_all(sound)
        .map_err(|error| error.to_string())?;
    let _ = unsafe { PlaySoundW(PCWSTR::null(), None, SND_FLAGS(0)) };
    pending.persist(&path).map_err(|error| error.to_string())?;
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
        let xml = toast_xml(&notice, false, None);
        assert!(xml.contains("A &lt; B &amp; C"));
        assert!(!xml.contains('\u{0001}'));
        assert!(xml.contains("2 reminders due"));
        assert!(xml.contains("silent=\"true\""));
        for target in ["complete", "snooze", "open"] {
            assert!(xml.contains(&format!("arguments=\"{target}\"")));
        }
        let link = activation::Link {
            scheme: activation::scheme("app.kairos.test"),
            token: "1".repeat(32),
        };
        let xml = toast_xml(&notice, false, Some(&link));
        assert!(xml.contains(&format!(
            "launch=\"{}\" activationType=\"protocol\"",
            link.uri(Action::Snooze)
        )));
        assert!(xml.contains("<actions><action content=\"Snooze\""));
        for (action, label) in [
            (Action::Complete, "Complete"),
            (Action::Snooze, "Snooze"),
            (Action::Open, "Open Task"),
        ] {
            assert!(xml.contains(&format!(
                "content=\"{label}\" arguments=\"{}\" activationType=\"protocol\"",
                link.uri(action)
            )));
        }
    }
}
