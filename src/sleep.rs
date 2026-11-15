//! Schlaf-Timer: nach X Minuten (mit sanftem Ausblenden) oder am Ende des laufenden Songs pausieren.

use std::time::{Duration, Instant};

/// So lange wird vor dem Ende leiser geblendet.
pub const FADE: Duration = Duration::from_secs(15);
/// Auswahl in Menü und Playtube-Ansicht (Minuten).
pub const CHOICES: [u32; 6] = [15, 30, 45, 60, 90, 120];
/// Obergrenze für Fernsteuerung.
pub const MAX_MINUTES: u32 = 24 * 60;

#[derive(Clone, Debug, PartialEq)]
pub enum SleepTimer {
    /// Pausieren um `deadline`, vorher ausblenden.
    At { deadline: Instant, minutes: u32 },
    /// Pausieren, sobald nach `track` ein anderer Song läuft (`None` = noch nichts gemerkt).
    EndOfTrack { track: Option<String> },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Nichts zu tun
    Wait,
    /// Lautstärke auf diesen Anteil (0..1) der ursprünglichen setzen
    Fade(f64),
    /// Jetzt pausieren, Timer ist abgelaufen
    Stop,
}

impl SleepTimer {
    pub fn minutes(minutes: u32, now: Instant) -> Self {
        let minutes = minutes.clamp(1, MAX_MINUTES);
        SleepTimer::At { deadline: now + Duration::from_secs(minutes as u64 * 60), minutes }
    }

    pub fn end_of_track(current: Option<String>) -> Self {
        SleepTimer::EndOfTrack { track: current }
    }

    /// Restzeit in Sekunden; `-1` = bis zum Ende des Songs (für Status/Anzeige).
    pub fn remaining_secs(&self, now: Instant) -> i64 {
        match self {
            SleepTimer::At { deadline, .. } => deadline.saturating_duration_since(now).as_secs_f64().ceil() as i64,
            SleepTimer::EndOfTrack { .. } => -1,
        }
    }

    /// Ein Takt; `track` = Video-ID des gerade laufenden Songs.
    pub fn tick(&mut self, now: Instant, track: Option<&str>) -> Step {
        match self {
            SleepTimer::At { deadline, .. } => {
                let left = deadline.saturating_duration_since(now);
                if left.is_zero() {
                    Step::Stop
                } else if left <= FADE {
                    Step::Fade(left.as_secs_f64() / FADE.as_secs_f64())
                } else {
                    Step::Wait
                }
            }
            SleepTimer::EndOfTrack { track: remembered } => match (remembered.as_deref(), track) {
                (None, Some(now_playing)) => {
                    *remembered = Some(now_playing.to_string());
                    Step::Wait
                }
                (Some(old), Some(now_playing)) if old != now_playing => Step::Stop,
                _ => Step::Wait,
            },
        }
    }
}

/// "12:34" bzw. "1:02:03" für die Restzeit.
pub fn format_remaining(secs: i64) -> String {
    let secs = secs.max(0);
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_wait_then_fade_then_stop() {
        let start = Instant::now();
        let mut timer = SleepTimer::minutes(1, start);
        assert_eq!(timer.remaining_secs(start), 60);
        assert_eq!(timer.tick(start + Duration::from_secs(10), None), Step::Wait);
        match timer.tick(start + Duration::from_secs(50), None) {
            Step::Fade(f) => assert!((f - 10.0 / 15.0).abs() < 0.01),
            other => panic!("{other:?}"),
        }
        assert_eq!(timer.tick(start + Duration::from_secs(60), None), Step::Stop);
        assert_eq!(timer.remaining_secs(start + Duration::from_secs(70)), 0);
    }

    #[test]
    fn minutes_are_clamped() {
        let start = Instant::now();
        assert_eq!(SleepTimer::minutes(0, start).remaining_secs(start), 60);
        assert_eq!(SleepTimer::minutes(99_999, start).remaining_secs(start), MAX_MINUTES as i64 * 60);
    }

    #[test]
    fn end_of_track_stops_when_the_next_song_starts() {
        let now = Instant::now();
        let mut timer = SleepTimer::end_of_track(None);
        assert_eq!(timer.tick(now, None), Step::Wait);
        assert_eq!(timer.tick(now, Some("aaa")), Step::Wait);
        assert_eq!(timer.tick(now, Some("aaa")), Step::Wait);
        assert_eq!(timer.tick(now, None), Step::Wait, "kurz kein Song (Ladepause) zählt nicht");
        assert_eq!(timer.tick(now, Some("bbb")), Step::Stop);
        assert_eq!(timer.remaining_secs(now), -1);
    }

    #[test]
    fn formats_remaining_time() {
        assert_eq!(format_remaining(65), "1:05");
        assert_eq!(format_remaining(3723), "1:02:03");
        assert_eq!(format_remaining(-4), "0:00");
    }
}
