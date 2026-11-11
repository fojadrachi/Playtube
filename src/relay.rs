//! Playtube-Ton: gibt den Ton, den die Seite per Web Audio abgreift (js/audio_tap.js), aus
//! Playtube.exe selbst aus (WASAPI, gemeinsamer Modus). Dadurch erkennt Windows den Ton als
//! "Playtube": Discord überträgt ihn beim Teilen des Playtube-Fensters, und Mixer wie SteelSeries
//! Sonar zeigen Playtube statt msedgewebview2 an.
//!
//! Pro Tab ein Relay mit eigenem Puffer und eigenem Ausgabegerät (Einstellung "Audioausgabe").

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Ziel-Vorlauf im Puffer (ms): groß genug, dass kurze Ruckler der Seite keine Aussetzer machen.
const TARGET_MS: u32 = 140;
/// Mehr als das im Puffer: auf den Vorlauf zurückkürzen (Uhren-Drift, Hänger).
const MAX_MS: u32 = 450;
/// Länge der Ein-/Ausblendung bei Aussetzern (Frames bei 48 kHz ~ 5 ms) - verhindert Knacken.
const FADE_FRAMES: usize = 240;

struct Shared {
    buffer: Mutex<VecDeque<i16>>,
    rate: AtomicU32,
    device: Mutex<String>,
    reopen: AtomicBool,
    stop: AtomicBool,
    /// Wiedergabe gestartet (Vorlauf erreicht); bei leerem Puffer wieder sammeln
    primed: AtomicBool,
}

pub struct Relay {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Relay {
    pub fn start(device: &str) -> Self {
        let shared = Arc::new(Shared {
            buffer: Mutex::new(VecDeque::with_capacity(48_000)),
            rate: AtomicU32::new(48_000),
            device: Mutex::new(device.to_string()),
            reopen: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            primed: AtomicBool::new(false),
        });
        let worker = shared.clone();
        let thread = std::thread::Builder::new().name("playtube-relay".into()).spawn(move || render_loop(worker)).ok();
        Self { shared, thread }
    }

    /// Neues Stück Ton (16 Bit, Stereo, verschachtelt).
    pub fn push(&self, rate: u32, samples: &[i16]) {
        if rate != self.shared.rate.swap(rate, Ordering::Relaxed) {
            self.shared.reopen.store(true, Ordering::Relaxed);
        }
        let max = (rate * MAX_MS / 1000 * 2) as usize;
        let target = (rate * TARGET_MS / 1000 * 2) as usize;
        let mut buffer = self.shared.buffer.lock().unwrap();
        buffer.extend(samples.iter().copied());
        if buffer.len() > max {
            let drop = buffer.len() - target;
            buffer.drain(..drop - drop % 2);
        }
    }

    pub fn set_device(&self, device: &str) {
        let mut current = self.shared.device.lock().unwrap();
        if *current != device {
            *current = device.to_string();
            self.shared.reopen.store(true, Ordering::Relaxed);
        }
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Nachricht der Seite `pcm:<rate>:<base64>` -> (Rate, Samples). Fremdes wird verworfen.
pub fn parse_message(text: &str) -> Option<(u32, Vec<i16>)> {
    use base64::Engine;
    let rest = text.strip_prefix("pcm:")?;
    let (rate, data) = rest.split_once(':')?;
    let rate: u32 = rate.parse().ok().filter(|r| (8_000..=192_000).contains(r))?;
    if data.len() > 200_000 {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data).ok()?;
    if bytes.len() % 4 != 0 {
        return None;
    }
    Some((rate, bytes.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect()))
}

fn render_loop(shared: Arc<Shared>) {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    while !shared.stop.load(Ordering::Relaxed) {
        shared.reopen.store(false, Ordering::Relaxed);
        let device = shared.device.lock().unwrap().clone();
        if let Err(e) = unsafe { play(&shared, &device) } {
            crate::log::line(&format!("relay: Ausgabe auf \"{device}\" fehlgeschlagen: {e}"));
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

unsafe fn find_device(name: &str) -> windows::core::Result<windows::Win32::Media::Audio::IMMDevice> {
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, STGM_READ};
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        if !name.is_empty() {
            let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            for i in 0..collection.GetCount()? {
                let device = collection.Item(i)?;
                let found = device
                    .OpenPropertyStore(STGM_READ)
                    .and_then(|store| store.GetValue(&PKEY_Device_FriendlyName))
                    .map(|v| v.to_string() == name)
                    .unwrap_or(false);
                if found {
                    return Ok(device);
                }
            }
        }
        enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)
    }
}

/// Spielt, bis das Gerät gewechselt/geschlossen werden soll.
unsafe fn play(shared: &Shared, device_name: &str) -> windows::core::Result<()> {
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::CLSCTX_ALL;
    unsafe {
        let device = find_device(device_name)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let rate = shared.rate.load(Ordering::Relaxed);
        let format = WAVEFORMATEX {
            wFormatTag: 1, // PCM
            nChannels: 2,
            nSamplesPerSec: rate,
            nAvgBytesPerSec: rate * 4,
            nBlockAlign: 4,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        // 100 ms Puffer bei WASAPI, Formatumwandlung übernimmt Windows.
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            1_000_000,
            0,
            &format,
            None,
        )?;
        let frames_total = client.GetBufferSize()?;
        let render: IAudioRenderClient = client.GetService()?;
        client.Start()?;
        let target = (rate * TARGET_MS / 1000 * 2) as usize;
        let mut last = [0i16; 2];
        let mut fade_in = 0usize;
        let result = loop {
            if shared.stop.load(Ordering::Relaxed) || shared.reopen.load(Ordering::Relaxed) {
                break Ok(());
            }
            std::thread::sleep(Duration::from_millis(8));
            let padding = match client.GetCurrentPadding() {
                Ok(p) => p,
                Err(e) => break Err(e), // z. B. Gerät entfernt -> neu öffnen
            };
            let free = frames_total.saturating_sub(padding);
            if free == 0 {
                continue;
            }
            let data = match render.GetBuffer(free) {
                Ok(d) => d as *mut i16,
                Err(e) => break Err(e),
            };
            let out = std::slice::from_raw_parts_mut(data, free as usize * 2);
            let mut buffer = shared.buffer.lock().unwrap();
            // Erst losspielen, wenn genug Vorlauf da ist; bei leerem Puffer neu sammeln.
            if !shared.primed.load(Ordering::Relaxed) && buffer.len() >= target {
                shared.primed.store(true, Ordering::Relaxed);
            }
            let mut written = 0;
            if shared.primed.load(Ordering::Relaxed) {
                for frame in out.chunks_exact_mut(2) {
                    if buffer.len() < 2 {
                        break;
                    }
                    // Nach einer Pause sanft einblenden.
                    let gain = if fade_in < FADE_FRAMES { fade_in as f32 / FADE_FRAMES as f32 } else { 1.0 };
                    fade_in = fade_in.saturating_add(1);
                    for slot in frame.iter_mut() {
                        let sample = buffer.pop_front().unwrap_or(0);
                        *slot = (sample as f32 * gain) as i16;
                    }
                    last = [frame[0], frame[1]];
                    written += 2;
                }
                if buffer.len() < 2 {
                    shared.primed.store(false, Ordering::Relaxed);
                }
            }
            drop(buffer);
            // Aussetzer: vom letzten Wert sanft auf Stille ausblenden statt hart abzuschneiden.
            let rest = &mut out[written..];
            for (i, frame) in rest.chunks_exact_mut(2).enumerate() {
                let gain = if i < FADE_FRAMES { 1.0 - (i + 1) as f32 / FADE_FRAMES as f32 } else { 0.0 };
                frame[0] = (last[0] as f32 * gain) as i16;
                frame[1] = (last[1] as f32 * gain) as i16;
            }
            if written < out.len() {
                last = [0, 0];
                fade_in = 0;
            }
            let _ = render.ReleaseBuffer(free, 0);
        };
        let _ = client.Stop();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn parses_pcm_messages() {
        let samples: [i16; 4] = [1, -1, 32767, -32768];
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let text = format!("pcm:48000:{}", base64::engine::general_purpose::STANDARD.encode(&bytes));
        assert_eq!(parse_message(&text), Some((48_000, samples.to_vec())));
        assert_eq!(parse_message("pcm:1:AAAA"), None);
        assert_eq!(parse_message("hallo"), None);
        assert_eq!(parse_message("pcm:48000:kein base64!"), None);
        assert_eq!(parse_message("pcm:48000:AAA="), None); // keine ganzen Stereo-Frames
    }

    #[test]
    fn buffer_stays_bounded() {
        let relay = Relay { shared: Arc::new(Shared {
            buffer: Mutex::new(VecDeque::new()),
            rate: AtomicU32::new(48_000),
            device: Mutex::new(String::new()),
            reopen: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            primed: AtomicBool::new(false),
        }), thread: None };
        for _ in 0..100 {
            relay.push(48_000, &[0i16; 2048]);
        }
        let len = relay.shared.buffer.lock().unwrap().len();
        assert!(len <= (48_000 * MAX_MS / 1000 * 2) as usize, "{len}");
        assert_eq!(len % 2, 0);
    }
}
