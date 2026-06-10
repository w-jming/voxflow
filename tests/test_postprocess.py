from voxflow.postprocess import DictationSession


def inserts(actions):
    return "".join(action.insert for action in actions if action.insert)


def test_removes_spoken_fillers_and_adds_punctuation():
    session = DictationSession()

    actions = session.process("嗯，我今天下午开会")

    assert inserts(actions) == "我今天下午开会。"


def test_default_script_converts_traditional_to_simplified():
    session = DictationSession()

    actions = session.process("這是語音輸入測試")

    assert inserts(actions) == "这是语音输入测试。"


def test_traditional_script_is_configurable():
    session = DictationSession(script="traditional")

    actions = session.process("这是语音输入测试")

    assert inserts(actions) == "這是語音輸入測試。"


def test_local_repair_keeps_prefix_and_replaces_time():
    session = DictationSession()

    actions = session.process("今天下午三点哦不四点")

    assert inserts(actions) == "今天下午四点。"
    assert not any(action.backspace for action in actions)


def test_sentence_not_contains_no_false_correction():
    session = DictationSession()

    actions = session.process("这不是一个问题")

    assert inserts(actions) == "这不是一个问题。"
    assert not any(action.backspace for action in actions)


def test_semantic_correction_can_be_disabled():
    session = DictationSession(semantic_correction_enabled=False)

    actions = session.process("今天下午三点不对四点")

    assert inserts(actions) == "今天下午三点不对四点。"
    assert not any(action.backspace for action in actions)


def test_initial_not_repair_deletes_previous_inserted_sentence():
    session = DictationSession()
    first = session.process("今天下午三点")
    second = session.process("不是四点")

    assert inserts(first) == "今天下午三点。"
    assert second[0].backspace == len("今天下午三点。")
    assert inserts(second) == "四点。"


def test_undo_command_deletes_last_clause():
    session = DictationSession()
    session.process("第一句，第二句")

    actions = session.process("撤回刚才")

    assert actions[0].backspace == len("第二句。")
