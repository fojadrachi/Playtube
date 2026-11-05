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
