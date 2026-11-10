(function() {
    // Keine "Website verlassen?"-Abfrage: Playtube lädt im YouTube-Music-Tab selbst neue Seiten
    // (Playtube-Ansicht, Jam). YouTube/YouTube Music melden dafür einen "beforeunload"-Handler an,
    // der dann jedes Mal nachfragt. Läuft vor allen Seitenskripten und lässt solche Handler ins Leere
    // laufen - die Seite wird also immer verlassen, ohne zu fragen.
    if (window.__playtubeNoLeavePrompt) { return; }
    window.__playtubeNoLeavePrompt = true;
    try {
        var add = EventTarget.prototype.addEventListener;
        EventTarget.prototype.addEventListener = function(type, listener, options) {
            if (this === window && String(type).toLowerCase() === 'beforeunload') { return; }
            return add.call(this, type, listener, options);
        };
    } catch (e) {}
    try {
        Object.defineProperty(window, 'onbeforeunload', {
            configurable: true,
            get: function() { return null; },
            set: function() {}
        });
    } catch (e) {}
})();
