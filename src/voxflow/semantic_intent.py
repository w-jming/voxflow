from __future__ import annotations

from dataclasses import asdict, dataclass


@dataclass(frozen=True, slots=True)
class SemanticIntentBackend:
    id: str
    label: str
    model: str
    license: str
    status: str
    description: str

    def to_dict(self) -> dict[str, str]:
        return asdict(self)


SEMANTIC_INTENT_BACKENDS = {
    "rules": SemanticIntentBackend(
        id="rules",
        label="规则状态机",
        model="built-in",
        license="MIT",
        status="available",
        description="默认可用，结合上下文规则和 VoxFlow 注入账本执行撤销。",
    ),
    "minilm-setfit": SemanticIntentBackend(
        id="minilm-setfit",
        label="MiniLM / SetFit 分类头",
        model="sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2",
        license="Apache-2.0",
        status="planned",
        description="少量标注样本微调后作为本地低延迟语义意图分类器。",
    ),
    "qwen3-embedding": SemanticIntentBackend(
        id="qwen3-embedding",
        label="Qwen3-Embedding 0.6B 分类头",
        model="Qwen/Qwen3-Embedding-0.6B",
        license="Apache-2.0",
        status="planned",
        description="高准确率语义意图分类底座，需要训练分类头。",
    ),
    "llm-arbiter": SemanticIntentBackend(
        id="llm-arbiter",
        label="LLM 低置信仲裁",
        model="configurable",
        license="model-dependent",
        status="planned",
        description="仅在低置信时输出结构化建议，删除仍必须经过账本验证。",
    ),
}


def list_semantic_intent_backends(*, include_planned: bool = False) -> list[SemanticIntentBackend]:
    backends = list(SEMANTIC_INTENT_BACKENDS.values())
    if include_planned:
        return backends
    return [backend for backend in backends if backend.status == "available"]
