from __future__ import annotations

import re


_LEADING_CN_FILLERS = re.compile(r"^\s*(?:嗯+|呃+|额+|啊+|唔+|呐+)[，,、\s]*")
_SEPARATED_CN_FILLERS = re.compile(r"([，,、\s])(?:嗯+|呃+|额+|啊+|唔+|呐+)(?=[，,、\s])")
_PHRASE_FILLERS = re.compile(r"(?:这个|那个|怎么说呢|你知道吧|对吧|是吧)[，,、\s]*")
_EN_FILLERS = re.compile(r"\b(?:um+|uh+|erm+|hmm+|you know)\b[,\s]*", re.IGNORECASE)


def remove_fillers(text: str) -> str:
    """Remove high-confidence spoken fillers without touching normal phrases."""
    result = _LEADING_CN_FILLERS.sub("", text)
    result = _SEPARATED_CN_FILLERS.sub(r"\1", result)
    result = _PHRASE_FILLERS.sub("", result)
    result = _EN_FILLERS.sub("", result)
    return _cleanup_spaces(result)


def _cleanup_spaces(text: str) -> str:
    text = re.sub(r"\s+([，。！？、,.!?;；:：])", r"\1", text)
    text = re.sub(r"([，。！？、；：])\s+", r"\1", text)
    text = re.sub(r"[ \t]{2,}", " ", text)
    return text.strip()
