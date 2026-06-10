from voxflow.semantic_intent import list_semantic_intent_backends


def test_only_rules_backend_is_currently_available():
    backends = {backend.id: backend for backend in list_semantic_intent_backends()}

    assert backends["rules"].status == "available"
    assert backends["minilm-setfit"].status == "planned"
    assert backends["qwen3-embedding"].status == "planned"
    assert backends["llm-arbiter"].status == "planned"


def test_planned_semantic_backends_have_license_and_model():
    for backend in list_semantic_intent_backends():
        assert backend.model
        assert backend.license
