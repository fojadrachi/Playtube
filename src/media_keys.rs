//! Medientasten der Tastatur (Play/Pause, Weiter, Zurück, Stopp) – auch wenn Playtube im
//! Hintergrund oder nur im Tray läuft.
//!
//! Ein Low-Level-Tastatur-Hook in einem eigenen Thread sieht die Tasten, schluckt sie aber nicht:
//! andere Programme bekommen sie weiterhin. Playtube reagiert nur, wenn in einem Tab ein Video
//! bzw. Song geladen ist (sonst würde z. B. Spotify und Playtube gleichzeitig starten). Die
//! eingebaute Medientasten-Behandlung von Chromium ist abgeschaltet (src/browser.rs), damit ein
//! Tastendruck nicht doppelt ankommt, wenn das Playtube-Fenster vorne ist.

use serde_json::Value;
use std::sync::OnceLock;

const VK_MEDIA_NEXT_TRACK: u32 = 0xB0;
const VK_MEDIA_PREV_TRACK: u32 = 0xB1;
const VK_MEDIA_STOP: u32 = 0xB2;
const VK_MEDIA_PLAY_PAUSE: u32 = 0xB3;

/// Befehl (wie im Tray-Menü) zu einem virtuellen Tastencode.
pub fn command_for(vk: u32) -> Option<&'static str> {
    match vk {
        VK_MEDIA_PLAY_PAUSE => Some("play_pause"),
        VK_MEDIA_NEXT_TRACK => Some("next"),
        VK_MEDIA_PREV_TRACK => Some("previous"),
        VK_MEDIA_STOP => Some("pause"),
        // Nur im Test: F24 (von keinem Programm benutzt) statt einer echten Medientaste.
        #[cfg(test)]
        0x87 => Some("test"),
        _ => None,
    }
}

/// Reagiert Playtube? Nur wenn ein Tab ein Video/einen Song geladen hat oder gerade spielt.
pub fn should_handle(tabs: &[Option<&Value>; 2]) -> bool {
    tabs.iter().flatten().any(|t| {
        ["playing", "hasVideo"].iter().any(|k| t.get(*k).and_then(Value::as_bool).unwrap_or(false))
    })
}

type Handler = Box<dyn Fn(&'static str) + Send + Sync>;
static HANDLER: OnceLock<Handler> = OnceLock::new();

/// Hook-Thread starten; `handler` bekommt "play_pause", "next", "previous" oder "pause"
/// (aus dem Hook-Thread heraus – also schnell weiterreichen, z. B. per invoke_from_event_loop).
pub fn start(handler: impl Fn(&'static str) + Send + Sync + 'static) {
    if HANDLER.set(Box::new(handler)).is_err() {
        return; // läuft schon
    }
    let spawned = std::thread::Builder::new().name("media-keys".into()).spawn(|| unsafe { run_hook() });
    if let Err(e) = spawned {
        crate::log::line(&format!("medientasten: Thread nicht gestartet: {e}"));
    }
}

unsafe fn run_hook() {
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, SetWindowsHookExW, MSG, WH_KEYBOARD_LL};
    let module = unsafe { GetModuleHandleW(None) }.ok();
    let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module.map(Into::into), 0) };
    if let Err(e) = hook {
        crate::log::line(&format!("medientasten: Hook nicht gesetzt: {e}"));
        return;
    }
    // Der Hook läuft nur, solange dieser Thread Nachrichten abholt.
    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {}
}

static DOWN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

unsafe extern "system" fn keyboard_proc(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use std::sync::atomic::Ordering;
    use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP};
    if code == HC_ACTION as i32 {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if let Some(command) = command_for(info.vkCode) {
            let message = wparam.0 as u32;
            if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
                // Gedrückt halten erzeugt Wiederholungen – nur der erste Druck zählt.
                if DOWN.swap(info.vkCode, Ordering::Relaxed) != info.vkCode {
                    if let Some(handler) = HANDLER.get() {
                        handler(command);
                    }
                }
            } else if message == WM_KEYUP || message == WM_SYSKEYUP {
                DOWN.store(0, Ordering::Relaxed);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_media_keys() {
        assert_eq!(command_for(0xB3), Some("play_pause"));
        assert_eq!(command_for(0xB0), Some("next"));
        assert_eq!(command_for(0xB1), Some("previous"));
        assert_eq!(command_for(0xB2), Some("pause"));
        assert_eq!(command_for(0x41), None); // "A"
        assert_eq!(command_for(0xAF), None); // Lauter bleibt bei Windows
    }

    /// Echter Hook: F24 per SendInput drücken (stört keine anderen Player).
    /// `cargo test hook_sees_injected_key -- --ignored`
    #[test]
    #[ignore]
    fn hook_sees_injected_key() {
        use std::sync::mpsc;
        use std::time::Duration;
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
        };
        let (tx, rx) = mpsc::channel::<&'static str>();
        let tx = std::sync::Mutex::new(tx);
        start(move |c| {
            let _ = tx.lock().unwrap().send(c);
        });
        std::thread::sleep(Duration::from_millis(300)); // Hook-Thread hochfahren lassen
        let key = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0x87), wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        };
        // Zweimal drücken, dazwischen loslassen; Halten (zweites Down ohne Up) zählt nicht.
        let inputs = [key(Default::default()), key(Default::default()), key(KEYEVENTF_KEYUP), key(Default::default()), key(KEYEVENTF_KEYUP)];
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        assert_eq!(sent, inputs.len() as u32);
        assert_eq!(rx.recv_timeout(Duration::from_secs(3)), Ok("test"));
        assert_eq!(rx.recv_timeout(Duration::from_secs(3)), Ok("test"));
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err(), "Wiederholung beim Halten darf nicht zählen");
    }

    #[test]
    fn only_reacts_with_something_loaded() {
        let idle = json!({"playing": false, "hasVideo": false});
        let loaded = json!({"playing": false, "hasVideo": true});
        let playing = json!({"playing": true});
        assert!(!should_handle(&[None, None]));
        assert!(!should_handle(&[Some(&idle), None]));
        assert!(should_handle(&[Some(&idle), Some(&loaded)]));
        assert!(should_handle(&[Some(&playing), None]));
    }
}
