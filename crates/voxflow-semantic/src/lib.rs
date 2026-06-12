use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const DATASET_VERSION: &str = "semantic-intent-synthetic-v0.1";
pub const GENERATED_AT: &str = "2026-06-10T00:00:00Z";
pub const MIN_TOTAL_SAMPLES: usize = 1500;
pub const MIN_PER_LABEL: usize = 150;
pub const MIN_TEST_SAMPLES: usize = 300;
pub const MIN_LITERAL_FALSE_TRIGGER_SHARE: f64 = 0.30;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum IntentLabel {
    Literal,
    UndoLast,
    UndoTarget,
    ReplaceEntity,
    RepairPrevious,
    Uncertain,
}

impl IntentLabel {
    pub fn all() -> [Self; 6] {
        [
            Self::Literal,
            Self::UndoLast,
            Self::UndoTarget,
            Self::ReplaceEntity,
            Self::RepairPrevious,
            Self::Uncertain,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Literal => "literal",
            Self::UndoLast => "undo_last",
            Self::UndoTarget => "undo_target",
            Self::ReplaceEntity => "replace_entity",
            Self::RepairPrevious => "repair_previous",
            Self::Uncertain => "uncertain",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DatasetSplit {
    Train,
    Dev,
    Test,
}

impl DatasetSplit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Dev => "dev",
            Self::Test => "test",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Unreviewed,
    Reviewed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreviousSegment {
    pub id: String,
    pub text: String,
    pub age_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticIntentSample {
    pub id: String,
    pub dataset_version: String,
    pub split: DatasetSplit,
    pub locale: String,
    pub current_text: String,
    pub previous_text: String,
    pub previous_segments: Vec<PreviousSegment>,
    pub language_hint: String,
    pub pause_before_ms: u32,
    pub pause_after_ms: u32,
    pub label: IntentLabel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    pub source: String,
    pub review_status: ReviewStatus,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabelSchema {
    pub version: String,
    pub generated_by: String,
    pub generated_at: String,
    pub test_set_frozen: bool,
    pub labels: Vec<LabelDefinition>,
    pub fields: Vec<FieldDefinition>,
    pub thresholds: ValidationThresholds,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabelDefinition {
    pub label: IntentLabel,
    pub description: String,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldDefinition {
    pub name: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationThresholds {
    pub min_total_samples: usize,
    pub min_per_label: usize,
    pub min_test_samples: usize,
    pub min_literal_false_trigger_share_percent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeneratedDataset {
    pub schema: LabelSchema,
    pub samples: Vec<SemanticIntentSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetValidationReport {
    pub dataset_version: String,
    pub total_samples: usize,
    pub split_counts: BTreeMap<String, usize>,
    pub label_counts: BTreeMap<String, usize>,
    pub test_samples: usize,
    pub literal_false_trigger_negatives: usize,
    pub literal_false_trigger_share: f64,
    pub duplicate_ids: Vec<String>,
    pub errors: Vec<String>,
    pub passed: bool,
}

pub fn generation_plan() -> BTreeMap<IntentLabel, usize> {
    BTreeMap::from([
        (IntentLabel::Literal, 600),
        (IntentLabel::UndoLast, 240),
        (IntentLabel::UndoTarget, 240),
        (IntentLabel::ReplaceEntity, 240),
        (IntentLabel::RepairPrevious, 240),
        (IntentLabel::Uncertain, 240),
    ])
}

pub fn generate_dataset() -> GeneratedDataset {
    let mut samples = Vec::new();
    let mut split_label_counters: BTreeMap<(DatasetSplit, IntentLabel), usize> = BTreeMap::new();

    for (label, count) in generation_plan() {
        let seeds = generate_label_seeds(label, count);
        let train_count = count * 60 / 100;
        let dev_count = count * 20 / 100;
        for (index, seed) in seeds.into_iter().enumerate() {
            let split = if index < train_count {
                DatasetSplit::Train
            } else if index < train_count + dev_count {
                DatasetSplit::Dev
            } else {
                DatasetSplit::Test
            };
            let counter = split_label_counters.entry((split, label)).or_insert(0);
            *counter += 1;
            samples.push(seed.into_sample(label, split, *counter, index));
        }
    }

    GeneratedDataset {
        schema: label_schema(),
        samples,
    }
}

pub fn write_dataset_dir(dataset: &GeneratedDataset, dir: impl AsRef<Path>) -> Result<()> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    write_pretty_json(&dir.join("label_schema.json"), &dataset.schema)?;
    for split in [DatasetSplit::Train, DatasetSplit::Dev, DatasetSplit::Test] {
        let split_samples = dataset
            .samples
            .iter()
            .filter(|sample| sample.split == split)
            .collect::<Vec<_>>();
        write_jsonl(
            &dir.join(format!("{}.jsonl", split.as_str())),
            &split_samples,
        )?;
    }
    Ok(())
}

pub fn read_dataset_dir(dir: impl AsRef<Path>) -> Result<GeneratedDataset> {
    let dir = dir.as_ref();
    let schema_path = dir.join("label_schema.json");
    let schema = serde_json::from_reader(
        File::open(&schema_path).with_context(|| format!("open {}", schema_path.display()))?,
    )
    .with_context(|| format!("parse {}", schema_path.display()))?;
    let mut samples = Vec::new();
    for split in [DatasetSplit::Train, DatasetSplit::Dev, DatasetSplit::Test] {
        samples.extend(read_jsonl(
            &dir.join(format!("{}.jsonl", split.as_str())),
            split,
        )?);
    }
    Ok(GeneratedDataset { schema, samples })
}

pub fn validate_dataset_dir(dir: impl AsRef<Path>) -> Result<DatasetValidationReport> {
    validate_dataset(&read_dataset_dir(dir)?)
}

pub fn validate_dataset(dataset: &GeneratedDataset) -> Result<DatasetValidationReport> {
    let mut errors = Vec::new();
    if dataset.schema.version.trim().is_empty() {
        errors.push("label_schema.version is empty".to_string());
    }
    if !dataset.schema.test_set_frozen {
        errors.push("label_schema.test_set_frozen must be true".to_string());
    }

    let mut split_counts = BTreeMap::new();
    let mut label_counts = BTreeMap::new();
    let mut seen_ids = BTreeSet::new();
    let mut duplicate_ids = BTreeSet::new();
    let mut literal_false_trigger_negatives = 0usize;

    for sample in &dataset.samples {
        if !seen_ids.insert(sample.id.clone()) {
            duplicate_ids.insert(sample.id.clone());
        }
        *split_counts
            .entry(sample.split.as_str().to_string())
            .or_insert(0) += 1;
        *label_counts
            .entry(sample.label.as_str().to_string())
            .or_insert(0) += 1;

        if sample.dataset_version != dataset.schema.version {
            errors.push(format!(
                "{} dataset_version {} does not match schema {}",
                sample.id, sample.dataset_version, dataset.schema.version
            ));
        }
        if sample.current_text.trim().is_empty() {
            errors.push(format!("{} current_text is empty", sample.id));
        }
        if sample.previous_text.trim().is_empty() || sample.previous_segments.is_empty() {
            errors.push(format!("{} previous context is empty", sample.id));
        }
        if sample.label == IntentLabel::Literal && sample.current_text.contains("不是") {
            literal_false_trigger_negatives += 1;
        }
        validate_label_fields(sample, &mut errors);
    }

    for label in IntentLabel::all() {
        label_counts
            .entry(label.as_str().to_string())
            .or_insert(0usize);
    }
    for split in [DatasetSplit::Train, DatasetSplit::Dev, DatasetSplit::Test] {
        split_counts
            .entry(split.as_str().to_string())
            .or_insert(0usize);
    }

    let total_samples = dataset.samples.len();
    let test_samples = split_counts.get("test").copied().unwrap_or_default();
    let literal_false_trigger_share = if total_samples == 0 {
        0.0
    } else {
        literal_false_trigger_negatives as f64 / total_samples as f64
    };

    if total_samples < MIN_TOTAL_SAMPLES {
        errors.push(format!(
            "total samples {total_samples} is below {MIN_TOTAL_SAMPLES}"
        ));
    }
    for label in IntentLabel::all() {
        let count = label_counts
            .get(label.as_str())
            .copied()
            .unwrap_or_default();
        if count < MIN_PER_LABEL {
            errors.push(format!(
                "{} count {count} is below {MIN_PER_LABEL}",
                label.as_str()
            ));
        }
    }
    if test_samples < MIN_TEST_SAMPLES {
        errors.push(format!(
            "test samples {test_samples} is below {MIN_TEST_SAMPLES}"
        ));
    }
    if literal_false_trigger_share + f64::EPSILON < MIN_LITERAL_FALSE_TRIGGER_SHARE {
        errors.push(format!(
            "literal false-trigger share {:.4} is below {:.2}",
            literal_false_trigger_share, MIN_LITERAL_FALSE_TRIGGER_SHARE
        ));
    }
    if !duplicate_ids.is_empty() {
        errors.push(format!("duplicate ids: {:?}", duplicate_ids));
    }

    let duplicate_ids = duplicate_ids.into_iter().collect::<Vec<_>>();
    Ok(DatasetValidationReport {
        dataset_version: dataset.schema.version.clone(),
        total_samples,
        split_counts,
        label_counts,
        test_samples,
        literal_false_trigger_negatives,
        literal_false_trigger_share,
        duplicate_ids,
        passed: errors.is_empty(),
        errors,
    })
}

fn validate_label_fields(sample: &SemanticIntentSample, errors: &mut Vec<String>) {
    match sample.label {
        IntentLabel::UndoTarget => {
            if sample
                .target
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                errors.push(format!("{} undo_target requires target", sample.id));
            }
            if sample.replacement.is_some() {
                errors.push(format!(
                    "{} undo_target must not set replacement",
                    sample.id
                ));
            }
        }
        IntentLabel::ReplaceEntity => {
            if sample
                .target
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                errors.push(format!("{} replace_entity requires target", sample.id));
            }
            if sample
                .replacement
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                errors.push(format!("{} replace_entity requires replacement", sample.id));
            }
        }
        IntentLabel::RepairPrevious => {
            if sample
                .replacement
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                errors.push(format!(
                    "{} repair_previous requires replacement",
                    sample.id
                ));
            }
        }
        IntentLabel::Literal | IntentLabel::UndoLast | IntentLabel::Uncertain => {}
    }
}

fn label_schema() -> LabelSchema {
    LabelSchema {
        version: DATASET_VERSION.to_string(),
        generated_by: "voxflow-semantic generate".to_string(),
        generated_at: GENERATED_AT.to_string(),
        test_set_frozen: true,
        labels: vec![
            LabelDefinition {
                label: IntentLabel::Literal,
                description: "原样输入,包括带'不是'但不应触发删除的安全负例".to_string(),
                examples: vec!["这不是问题".to_string(), "不是要删除这句话".to_string()],
            },
            LabelDefinition {
                label: IntentLabel::UndoLast,
                description: "删除最近一个 VoxFlow 注入片段".to_string(),
                examples: vec!["刚才那句删掉".to_string(), "撤销上一句".to_string()],
            },
            LabelDefinition {
                label: IntentLabel::UndoTarget,
                description: "删除最近账本中明确提到的目标文本".to_string(),
                examples: vec!["把前面的三点删掉".to_string()],
            },
            LabelDefinition {
                label: IntentLabel::ReplaceEntity,
                description: "将上一片段里的实体 target 替换为 replacement".to_string(),
                examples: vec!["三点不对,四点".to_string()],
            },
            LabelDefinition {
                label: IntentLabel::RepairPrevious,
                description: "修正上一短语或上一片段,通常只给 replacement".to_string(),
                examples: vec!["不对,应该是四点".to_string()],
            },
            LabelDefinition {
                label: IntentLabel::Uncertain,
                description: "上下文不足或风险过高,不得直接删除".to_string(),
                examples: vec!["不是".to_string(), "刚才那个".to_string()],
            },
        ],
        fields: vec![
            field("id", true, "稳定样本 ID"),
            field("dataset_version", true, "数据集版本"),
            field("split", true, "train/dev/test"),
            field("locale", true, "样本 locale"),
            field("current_text", true, "当前 ASR stable/final 文本"),
            field("previous_text", true, "最近账本片段的扁平文本"),
            field("previous_segments", true, "最近 1-3 个账本片段"),
            field("language_hint", true, "zh/en/mixed"),
            field("pause_before_ms", true, "当前片段前停顿"),
            field("pause_after_ms", true, "当前片段后停顿"),
            field("label", true, "六类固定标签"),
            field("target", false, "undo_target/replace_entity 的目标提示"),
            field(
                "replacement",
                false,
                "replace_entity/repair_previous 的替换提示",
            ),
            field("source", true, "样本来源"),
            field("review_status", true, "人工复核状态"),
            field("tags", true, "覆盖维度标签"),
        ],
        thresholds: ValidationThresholds {
            min_total_samples: MIN_TOTAL_SAMPLES,
            min_per_label: MIN_PER_LABEL,
            min_test_samples: MIN_TEST_SAMPLES,
            min_literal_false_trigger_share_percent: 30,
        },
        notes: vec![
            "本版本为确定性 synthetic seed,全部 review_status=unreviewed".to_string(),
            "发布轻量 ONNX 分类器前必须完成人工复核并记录 release gate".to_string(),
        ],
    }
}

fn field(name: &str, required: bool, description: &str) -> FieldDefinition {
    FieldDefinition {
        name: name.to_string(),
        required,
        description: description.to_string(),
    }
}

#[derive(Debug, Clone)]
struct SeedCase {
    locale: String,
    current_text: String,
    previous_text: String,
    language_hint: String,
    target: Option<String>,
    replacement: Option<String>,
    tags: Vec<String>,
}

impl SeedCase {
    fn into_sample(
        self,
        label: IntentLabel,
        split: DatasetSplit,
        split_label_index: usize,
        label_index: usize,
    ) -> SemanticIntentSample {
        let previous_segment_id = format!("ctx-{}-{:04}", label.as_str(), label_index + 1);
        SemanticIntentSample {
            id: format!(
                "si-v0_1-{}-{}-{:04}",
                split.as_str(),
                label.as_str(),
                split_label_index
            ),
            dataset_version: DATASET_VERSION.to_string(),
            split,
            locale: self.locale,
            current_text: self.current_text,
            previous_text: self.previous_text.clone(),
            previous_segments: vec![PreviousSegment {
                id: previous_segment_id,
                text: self.previous_text,
                age_ms: 900 + ((label_index % 7) as u32 * 260),
            }],
            language_hint: self.language_hint,
            pause_before_ms: pause_before(label_index),
            pause_after_ms: pause_after(label_index),
            label,
            target: self.target,
            replacement: self.replacement,
            source: "synthetic_rule_seed".to_string(),
            review_status: ReviewStatus::Unreviewed,
            tags: self.tags,
        }
    }
}

fn generate_label_seeds(label: IntentLabel, count: usize) -> Vec<SeedCase> {
    (0..count)
        .map(|index| match label {
            IntentLabel::Literal => literal_seed(index),
            IntentLabel::UndoLast => undo_last_seed(index),
            IntentLabel::UndoTarget => undo_target_seed(index),
            IntentLabel::ReplaceEntity => replace_entity_seed(index),
            IntentLabel::RepairPrevious => repair_previous_seed(index),
            IntentLabel::Uncertain => uncertain_seed(index),
        })
        .collect()
}

fn literal_seed(index: usize) -> SeedCase {
    let topic = format!("第{}项", index % 47 + 1);
    let templates = [
        "这不是问题,继续记录{topic}",
        "我说的不是不对,是{topic}要保留",
        "不是要删除{topic},只是补充说明",
        "不是说要撤回{topic},先保留",
        "{topic}不是三点那个意思",
        "not a delete,不是要删{topic}",
        "这不是错了,{topic}保持不变",
        "不是刚才那句删掉,{topic}还要留着",
    ];
    let current_text = fill_topic(templates[index % templates.len()], &topic);
    SeedCase {
        locale: locale_for(index),
        current_text,
        previous_text: context_text(index),
        language_hint: language_hint_for(index),
        target: None,
        replacement: None,
        tags: vec![
            "false_trigger_negative".to_string(),
            "contains_不是".to_string(),
        ],
    }
}

fn undo_last_seed(index: usize) -> SeedCase {
    let templates = [
        "刚才那句删掉",
        "删掉刚才那句",
        "撤销上一句",
        "删除上一句",
        "把上一句撤回",
        "上一段不要了",
        "delete the previous sentence",
        "remove that last part",
    ];
    SeedCase {
        locale: locale_for(index),
        current_text: templates[index % templates.len()].to_string(),
        previous_text: context_text(index),
        language_hint: language_hint_for(index),
        target: None,
        replacement: None,
        tags: vec!["deictic_undo".to_string()],
    }
}

fn undo_target_seed(index: usize) -> SeedCase {
    let entity = entity_case(index);
    let templates = [
        "把前面的{target}删掉",
        "删掉{target}",
        "撤销前面那个{target}",
        "{target}不要了",
        "remove {target} from before",
        "delete the earlier {target}",
    ];
    SeedCase {
        locale: locale_for(index),
        current_text: fill_target(templates[index % templates.len()], &entity.target),
        previous_text: entity.context,
        language_hint: language_hint_for(index),
        target: Some(entity.target),
        replacement: None,
        tags: vec!["targeted_undo".to_string(), entity.kind],
    }
}

fn replace_entity_seed(index: usize) -> SeedCase {
    let entity = entity_case(index);
    let templates = [
        "{target}不对,{replacement}",
        "{target}不对改成{replacement}",
        "{target}错了,应该是{replacement}",
        "把{target}换成{replacement}",
        "change {target} to {replacement}",
        "{target} should be {replacement}",
    ];
    SeedCase {
        locale: locale_for(index),
        current_text: fill_target_replacement(
            templates[index % templates.len()],
            &entity.target,
            &entity.replacement,
        ),
        previous_text: entity.context,
        language_hint: language_hint_for(index),
        target: Some(entity.target),
        replacement: Some(entity.replacement),
        tags: vec!["entity_replace".to_string(), entity.kind],
    }
}

fn repair_previous_seed(index: usize) -> SeedCase {
    let entity = entity_case(index);
    let templates = [
        "不对,应该是{replacement}",
        "不对,{replacement}",
        "错了改成{replacement}",
        "错了,应该是{replacement}",
        "no, make it {replacement}",
        "sorry, it should be {replacement}",
    ];
    SeedCase {
        locale: locale_for(index),
        current_text: fill_replacement(templates[index % templates.len()], &entity.replacement),
        previous_text: entity.context,
        language_hint: language_hint_for(index),
        target: None,
        replacement: Some(entity.replacement),
        tags: vec!["repair_previous".to_string(), entity.kind],
    }
}

fn uncertain_seed(index: usize) -> SeedCase {
    let templates = [
        "不是",
        "不对吧",
        "刚才那个",
        "算了",
        "可能不是",
        "这个先别动",
        "maybe not",
        "not sure about that",
        "不是不对",
        "等等前面那个",
    ];
    SeedCase {
        locale: locale_for(index),
        current_text: templates[index % templates.len()].to_string(),
        previous_text: context_text(index),
        language_hint: language_hint_for(index),
        target: None,
        replacement: None,
        tags: vec!["ambiguous".to_string(), "safe_downgrade".to_string()],
    }
}

#[derive(Debug, Clone)]
struct EntityCase {
    target: String,
    replacement: String,
    context: String,
    kind: String,
}

fn entity_case(index: usize) -> EntityCase {
    let entities = [
        ("三点", "四点", "time"),
        ("周三", "周四", "date"),
        ("小王", "小李", "person"),
        ("预算表", "报价单", "document"),
        ("A区", "B区", "location"),
        ("version one", "version two", "english"),
        ("alpha", "beta", "english"),
        ("meeting", "review", "english"),
        ("蓝牙", "麦克风", "device"),
        ("百分之二十", "百分之三十", "number"),
    ];
    let contexts = [
        "今天下午{target}开会",
        "把提醒设到{target}",
        "请{target}确认一下",
        "标题先写成{target}",
        "we need {target} today",
        "把{target}加到备注",
        "下一步检查{target}",
        "{target}这一项先放在前面",
    ];
    let (target, replacement, kind) = entities[index % entities.len()];
    let context = fill_target(contexts[(index / entities.len()) % contexts.len()], target);
    EntityCase {
        target: target.to_string(),
        replacement: replacement.to_string(),
        context,
        kind: kind.to_string(),
    }
}

fn context_text(index: usize) -> String {
    let contexts = [
        "今天下午三点开会",
        "把预算表发给小王",
        "明天上午同步项目进度",
        "请客户确认报价单",
        "周五之前完成测试记录",
        "we need version one today",
        "把蓝牙设备切到麦克风模式",
        "下周一复盘发布问题",
    ];
    contexts[index % contexts.len()].to_string()
}

fn locale_for(index: usize) -> String {
    match index % 6 {
        0..=2 => "zh-CN",
        3 => "en-US",
        _ => "mixed",
    }
    .to_string()
}

fn language_hint_for(index: usize) -> String {
    match index % 6 {
        0..=2 => "zh",
        3 => "en",
        _ => "mixed",
    }
    .to_string()
}

fn pause_before(index: usize) -> u32 {
    [120, 240, 320, 480, 760][index % 5]
}

fn pause_after(index: usize) -> u32 {
    [180, 280, 420, 560, 900][index % 5]
}

fn fill_topic(template: &str, topic: &str) -> String {
    template.replace("{topic}", topic)
}

fn fill_target(template: &str, target: &str) -> String {
    template.replace("{target}", target)
}

fn fill_replacement(template: &str, replacement: &str) -> String {
    template.replace("{replacement}", replacement)
}

fn fill_target_replacement(template: &str, target: &str, replacement: &str) -> String {
    template
        .replace("{target}", target)
        .replace("{replacement}", replacement)
}

fn write_pretty_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    serde_json::to_writer_pretty(BufWriter::new(file), value)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn write_jsonl<T: Serialize>(path: &Path, values: &[T]) -> Result<()> {
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for value in values {
        let line = serde_json::to_string(value)?;
        writeln!(writer, "{line}").with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}

fn read_jsonl(path: &Path, expected_split: DatasetSplit) -> Result<Vec<SemanticIntentSample>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut samples = Vec::new();
    for (line_index, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("read {}", path.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        let sample: SemanticIntentSample = serde_json::from_str(&line)
            .with_context(|| format!("parse {} line {}", path.display(), line_index + 1))?;
        if sample.split != expected_split {
            bail!(
                "{} line {} has split {:?}, expected {:?}",
                path.display(),
                line_index + 1,
                sample.split,
                expected_split
            );
        }
        samples.push(sample);
    }
    Ok(samples)
}
