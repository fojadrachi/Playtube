//! Namen der Audio-Ausgabegeräte (für "Audioausgabe pro Tab"). Die Seite sucht das Gerät über
//! genau diesen Namen (src/js/audio_router.js), denn WebView2 vergibt pro Website eigene IDs.

use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{eRender, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED, STGM_READ};

/// Aktive Ausgabegeräte in der Reihenfolge des Systems, ohne Doppelte.
pub fn output_devices() -> Vec<String> {
    unsafe {
        // Im UI-Thread ist COM schon (als STA) initialisiert; ein zweiter Aufruf schadet nicht.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else {
            return Vec::new();
        };
        let Ok(collection) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else { return Vec::new() };
        let count = collection.GetCount().unwrap_or(0);
        let mut names = Vec::new();
        for i in 0..count {
            let Ok(device) = collection.Item(i) else { continue };
            let Ok(store) = device.OpenPropertyStore(STGM_READ) else { continue };
            let Ok(value) = store.GetValue(&PKEY_Device_FriendlyName) else { continue };
            let name = value.to_string();
            if !name.is_empty() && !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }
}

/// Programme, die gerade ein Aufnahmegerät (Mikrofon) aktiv nutzen - Dateiname klein, z. B.
/// "discord.exe". Ein Programm taucht nur auf, solange es wirklich aufnimmt.
pub fn active_capture_apps() -> Vec<String> {
    use windows::core::Interface;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Media::Audio::{
        eCapture, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2,
    };
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    let mut apps = Vec::new();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else {
            return apps;
        };
        let Ok(devices) = enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) else { return apps };
        for i in 0..devices.GetCount().unwrap_or(0) {
            let Ok(device) = devices.Item(i) else { continue };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
            let Ok(sessions) = manager.GetSessionEnumerator() else { continue };
            for j in 0..sessions.GetCount().unwrap_or(0) {
                let Ok(session) = sessions.GetSession(j) else { continue };
                if session.GetState().ok() != Some(AudioSessionStateActive) {
                    continue;
                }
                let Ok(session2) = session.cast::<IAudioSessionControl2>() else { continue };
                let Ok(pid) = session2.GetProcessId() else { continue };
                let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { continue };
                let mut buffer = [0u16; 512];
                let mut len = buffer.len() as u32;
                let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, windows::core::PWSTR(buffer.as_mut_ptr()), &mut len).is_ok();
                let _ = CloseHandle(process);
                if !ok {
                    continue;
                }
                let path = String::from_utf16_lossy(&buffer[..len as usize]);
                let name = path.rsplit('\\').next().unwrap_or("").to_lowercase();
                if !name.is_empty() && !apps.contains(&name) {
                    apps.push(name);
                }
            }
        }
    }
    apps
}

/// Ist Discord gerade in einem Sprachkanal (Discord nimmt vom Mikrofon auf)? Nur von dort aus kann
/// man streamen - dann braucht es den Ton über Playtube.exe ("Für Discord-Streams").
pub fn discord_in_voice(apps: &[String]) -> bool {
    apps.iter().any(|name| name.starts_with("discord"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_discord_builds() {
        assert!(discord_in_voice(&["discord.exe".into()]));
        assert!(discord_in_voice(&["chrome.exe".into(), "discordptb.exe".into()]));
        assert!(discord_in_voice(&["discordcanary.exe".into()]));
        assert!(!discord_in_voice(&["chrome.exe".into(), "obs64.exe".into()]));
        assert!(!discord_in_voice(&[]));
    }

    /// Echte Geräte: listet, wer gerade aufnimmt (cargo test -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn lists_capture_apps() {
        println!("aktive Aufnahme: {:?}", active_capture_apps());
    }
}
