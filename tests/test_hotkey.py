from local_speak_input.hotkey import CONTROL_MASK, MOD1_MASK, SHIFT_MASK, parse_hotkey


def test_parse_hotkey_ctrl_alt_space():
    spec = parse_hotkey("ctrl+alt+space")

    assert spec.key == "space"
    assert spec.modifiers == CONTROL_MASK | MOD1_MASK


def test_parse_hotkey_angle_bracket_format():
    spec = parse_hotkey("<Control><Shift>Return")

    assert spec.key == "Return"
    assert spec.modifiers == CONTROL_MASK | SHIFT_MASK


def test_parse_hotkey_rejects_multiple_regular_keys():
    try:
        parse_hotkey("ctrl+a+b")
    except ValueError as exc:
        assert "只包含一个普通按键" in str(exc)
    else:
        raise AssertionError("expected ValueError")
