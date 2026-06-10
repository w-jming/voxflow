from voxflow.input.injector import DryRunInjector, apply_actions
from voxflow.postprocess import EditAction


def test_dry_run_injector_records_actions():
    injector = DryRunInjector()

    apply_actions(injector, [EditAction(backspace=2), EditAction(insert="你好。")])

    assert injector.events[0].kind == "backspace"
    assert injector.events[0].value == 2
    assert injector.events[1].kind == "type"
    assert injector.events[1].value == "你好。"
