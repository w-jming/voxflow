from __future__ import annotations

from functools import lru_cache


_OPENCC_CONFIGS = {
    "simplified": "t2s.json",
    "traditional": "s2t.json",
}


def convert_script(text: str, script: str) -> str:
    if script == "original" or not text:
        return text
    converter = _opencc_converter(script)
    return converter.convert(text)


@lru_cache(maxsize=2)
def _opencc_converter(script: str) -> object:
    config = _OPENCC_CONFIGS.get(script)
    if not config:
        raise ValueError("文本字形必须是 simplified、traditional 或 original")
    try:
        import opencc
    except Exception as exc:
        raise RuntimeError("缺少 OpenCC 简繁转换库。请安装完整 VoxFlow deb 包或运行 uv pip install 'OpenCC>=1.3.1,<2'。") from exc

    try:
        return opencc.OpenCC(config)
    except Exception:
        legacy_config = config.removesuffix(".json")
        return opencc.OpenCC(legacy_config)
