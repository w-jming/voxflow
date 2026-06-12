use std::collections::BTreeMap;

use voxflow_semantic::{
    generate_dataset, validate_dataset, DatasetSplit, IntentLabel, MIN_LITERAL_FALSE_TRIGGER_SHARE,
    MIN_PER_LABEL, MIN_TEST_SAMPLES, MIN_TOTAL_SAMPLES,
};

#[test]
fn generated_dataset_meets_p0_thresholds() {
    let dataset = generate_dataset();
    let report = validate_dataset(&dataset).unwrap();

    assert!(report.passed, "{:?}", report.errors);
    assert!(report.total_samples >= MIN_TOTAL_SAMPLES);
    assert!(report.test_samples >= MIN_TEST_SAMPLES);
    assert!(report.literal_false_trigger_share >= MIN_LITERAL_FALSE_TRIGGER_SHARE);
    for label in IntentLabel::all() {
        let count = report
            .label_counts
            .get(label.as_str())
            .copied()
            .unwrap_or_default();
        assert!(count >= MIN_PER_LABEL, "{} -> {}", label.as_str(), count);
    }
}

#[test]
fn generated_dataset_is_stratified_by_label_and_split() {
    let dataset = generate_dataset();
    let mut counts = BTreeMap::new();
    for sample in dataset.samples {
        *counts.entry((sample.split, sample.label)).or_insert(0usize) += 1;
    }

    assert_eq!(counts[&(DatasetSplit::Train, IntentLabel::Literal)], 360);
    assert_eq!(counts[&(DatasetSplit::Dev, IntentLabel::Literal)], 120);
    assert_eq!(counts[&(DatasetSplit::Test, IntentLabel::Literal)], 120);
    for label in [
        IntentLabel::UndoLast,
        IntentLabel::UndoTarget,
        IntentLabel::ReplaceEntity,
        IntentLabel::RepairPrevious,
        IntentLabel::Uncertain,
    ] {
        assert_eq!(counts[&(DatasetSplit::Train, label)], 144);
        assert_eq!(counts[&(DatasetSplit::Dev, label)], 48);
        assert_eq!(counts[&(DatasetSplit::Test, label)], 48);
    }
}
