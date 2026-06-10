from __future__ import annotations

from dataclasses import dataclass
import re

from .fillers import remove_fillers
from .punctuation import AutoPunctuator, normalize_text


_COMMAND_RE = re.compile(
    r"(?P<undo>删除\s*刚才|撤\s*回(?:\s*刚才|\s*上一\s*句)?|重\s*来|重新\s*来)"
    r"|(?P<repair>(?:哦|噢|喔)\s*不|不\s*对|错\s*了)"
    r"|(?P<repair_not>^不\s*是|(?<=[，,。.!?！？；;\s])不\s*是)"
)
_BOUNDARY_CHARS = "，,。.!?！？；;：:\n"
_TIME_OR_NUMBER_RE = re.compile(
    r"(今天|明天|后天|昨天|前天|上午|下午|中午|晚上|早上|凌晨|"
    r"周[一二三四五六日天]|星期[一二三四五六日天]|礼拜[一二三四五六日天]|"
    r"(?:[零一二三四五六七八九十两\d]+)(?:点半?|分钟?|小时|号|日|月|年|个|位|次|块|元))$"
)
_EN_WORD_RE = re.compile(r"[A-Za-z0-9_./+-]+$")


@dataclass(slots=True)
class EditAction:
    insert: str = ""
    backspace: int = 0
    reason: str = ""

    @property
    def is_noop(self) -> bool:
        return not self.insert and self.backspace <= 0


class TextHistory:
    def __init__(self) -> None:
        self._chunks: list[str] = []

    @property
    def chunks(self) -> tuple[str, ...]:
        return tuple(self._chunks)

    def record(self, text: str) -> None:
        if text:
            self._chunks.append(text)

    def delete_last_unit(self) -> tuple[int, str]:
        while self._chunks and not self._chunks[-1]:
            self._chunks.pop()
        if not self._chunks:
            return 0, ""

        current = self._chunks[-1]
        start = _history_delete_start(current)
        kept = current[:start].rstrip()
        deleted = current[len(kept) :]
        if kept:
            self._chunks[-1] = kept
        else:
            self._chunks.pop()
        return len(deleted), deleted


class DictationSession:
    """Stateful text post-processor for one dictation session.

    The command scan is linear in the transcript length. Already inserted text is
    tracked as chunks, so undo commands only need to inspect the latest chunk.
    """

    def __init__(self, remove_spoken_fillers: bool = True, auto_punctuation: bool = True) -> None:
        self.remove_spoken_fillers = remove_spoken_fillers
        self.auto_punctuation = auto_punctuation
        self.history = TextHistory()
        self.punctuator = AutoPunctuator()

    def process(self, raw_text: str) -> list[EditAction]:
        text = _strip_model_tokens(raw_text)
        if not text:
            return []

        actions: list[EditAction] = []
        buffer = ""
        cursor = 0

        for match in _COMMAND_RE.finditer(text):
            prefix = text[cursor : match.start()]
            buffer = _append_clean(buffer, prefix, self.remove_spoken_fillers)
            command = match.group(0)

            if match.lastgroup == "undo":
                buffer = ""
                count, deleted = self.history.delete_last_unit()
                if count:
                    actions.append(EditAction(backspace=count, reason=f"undo:{deleted}"))
            else:
                if buffer:
                    buffer = _remove_local_repair_target(buffer)
                else:
                    count, deleted = self.history.delete_last_unit()
                    if count:
                        actions.append(EditAction(backspace=count, reason=f"repair:{command}:{deleted}"))

            cursor = match.end()

        buffer = _append_clean(buffer, text[cursor:], self.remove_spoken_fillers)
        final_text = self._finalize_insert(buffer)
        if final_text:
            actions.append(EditAction(insert=final_text))
            self.history.record(final_text)

        return [action for action in actions if not action.is_noop]

    def _finalize_insert(self, text: str) -> str:
        cleaned = normalize_text(text)
        if not cleaned:
            return ""
        if self.auto_punctuation:
            return self.punctuator.punctuate(cleaned)
        return cleaned


def _append_clean(buffer: str, text: str, should_remove_fillers: bool) -> str:
    cleaned = remove_fillers(text) if should_remove_fillers else normalize_text(text)
    if not cleaned:
        return buffer
    if _needs_space(buffer, cleaned):
        return f"{buffer} {cleaned}"
    return f"{buffer}{cleaned}"


def _needs_space(left: str, right: str) -> bool:
    return bool(left and right and left[-1].isascii() and right[0].isascii() and left[-1].isalnum() and right[0].isalnum())


def _strip_model_tokens(text: str) -> str:
    text = re.sub(r"<\|[^>]+?\|>", "", text)
    return normalize_text(text)


def _remove_local_repair_target(buffer: str) -> str:
    stripped = buffer.rstrip()
    if not stripped:
        return ""
    start = _repair_target_start(stripped)
    return stripped[:start].rstrip()


def _repair_target_start(text: str) -> int:
    boundary = max(text.rfind(ch) for ch in _BOUNDARY_CHARS)
    clause_start = boundary + 1 if boundary >= 0 else 0
    clause = text[clause_start:].strip()
    leading_spaces = len(text[clause_start:]) - len(text[clause_start:].lstrip())
    clause_offset = clause_start + leading_spaces

    if not clause:
        return clause_start

    time_match = _TIME_OR_NUMBER_RE.search(clause)
    if time_match:
        return clause_offset + time_match.start()

    en_match = _EN_WORD_RE.search(clause)
    if en_match and en_match.start() > 0:
        return clause_offset + en_match.start()

    if " " in clause:
        return clause_offset + clause.rstrip().rfind(" ") + 1

    return clause_start


def _history_delete_start(text: str) -> int:
    stripped = text.rstrip()
    if not stripped:
        return 0

    search = stripped
    if search[-1] in "。.!?！？":
        search = search[:-1]

    boundary = max(search.rfind(ch) for ch in "，,；;：:\n")
    if boundary >= 0:
        return boundary + 1
    return 0
