"""Audio-Ausgabe pro Tab: YouTube und YouTube Musik koennen ueber verschiedene
Ausgabegeraete laufen (z.B. verschiedene Sonar-Kanaele), einstellbar im Einstellungen-Tab.

Warum ueber die Seite und nicht ueber Windows: QtWebEngine (Chromium) spielt den Ton beider
Tabs ueber EINEN gemeinsamen Audio-Prozess ab - Windows kann die Tabs deshalb nicht einzeln
routen (auch nicht "App-Lautstaerke und Geraeteeinstellungen"). Chromium selbst kann aber pro
Medienelement ein Ausgabegeraet waehlen (HTMLMediaElement.setSinkId). Dafuer wird in jeden
Tab ein kleines Skript eingeschleust (build_router_script), das die <video>/<audio>-Elemente
der Seite auf das gewaehlte Geraet legt.

Geraete werden im Skript ueber ihren NAMEN gefunden (Chromium vergibt pro Website eigene
Geraete-IDs, die man von aussen nicht kennt). Chromium blendet Geraetenamen aber aus, solange
die Seite keine Mikrofon-Berechtigung hat - deshalb erteilt Playtube diese Berechtigung fuer
YouTube/YouTube Musik, allerdings nur, solange mindestens ein Tab ein eigenes Geraet nutzt
(sync_audio_permissions). Zum Aufnehmen wird nichts geoeffnet; die Berechtigung dient nur dazu,
dass die Seite die Geraetenamen sehen darf.

Verhalten der Berechtigung (per Test in QtWebEngine 6.11 ermittelt): Sie gilt nur fuer die
laufende Sitzung - Qt schreibt Mikrofon-Freigaben nicht auf die Festplatte. Sie muss also bei
JEDEM Programmstart neu erteilt werden (MainWindow tut das), und wer die Funktion ausschaltet,
hat nach dem naechsten Start keine Freigabe mehr. Innerhalb einer Sitzung bleiben bereits
sichtbar gewordene Geraetenamen bis zum Neustart sichtbar, auch nach dem Entziehen.
Wichtig: Auf der YouTube-Startseite (www.youtube.com/) greift eine Freigabe, die NUR VOR dem
Seitenaufbau erteilt wurde, nicht - eine danach erteilte schon (auf music.youtube.com und
anderen Seiten reicht die Freigabe vorher). Deshalb erteilt MainWindow sie nach jedem
Seitenaufbau erneut; der Router (build_router_script) wiederholt die Geraetesuche alle 2,5 s
und findet die Namen dann von selbst - ohne Neuladen der Seite.
"""
from __future__ import annotations

import json

from PySide6.QtCore import QUrl
from PySide6.QtMultimedia import QMediaDevices
from PySide6.QtWebEngineCore import QWebEnginePermission, QWebEngineProfile

# Name des eingeschleusten Skripts (pro Seite), um es beim Aendern ersetzen zu koennen.
SCRIPT_NAME = "playtube-audio-output"

# Herkunft (Origin), fuer die die Berechtigung erteilt wird - nur die beiden Tabs.
YOUTUBE_ORIGINS = ("https://www.youtube.com", "https://music.youtube.com")

SYSTEM_DEFAULT_LABEL = "Systemstandard"


def list_output_devices() -> list[str]:
    """Namen aller aktuell vorhandenen Ausgabegeraete (Reihenfolge des Systems, ohne
    Doppelte). Diese Namen speichert die Konfiguration und sucht das Skript in der Seite."""
    names: list[str] = []
    for device in QMediaDevices.audioOutputs():
        name = device.description()
        if name and name not in names:
            names.append(name)
    return names


def sync_audio_permissions(profile: QWebEngineProfile, wanted: bool) -> None:
    """Erteilt (wanted=True) bzw. entzieht (wanted=False) die Audiogeraete-Berechtigung fuer
    YouTube/YouTube Musik - damit die Seite die Namen der Ausgabegeraete sehen kann (siehe
    Moduldoku). Gilt nur fuer die laufende Sitzung, deshalb bei jedem Start aufrufen. Der
    Status wird von Qt fuer Mikrofon-Freigaben nicht zuverlaessig gemeldet (bleibt "Ask"),
    daher wird hier einfach jedes Mal erteilt bzw. zurueckgesetzt."""
    permission_type = QWebEnginePermission.PermissionType.MediaAudioCapture
    for origin in YOUTUBE_ORIGINS:
        permission = profile.queryPermission(QUrl(origin), permission_type)
        if wanted:
            permission.grant()
        else:
            permission.reset()


# JavaScript-Vorlage; __TARGET__ wird durch den (JSON-kodierten) Geraetenamen ersetzt.
# Leerer Name = Systemstandard. Mehrfaches Ausfuehren ist sicher: ist der Router schon
# installiert, wird nur das Ziel gewechselt.
_ROUTER_TEMPLATE = r"""
(function () {
  var TARGET = __TARGET__;
  if (window.__playtubeAudio) { window.__playtubeAudio.setTarget(TARGET); return; }

  var SPECIAL = { "default": 1, "communications": 1 };   // virtuelle Eintraege, keine echten Geraete
  var HW_SUFFIX = /^ \([0-9a-f]{4}:[0-9a-f]{4}\)$/i;      // Chromium haengt bei manchen Geraeten die Hardware-ID an
  var state = { target: TARGET, deviceId: "", resolved: false, tries: 0, timer: null, applied: 0, lastError: "", labels: 0 };

  function matches(label, wanted) {
    if (label === wanted) return true;
    return label.indexOf(wanted) === 0 && HW_SUFFIX.test(label.slice(wanted.length));
  }

  async function resolveDevice() {
    if (!state.target) { state.deviceId = ""; state.resolved = true; return; }
    try {
      var list = await navigator.mediaDevices.enumerateDevices();
      var found = null, labelled = 0;
      for (var i = 0; i < list.length; i++) {
        var d = list[i];
        if (d.kind !== "audiooutput" || !d.deviceId || SPECIAL[d.deviceId]) continue;
        if (d.label) labelled++;
        if (!found && d.label && matches(d.label, state.target)) found = d;
      }
      state.labels = labelled;
      if (found) { state.deviceId = found.deviceId; state.resolved = true; }
      else if (labelled > 0) { state.deviceId = ""; state.resolved = true; }   // Namen sichtbar, Geraet aber weg -> Standard
      else { state.resolved = false; }                                          // Namen noch ausgeblendet (Berechtigung fehlt noch)
    } catch (e) { state.lastError = String(e && e.name || e); state.resolved = false; }
  }

  async function applyTo(el) {
    if (!el || typeof el.setSinkId !== "function") return;
    if (el.sinkId === state.deviceId) return;
    try { await el.setSinkId(state.deviceId); state.applied++; }
    catch (e) { state.lastError = String(e && e.name || e); }
  }

  function applyAll() {
    var els = document.querySelectorAll("video, audio");
    for (var i = 0; i < els.length; i++) applyTo(els[i]);
  }

  async function refresh() {
    await resolveDevice();
    applyAll();
    if (!state.resolved && state.tries++ < 40) {
      clearTimeout(state.timer);
      state.timer = setTimeout(refresh, 2500);
    }
  }

  // Neue/abspielende Medienelemente sofort auf das Ziel legen (YouTube erzeugt sie dynamisch).
  var onMedia = function (e) { applyTo(e.target); };
  document.addEventListener("play", onMedia, true);
  document.addEventListener("loadstart", onMedia, true);
  var pending = null;
  new MutationObserver(function () {
    if (pending) return;
    pending = setTimeout(function () { pending = null; applyAll(); }, 200);
  }).observe(document.documentElement, { childList: true, subtree: true });
  try { navigator.mediaDevices.addEventListener("devicechange", refresh); } catch (e) {}

  window.__playtubeAudio = {
    setTarget: function (t) { state.target = t; state.tries = 0; state.resolved = false; refresh(); },
    status: function () { return { target: state.target, deviceId: state.deviceId, resolved: state.resolved, applied: state.applied, labels: state.labels, lastError: state.lastError }; }
  };
  refresh();
})();
"""


def build_router_script(device_name: str) -> str:
    """JavaScript, das die Medienelemente einer Seite auf das Ausgabegeraet `device_name`
    legt (leer = Systemstandard)."""
    return _ROUTER_TEMPLATE.replace("__TARGET__", json.dumps(device_name or ""))
