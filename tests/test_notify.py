import subprocess

from local_speak_input.notify import Notifier


def test_notifier_ignores_notify_send_timeout(monkeypatch):
    monkeypatch.setattr("local_speak_input.notify.shutil.which", lambda _name: "/usr/bin/notify-send")

    def raise_timeout(*_args, **_kwargs):
        raise subprocess.TimeoutExpired(["notify-send"], timeout=2)

    monkeypatch.setattr("local_speak_input.notify.subprocess.run", raise_timeout)

    Notifier().send("开始录音", "请说话")
