"""Tests fuer die lokale Fernsteuerung (playtube/remote_control.py, media_control.py).

Start:  .venv\\Scripts\\python -m unittest discover -s tests -v
"""
from __future__ import annotations

import json
import sys
import threading
import unittest
import uuid
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from playtube.media_control import MEDIA_COMMANDS, build_control_js  # noqa: E402
from playtube.remote_control import (  # noqa: E402
    ProtocolError,
    Request,
    build_state,
    handle_request,
    parse_request,
    process_line,
    resolve_target,
    sanitize_playlists,
)


class FakeController:
    def __init__(self) -> None:
        self.calls: list[tuple] = []

    def remote_state(self) -> dict:
        return {"protocol": 1}

    def remote_show(self) -> None:
        self.calls.append(("show",))

    def remote_switch_tab(self, target: str) -> None:
        self.calls.append(("switch_tab", target))

    def remote_media_command(self, target: str, command: str, value: float) -> None:
        self.calls.append(("media", target, command, value))

    def remote_list_playlists(self, callback) -> None:
        self.pending_playlist_callback = callback

    def remote_play_playlist(self, playlist_id: str) -> None:
        self.calls.append(("play_playlist", playlist_id))


def run(request: Request, controller: FakeController) -> list[dict]:
    replies: list[dict] = []
    handle_request(request, controller, replies.append)
    return replies


def run_line(line: str, controller: FakeController) -> list[dict]:
    written: list[bytes] = []
    process_line(line, controller, written.append)
    return [json.loads(chunk) for chunk in written]


class ParseRequestTests(unittest.TestCase):
    def test_parses_media_command_with_default_target(self):
        request = parse_request('{"id": 7, "cmd": "play_pause"}')
        self.assertEqual(request, Request(id=7, command="play_pause", target="auto", value=0))

    def test_parses_ranged_value(self):
        request = parse_request('{"cmd": "seek", "target": "music", "value": -10}')
        self.assertEqual((request.target, request.value), ("music", -10.0))

    def test_ignores_value_for_commands_without_range(self):
        self.assertEqual(parse_request('{"cmd": "next", "value": "evil()"}').value, 0)

    def test_rejects_invalid_input(self):
        bad_lines = [
            "not json",
            "[1, 2]",
            '{"cmd": "eval"}',
            '{"cmd": ["play_pause"]}',
            '{"cmd": "next", "target": "https://example.com"}',
            '{"cmd": "next", "id": {"x": 1}}',
            '{"cmd": "next", "id": true}',
            '{"cmd": "set_volume"}',
            '{"cmd": "set_volume", "value": "50"}',
            '{"cmd": "set_volume", "value": true}',
            '{"cmd": "set_volume", "value": 101}',
            '{"cmd": "seek", "value": 99999}',
            '{"cmd": "volume_change", "value": NaN}',
            '{"cmd": "play_playlist"}',
            '{"cmd": "play_playlist", "playlist": "x"}',
            '{"cmd": "play_playlist", "playlist": "PL1&autoplay=0"}',
            '{"cmd": "play_playlist", "playlist": "../evil"}',
            '{"cmd": "play_playlist", "playlist": 123}',
        ]
        for line in bad_lines:
            with self.subTest(line=line), self.assertRaises(ProtocolError):
                parse_request(line)


class HandleRequestTests(unittest.TestCase):
    def test_status_returns_state(self):
        replies = run(Request(id=1, command="status"), FakeController())
        self.assertEqual(replies, [{"ok": True, "state": {"protocol": 1}}])

    def test_dispatches_app_and_media_commands(self):
        controller = FakeController()
        run(Request(id=1, command="show"), controller)
        run(Request(id=2, command="switch_tab", target="music"), controller)
        run(Request(id=3, command="volume_change", target="youtube", value=5), controller)
        run(parse_request('{"cmd": "play_playlist", "playlist": "PLabc_-1"}'), controller)
        self.assertEqual(
            controller.calls,
            [
                ("show",),
                ("switch_tab", "music"),
                ("media", "youtube", "volume_change", 5),
                ("play_playlist", "PLabc_-1"),
            ],
        )

    def test_list_playlists_replies_when_page_answers(self):
        controller = FakeController()
        written: list[bytes] = []
        process_line('{"id": 9, "cmd": "list_playlists"}', controller, written.append)
        self.assertEqual(written, [])  # Seite hat noch nicht geantwortet

        controller.pending_playlist_callback([{"id": "PL1", "title": "Mix"}])
        self.assertEqual(
            [json.loads(chunk) for chunk in written],
            [{"id": 9, "ok": True, "playlists": [
                {"id": "LM", "title": "Titel, die ich mag"},
                {"id": "PL1", "title": "Mix"},
            ]}],
        )

    def test_process_line_reports_protocol_errors_without_raising(self):
        [reply] = run_line('{"cmd": "nope"}', FakeController())
        self.assertFalse(reply["ok"])
        self.assertIsNone(reply["id"])

    def test_process_line_hides_internal_errors(self):
        class Broken(FakeController):
            def remote_show(self) -> None:
                raise RuntimeError("secret detail")

        [reply] = run_line('{"id": 4, "cmd": "show"}', Broken())
        self.assertEqual(reply, {"id": 4, "ok": False, "error": "interner Fehler"})


class SanitizePlaylistsTests(unittest.TestCase):
    def test_keeps_valid_entries_and_puts_liked_music_first(self):
        raw = [
            {"id": "PL1", "title": "  Mein   Mix 	"},
            {"id": "LM", "title": "Liked music"},
            {"id": "PL1", "title": "Duplikat"},
            {"id": "bad id!", "title": "x"},
            {"id": "PL2", "title": None},
            "kein Objekt",
        ]
        self.assertEqual(
            sanitize_playlists(raw),
            [
                {"id": "LM", "title": "Titel, die ich mag"},
                {"id": "PL1", "title": "Mein Mix"},
                {"id": "PL2", "title": "PL2"},
            ],
        )

    def test_handles_garbage_and_limits_size(self):
        self.assertEqual(len(sanitize_playlists(None)), 1)
        many = [{"id": f"PL{i:03d}", "title": "t" * 500} for i in range(500)]
        result = sanitize_playlists(many)
        self.assertEqual(len(result), 100)
        self.assertTrue(all(len(item["title"]) <= 100 for item in result))


class ResolveTargetTests(unittest.TestCase):
    def test_explicit_target_wins(self):
        self.assertEqual(resolve_target("music", "youtube", {}), "music")

    def test_auto_prefers_the_playing_tab(self):
        tabs = {"youtube": {"playing": False}, "music": {"playing": True}}
        self.assertEqual(resolve_target("auto", "youtube", tabs), "music")

    def test_auto_uses_visible_tab_when_both_or_none_play(self):
        both = {"youtube": {"playing": True}, "music": {"playing": True}}
        self.assertEqual(resolve_target("auto", "music", both), "music")
        self.assertEqual(resolve_target("auto", "youtube", {}), "youtube")

    def test_auto_from_settings_falls_back_to_tab_with_video(self):
        tabs = {"youtube": {"hasVideo": False}, "music": {"hasVideo": True}}
        self.assertEqual(resolve_target("auto", "settings", tabs), "music")
        self.assertEqual(resolve_target("auto", "settings", {}), "youtube")


class BuildStateTests(unittest.TestCase):
    def test_exposes_only_public_fields(self):
        tabs = {"youtube": {"title": "T", "playing": True, "url": "https://x"}, "music": None}
        state = build_state("9.9.9", "youtube", tabs)
        self.assertEqual(state["activeTab"], "youtube")
        self.assertIsNone(state["tabs"]["music"])
        self.assertEqual(state["tabs"]["youtube"]["title"], "T")
        self.assertNotIn("url", state["tabs"]["youtube"])


class BuildControlJsTests(unittest.TestCase):
    def test_every_command_builds_a_wrapped_script(self):
        for command in MEDIA_COMMANDS:
            with self.subTest(command=command):
                script = build_control_js(command, 5)
                self.assertTrue(script.startswith("(function() {"))
                self.assertNotIn("{value}", script)

    def test_value_is_inserted_as_number(self):
        self.assertIn("setVolume(42.0)", build_control_js("set_volume", 42))

    def test_rejects_unknown_command_and_non_finite_value(self):
        with self.assertRaises(ValueError):
            build_control_js("alert", 0)
        with self.assertRaises(ValueError):
            build_control_js("seek", float("inf"))


@unittest.skipUnless(sys.platform == "win32", "Named-Pipe-Test nur unter Windows")
class NamedPipeIntegrationTests(unittest.TestCase):
    """Echte Named Pipe: Client in einem Thread, Server im Qt-Event-Loop."""

    def test_round_trip_over_named_pipe(self):
        from PySide6.QtCore import QCoreApplication, QTimer

        from playtube.remote_control import RemoteControlServer

        app = QCoreApplication.instance() or QCoreApplication([])
        key = f"Playtube.Remote.Test.{uuid.uuid4().hex}"
        controller = FakeController()
        server = RemoteControlServer(key, controller)
        self.assertTrue(server.start())

        replies: list[dict] = []
        errors: list[BaseException] = []

        def client() -> None:
            try:
                with open(rf"\\.\pipe\{key}", "r+b", buffering=0) as pipe:
                    pipe.write(b'{"id": 1, "cmd": "next"}\n{"id": 2, "cmd": "status"}\n')
                    data = b""
                    while data.count(b"\n") < 2:
                        chunk = pipe.read(4096)
                        if not chunk:
                            break
                        data += chunk
                replies.extend(json.loads(line) for line in data.splitlines())
            except BaseException as exc:  # noqa: BLE001 - im Hauptthread auswerten
                errors.append(exc)
            finally:
                QTimer.singleShot(0, app.quit)

        thread = threading.Thread(target=client, daemon=True)
        QTimer.singleShot(0, thread.start)
        QTimer.singleShot(5000, app.quit)  # Sicherheitsnetz gegen Haengen
        app.exec()
        thread.join(2)

        self.assertEqual(errors, [])
        self.assertEqual(replies, [{"id": 1, "ok": True}, {"id": 2, "ok": True, "state": {"protocol": 1}}])
        self.assertEqual(controller.calls, [("media", "auto", "next", 0)])


if __name__ == "__main__":
    unittest.main()
