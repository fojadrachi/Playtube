(function() {
    // Playtube Jam: Stand des Players im YouTube-Music-Tab (src/jam.rs gleicht ihn mit dem Jam ab).
    // Nach dem Laden über die URL gehört video.currentTime sicher zum aktuellen Titel.
    var video = document.querySelector('video');
    var id = null;
    try {
        if (location.pathname === '/watch') { id = new URL(location.href).searchParams.get('v'); }
    } catch (e) { id = null; }
    var ad = !!document.querySelector('.ad-showing, .ad-interrupting, ytmusic-player[player-ui-state="AD"]');
    var error = !!document.querySelector('.ytp-error, yt-playability-error-supported-renderers:not([hidden]) .reason, ytmusic-player .error-overlay:not([hidden])');
    return JSON.stringify({
        id: id,
        t: video ? video.currentTime : 0,
        d: (video && isFinite(video.duration)) ? video.duration : 0,
        paused: video ? video.paused : true,
        ended: video ? video.ended : false,
        ready: video ? video.readyState : 0,
        ad: ad,
        error: error,
        host: location.hostname + location.pathname
    });
})();
