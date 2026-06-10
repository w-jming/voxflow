from __future__ import annotations

import re


_END_PUNCT = "。！？.!?"
_QUESTION_RE = re.compile(
    r"(吗|么|是不是|能不能|可不可以|要不要|为什么|怎么|如何|多少|哪[个里]?|what|why|how|when|where|who)\s*$",
    re.IGNORECASE,
)
_CHINESE_RE = re.compile(r"[\u4e00-\u9fff]")
_SPACING_RE = re.compile(r"\s+")


class AutoPunctuator:
    def punctuate(self, text: str) -> str:
        cleaned = normalize_text(text)
        if not cleaned:
            return cleaned
        if cleaned[-1] in _END_PUNCT:
            return cleaned
        if _QUESTION_RE.search(cleaned):
            return cleaned + ("？" if _has_chinese(cleaned) else "?")
        return cleaned + ("。" if _has_chinese(cleaned) else ".")


def normalize_text(text: str) -> str:
    text = text.strip()
    text = _SPACING_RE.sub(" ", text)
    text = re.sub(r"\s+([，。！？、,.!?;；:：])", r"\1", text)
    text = re.sub(r"([，。！？、；：])\s+", r"\1", text)
    text = re.sub(r"\s+([,.!?;:])", r"\1", text)
    return text.strip()


def _has_chinese(text: str) -> bool:
    return bool(_CHINESE_RE.search(text))
