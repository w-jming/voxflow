from voxflow.composition import InjectionLedger, actions_to_composition_commands
from voxflow.postprocess import DictationSession, EditAction


def test_ledger_limits_delete_to_voxflow_committed_text():
    ledger = InjectionLedger()
    ledger.record_insert("你好")

    commands = actions_to_composition_commands([EditAction(backspace=8)], ledger)

    assert commands[0].kind == "delete_before_cursor"
    assert commands[0].value == 2
    assert ledger.committed_text == ""


def test_actions_update_ledger_in_order():
    ledger = InjectionLedger()

    commands = actions_to_composition_commands([EditAction(insert="今天三点。")], ledger)

    assert commands[0].kind == "commit"
    assert commands[0].value == "今天三点。"
    assert ledger.committed_text == "今天三点。"


def test_semantic_repair_keeps_literal_not_wrong_phrase():
    session = DictationSession()

    actions = session.process("不是不对是另一个意思")

    assert "".join(action.insert for action in actions if action.insert) == "不是不对是另一个意思。"
    assert not any(action.backspace for action in actions)


def test_semantic_repair_can_delete_previous_voxflow_phrase():
    session = DictationSession()
    ledger = InjectionLedger()
    first = session.process("今天下午三点")
    actions_to_composition_commands(first, ledger)

    second = session.process("不对四点")
    commands = actions_to_composition_commands(second, ledger)

    assert commands[0].kind == "delete_before_cursor"
    assert commands[0].value == len("今天下午三点。")
    assert commands[1].kind == "commit"
    assert commands[1].value == "四点。"
