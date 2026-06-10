import tomllib
from pathlib import Path

from local_speak_input import __version__


def test_package_version_matches_pyproject():
    data = tomllib.loads(Path("pyproject.toml").read_text(encoding="utf-8"))

    assert __version__ == data["project"]["version"]
