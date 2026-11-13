(function() {
    // Playtube Jam: Stand des Players im YouTube-Music-Tab (src/jam_ui.rs gleicht ihn mit dem Jam ab).
    // Die Video-ID kommt vom Player selbst: so darf man in YouTube Music weiter stöbern (Suche,
    // Bibliothek …), während der Jam-Song unten im Player weiterläuft.
    var player = document.getElementById('movie_player');
    // Es kann mehrere <video> geben (Vorladen) - das des Players nehmen, sonst das bereite.
    var video = (player && player.querySelector('video')) || null;
    if (!video || !video.readyState) {
        var all = document.querySelectorAll('video');
        for (var i = 0; i < all.length; i++) { if (all[i].readyState > 0) { video = all[i]; break; } }
    }
    if (!video) { video = document.querySelector('video'); }
    function api(fn) { return player && typeof player[fn] === 'function'; }
    var state = api('getPlayerState') ? player.getPlayerState() : null;  // 1 spielt, 2 Pause, 3 lädt, 0 Ende
    var id = null;
    try {
        var data = (player && typeof player.getVideoData === 'function') ? player.getVideoData() : null;
        id = data && data.video_id ? data.video_id : null;
    } catch (e) { id = null; }
    if (!id) {
        try {
            if (location.pathname === '/watch') { id = new URL(location.href).searchParams.get('v'); }
        } catch (e) { id = null; }
    }
    var meta = navigator.mediaSession ? navigator.mediaSession.metadata : null;
    // getPlayerState ist bei YouTube Music nicht immer verlässlich (-1 trotz Wiedergabe) - dann
    // zählt der Status der MediaSession, zuletzt das <video>.
    var session = navigator.mediaSession ? navigator.mediaSession.playbackState : 'none';
    var paused;
    if (state === 1 || state === 3) { paused = false; }
    else if (state === 2) { paused = true; }
    else if (session === 'playing' || session === 'paused') { paused = session === 'paused'; }
    else { paused = video ? video.paused : true; }
    var ad = !!document.querySelector('.ad-showing, .ad-interrupting, ytmusic-player[player-ui-state="AD"]');
    var error = !!document.querySelector('.ytp-error, yt-playability-error-supported-renderers:not([hidden]) .reason, ytmusic-player .error-overlay:not([hidden])');
    return JSON.stringify({
        id: id,
        t: api('getCurrentTime') ? player.getCurrentTime() : (video ? video.currentTime : 0),
        d: api('getDuration') && player.getDuration() > 0 ? player.getDuration()
            : ((video && isFinite(video.duration)) ? video.duration : 0),
        paused: paused,
        ended: state === 0 || (video ? video.ended : false),
        ready: video ? video.readyState : 0,
        rate: video && video.playbackRate > 0 ? video.playbackRate : 1,
        now: Date.now(),
        state: state,
        session: session,
        ad: ad,
        error: error,
        title: meta && meta.title ? meta.title : null,
        artist: meta && meta.artist ? meta.artist : null,
        host: location.hostname + location.pathname
    });
})();
