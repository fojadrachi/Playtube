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
