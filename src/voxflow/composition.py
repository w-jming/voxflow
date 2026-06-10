from __future__ import annotations

from dataclasses import dataclass

from .postprocess import EditAction


@dataclass(frozen=True, slots=True)
class CompositionCommand:
    kind: str
    value: str | int


class InjectionLedger:
    """Tracks text committed by VoxFlow so correction only touches our text."""

    def __init__(self) -> None:
        self._chunks: list[str] = []

    @property
    def chunks(self) -> tuple[str, ...]:
        return tuple(self._chunks)

    @property
    def committed_text(self) -> str:
        return "".join(self._chunks)

    def record_insert(self, text: str) -> None:
        if text:
            self._chunks.append(text)

    def consume_backspace(self, requested: int) -> int:
        remaining = max(0, requested)
        consumed = 0
        while remaining and self._chunks:
            current = self._chunks[-1]
            take = min(len(current), remaining)
            kept = current[: len(current) - take]
            consumed += take
            remaining -= take
            if kept:
                self._chunks[-1] = kept
            else:
                self._chunks.pop()
        return consumed


def actions_to_composition_commands(
    actions: list[EditAction],
    ledger: InjectionLedger,
) -> list[CompositionCommand]:
    commands: list[CompositionCommand] = []
    for action in actions:
        if action.backspace:
            count = ledger.consume_backspace(action.backspace)
            if count:
                commands.append(CompositionCommand("delete_before_cursor", count))
        if action.insert:
            ledger.record_insert(action.insert)
            commands.append(CompositionCommand("commit", action.insert))
    return commands
