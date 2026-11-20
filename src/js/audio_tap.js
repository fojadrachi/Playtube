(function() {
    // Playtube-Ton: Der Ton der Seite wird hier abgegriffen (Web Audio) und an Playtube geschickt,
    // das ihn selbst ausgibt (src/relay.rs). So kommt der Ton aus Playtube.exe statt aus dem
    // Hilfsprozess msedgewebview2.exe: Discord überträgt ihn beim Teilen des Fensters, und Sonar &
    // Co. zeigen "Playtube" an. Ist die Funktion aus, läuft der Ton wie gewohnt über die Seite.
    if (window.__playtubeTap) { return; }
    var DEFAULT_ON = __ENABLED__;
    var state = { enabled: DEFAULT_ON, ctx: null, taps: [] };
    window.__playtubeTap = state;

    function post(text) {
        try { if (window.ipc && window.ipc.postMessage) { window.ipc.postMessage(text); } } catch (e) {}
    }

    function context() {
        if (state.ctx) { return state.ctx; }
        var options = { sampleRate: 48000, latencyHint: 'interactive' };
        try { options.sinkId = { type: 'none' }; } catch (e) {}
        try { state.ctx = new AudioContext(options); }
        catch (e) { state.ctx = new AudioContext({ sampleRate: 48000 }); }
        return state.ctx;
    }

    // Float -> 16 Bit, als Base64 (kompakt für die Übergabe an Playtube).
    function encode(left, right, gain) {
        var n = left.length;
        var bytes = new Uint8Array(n * 4);
        for (var i = 0; i < n; i++) {
            var l = Math.max(-1, Math.min(1, left[i] * gain)) * 32767;
            var r = Math.max(-1, Math.min(1, right[i] * gain)) * 32767;
            var a = l | 0, b = r | 0;
            bytes[i * 4] = a & 255; bytes[i * 4 + 1] = (a >> 8) & 255;
            bytes[i * 4 + 2] = b & 255; bytes[i * 4 + 3] = (b >> 8) & 255;
        }
        var text = '';
        for (var j = 0; j < bytes.length; j += 8192) {
            text += String.fromCharCode.apply(null, bytes.subarray(j, j + 8192));
        }
        return btoa(text);
    }

    // Lautstärke „virtuell“ machen: Die Seite (YouTube) liest und setzt volume/muted wie gewohnt,
    // das Element selbst bleibt aber auf 100 % und nicht stumm. Playtube wendet die Lautstärke erst bei
    // der eigenen Ausgabe an (src/main.rs). So bekommt ein Discord-Bot den Ton unabhängig vom Regler.
    var PROTO = HTMLMediaElement.prototype;
    var VOLUME = Object.getOwnPropertyDescriptor(PROTO, 'volume');
    var MUTED = Object.getOwnPropertyDescriptor(PROTO, 'muted');
    function virtualize(el) {
        if (el.__playtubeVolume) { return; }
        var volume = VOLUME.get.call(el), muted = MUTED.get.call(el);
        try {
            Object.defineProperty(el, 'volume', { configurable: true, get: function() { return volume; },
                set: function(v) {
                    v = Number(v);
                    if (!(v >= 0 && v <= 1)) { throw new DOMException('volume außerhalb von 0–1', 'IndexSizeError'); }
                    if (v !== volume) { volume = v; el.dispatchEvent(new Event('volumechange')); }
                } });
            Object.defineProperty(el, 'muted', { configurable: true, get: function() { return muted; },
                set: function(m) {
                    m = !!m;
                    if (m !== muted) { muted = m; el.dispatchEvent(new Event('volumechange')); }
                } });
        } catch (e) { return; }
        VOLUME.set.call(el, 1);
        MUTED.set.call(el, false);
        el.__playtubeVolume = {
            level: function() { return muted ? 0 : volume; },
            restore: function() {
                delete el.volume; delete el.muted;
                VOLUME.set.call(el, volume); MUTED.set.call(el, muted);
                el.__playtubeVolume = null;
            }
        };
    }

    function tap(el) {
        if (el.__playtubeTap || !state.enabled) { return; }
        var ctx = context();
        var source;
        try { source = ctx.createMediaElementSource(el); } catch (e) { return; }
        // 2048 Frames (~43 ms) je Stück: weniger Aufrufe, robuster gegen kurze Ruckler der Seite.
        var proc = ctx.createScriptProcessor(2048, 2, 2);
        var sink = ctx.createMediaStreamDestination();
        source.connect(proc);
        proc.connect(sink);
        proc.onaudioprocess = function(event) {
            var input = event.inputBuffer;
            if (!state.enabled) { return; }
            var left = input.getChannelData(0);
            var right = input.numberOfChannels > 1 ? input.getChannelData(1) : left;
            // Stille (Pause) nicht übertragen - Playtube füllt Lücken selbst mit Stille.
            var loud = false;
            for (var i = 0; i < left.length; i += 16) { if (left[i] !== 0 || right[i] !== 0) { loud = true; break; } }
            if (!loud) { return; }
            // Der Ton kommt mit voller Lautstärke; die Lautstärke der Seite geht mit und wird erst bei der
            // Ausgabe am PC angewendet. Klappt das Virtualisieren nicht, hat Chrome sie schon angewendet (1).
            var level = el.__playtubeVolume ? el.__playtubeVolume.level() : 1;
            post('pcm:' + ctx.sampleRate + ':' + level.toFixed(4) + ':' + encode(left, right, 1));
        };
        virtualize(el);
        el.__playtubeTap = { source: source, proc: proc, el: el };
        state.taps.push(el.__playtubeTap);
        if (ctx.state === 'suspended') { ctx.resume(); }
    }

    function scan() {
        var media = document.querySelectorAll('video, audio');
        for (var i = 0; i < media.length; i++) { tap(media[i]); }
    }

    // Jede Wiedergabe erfassen (auch Elemente, die erst später entstehen).
    document.addEventListener('play', function(e) {
        if (e.target && (e.target.tagName === 'VIDEO' || e.target.tagName === 'AUDIO')) {
            tap(e.target);
            if (state.ctx && state.ctx.state === 'suspended') { state.ctx.resume(); }
        }
    }, true);

    // An/aus zur Laufzeit (Einstellung). Schon erfasste Elemente gehen beim Ausschalten wieder
    // direkt über die Seite raus.
    state.enable = function(on) {
        state.enabled = !!on;
        if (state.enabled) {
            if (state.ctx && state.ctx.setSinkId) { try { state.ctx.setSinkId({ type: 'none' }); } catch (e) {} }
            for (var i = 0; i < state.taps.length; i++) {
                try { state.taps[i].source.disconnect(); state.taps[i].source.connect(state.taps[i].proc); } catch (e) {}
                virtualize(state.taps[i].el);
            }
            scan();
        } else if (state.ctx) {
            if (state.ctx.setSinkId) { try { state.ctx.setSinkId(''); } catch (e) {} }
            for (var k = 0; k < state.taps.length; k++) {
                try { state.taps[k].source.disconnect(); state.taps[k].source.connect(state.ctx.destination); } catch (e) {}
                // Seite gibt den Ton wieder selbst aus: echte Lautstärke zurück aufs Element.
                if (state.taps[k].el.__playtubeVolume) { state.taps[k].el.__playtubeVolume.restore(); }
            }
        }
        return state.enabled;
    };
})();
