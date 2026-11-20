(function(id) {
    // Playtube Jam: YouTube Music schaltet am Titelende von selbst zum nächsten Vorschlag (Autoplay).
    // Im Jam hörte man dann kurz einen Song, der gar nicht in der Jam-Liste ist. Deshalb den Jam-Song
    // kurz vor dem Ende anhalten – dann kommt kein Autoplay, und Playtube lädt den nächsten Jam-Song.
    // Selbst gestartete Songs bleiben unberührt (die kommen über src/jam_ui.rs in den Jam).
    var g = window.__playtubeJamGuard;
    if (!g) {
        g = window.__playtubeJamGuard = { id: null, stopped: null };
        document.addEventListener('timeupdate', function(e) {
            var v = e.target;
            if (!g.id || !v || v.tagName !== 'VIDEO') { return; }
            var p = document.getElementById('movie_player');
            if (p && !p.contains(v) && document.querySelectorAll('video').length > 1) { return; }
            var cur = null;
            try { var d = p && typeof p.getVideoData === 'function' ? p.getVideoData() : null; cur = d && d.video_id; } catch (x) { cur = null; }
            if (cur !== g.id) { return; }
            var total = p && typeof p.getDuration === 'function' && p.getDuration() > 0 ? p.getDuration() : v.duration;
            if (!isFinite(total) || total <= 0) { return; }
            if (v.currentTime < total - 2) { g.stopped = null; return; }
            if (v.currentTime >= total - 0.5 && !v.paused && g.stopped !== cur) {
                g.stopped = cur;
                if (p && typeof p.pauseVideo === 'function') { p.pauseVideo(); } else { v.pause(); }
            }
        }, true);
    }
    if (g.id !== id) { g.stopped = null; }
    g.id = id;
})(__ID__);
