from voxflow.semantic_intent import list_semantic_intent_backends


def test_only_rules_backend_is_currently_available():
    backends = {backend.id: backend for backend in list_semantic_intent_backends()}

    assert backends["rules"].status == "available"
    assert "minilm-setfit" not in backends
    assert "qwen3-embedding" not in backends
    assert "llm-arbiter" not in backends


def test_planned_backends_are_source_visible_but_not_user_visible():
    backends = {backend.id: backend for backend in list_semantic_intent_backends(include_planned=True)}

    assert backends["minilm-setfit"].status == "planned"
    assert backends["qwen3-embedding"].status == "planned"
    assert backends["llm-arbiter"].status == "planned"


def test_planned_semantic_backends_have_license_and_model():
    for backend in list_semantic_intent_backends(include_planned=True):
        assert backend.model
        assert backend.license
