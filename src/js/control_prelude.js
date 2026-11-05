    var isMusic = location.hostname.indexOf('music.youtube.com') !== -1;
    var video = document.querySelector('video');
    var player = document.getElementById('movie_player');
    function has(fn) { return player && typeof player[fn] === 'function'; }
    function click(selectors) {
        for (var i = 0; i < selectors.length; i++) {
            var el = document.querySelector(selectors[i]);
            if (el) { el.click(); return true; }
        }
        return false;
    }
    function clamp(v) { return Math.max(0, Math.min(100, Math.round(v))); }
    function getVolume() {
        if (has('getVolume')) { return player.getVolume(); }
        return video ? video.volume * 100 : 0;
    }
    function unmute() {
        if (has('unMute')) { player.unMute(); } else if (video) { video.muted = false; }
    }
    function mute() {
        if (has('mute')) { player.mute(); } else if (video) { video.muted = true; }
    }
    function isMuted() {
        if (has('isMuted')) { return player.isMuted(); }
        return video ? video.muted : false;
    }
    function setVolume(v) {
        v = clamp(v);
        if (has('setVolume')) { player.setVolume(v); }
        else if (video) { video.volume = v / 100; }
        if (v > 0) { unmute(); }
    }
