from __future__ import annotations

import argparse
import json
from pathlib import Path

from .model_registry import download_model_profile


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="voxflow-download-worker")
    parser.add_argument("--profile", required=True)
    parser.add_argument("--dir", type=Path, required=True)
    args = parser.parse_args(argv)

    path = download_model_profile(args.profile, args.dir)
    print(json.dumps({"model_profile": args.profile, "path": str(path)}, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
