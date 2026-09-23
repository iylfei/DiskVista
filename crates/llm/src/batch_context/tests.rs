use super::*;
use crate::client::synthetic_context;
use serde_json::json;

fn context(id: i64, path: &str) -> AnalysisContext {
    AnalysisContext {
        entry_id: id,
        path: path.into(),
        ..synthetic_context()
    }
}

fn history() -> HistoryReference {
    HistoryReference {
        id: "history:a".into(),
        path: "%USERPROFILE%/Example/old.zip".into(),
        bytes: 100,
        recycled_at: 1,
        owner: None,
        category: None,
        match_basis: vec!["名称线索".into()],
    }
}

#[test]
fn manually_selected_history_can_exceed_the_default_six_references() {
    let mut input = context(1, "D:/Example/file.bin");
    input.history_references = (0..10)
        .map(|index| HistoryReference {
            id: format!("history:{index}"),
            ..history()
        })
        .collect();
    let value: Value = serde_json::from_str(&payload(&[input.clone()], None).unwrap()).unwrap();
    assert_eq!(
        value["metadata"]["historyReferences"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
    input.history_references = (0..101)
        .map(|index| HistoryReference {
            id: format!("history:{index}"),
            ..history()
        })
        .collect();
    assert!(validate_batch_contexts(&[input]).is_err());
}

#[test]
fn shared_paths_notes_and_history_preserve_each_items_own_authorization_and_basis() {
    let mut first = context(1, "%USERPROFILE%/Example/a.bin");
    first.history_references = vec![history()];
    first.note = "共同说明".repeat(300);
    let mut second = context(2, "%USERPROFILE%/Example/b.bin");
    second.history_references = vec![HistoryReference {
        match_basis: vec!["同一来源线索".into()],
        ..history()
    }];
    second.note = first.note.clone();
    let mut third = context(3, "D:/Different/c.bin");
    third.note = first.note.clone();
    let contexts = [first, second, third];
    let encoded = payload(&contexts, None).unwrap();
    let value: Value = serde_json::from_str(&encoded).unwrap();
    let metadata = &value["metadata"];
    assert_eq!(metadata["directories"].as_array().unwrap().len(), 2);
    assert_eq!(metadata["notes"].as_array().unwrap().len(), 1);
    assert_eq!(metadata["historyReferences"].as_array().unwrap().len(), 1);
    assert!(metadata["historyReferences"][0].get("matchBasis").is_none());
    assert_eq!(
        metadata["items"][0]["historyReferences"][0]["matchBasis"],
        json!(["名称线索"])
    );
    assert_eq!(
        metadata["items"][1]["historyReferences"][0]["matchBasis"],
        json!(["同一来源线索"])
    );
    assert_eq!(metadata["items"][2]["historyReferences"], json!([]));
    assert!(value.get("authorizedTextSamples").is_none());
    assert_eq!(
        batch_metadata_bytes(&contexts).unwrap(),
        metadata.to_string().len()
    );
    assert!(metadata.to_string().len() < serde_json::to_vec(&contexts).unwrap().len());
}

#[test]
fn parent_path_compression_preserves_windows_roots_and_target_names() {
    let contexts = [context(1, "C:\\"), context(2, "C:\\Data\\file.bin")];
    let value: Value = serde_json::from_str(&payload(&contexts, None).unwrap()).unwrap();
    for (index, context) in contexts.iter().enumerate() {
        let item = &value["metadata"]["items"][index];
        let parent =
            &value["metadata"]["directories"][item["directoryId"].as_u64().unwrap() as usize];
        assert_eq!(
            format!(
                "{}{}",
                parent.as_str().unwrap(),
                item["name"].as_str().unwrap()
            ),
            context.path
        );
    }
}

#[test]
fn invalid_batch_shapes_and_conflicting_history_snapshots_are_rejected() {
    assert!(validate_batch_contexts(&[]).is_err());
    let contexts: Vec<_> = (0..21)
        .map(|id| context(id, "D:/Example/file.bin"))
        .collect();
    assert!(validate_batch_contexts(&contexts).is_err());
    assert!(validate_batch_contexts(&contexts[..20]).is_ok());
    let first = context(1, "D:/Example/a.bin");
    assert!(validate_batch_contexts(&[first.clone(), first.clone()]).is_err());
    let mut other_scan = context(2, "D:/Example/b.bin");
    other_scan.scan_id = "different-scan".into();
    assert!(validate_batch_contexts(&[first, other_scan]).is_err());
    let mut first = context(1, "D:/Example/a.bin");
    first.history_references = vec![history()];
    let mut second = context(2, "D:/Example/b.bin");
    second.history_references = vec![HistoryReference {
        bytes: 200,
        ..history()
    }];
    assert!(validate_batch_contexts(&[first, second])
        .unwrap_err()
        .to_string()
        .contains("快照不一致"));
    let mut duplicate = context(1, "D:/Example/a.bin");
    duplicate.history_references = vec![history(), history()];
    assert!(validate_batch_contexts(&[duplicate]).is_err());
}

#[test]
fn metadata_limit_uses_serialized_utf8_bytes_and_can_be_preflighted() {
    let mut context = context(1, "D:/Example/a.bin");
    context.note = "界".repeat(MAX_BATCH_METADATA_BYTES / 3 + 1);
    let contexts = [context];
    assert!(batch_metadata_bytes(&contexts).unwrap() > MAX_BATCH_METADATA_BYTES);
    assert!(validate_batch_contexts(&contexts)
        .unwrap_err()
        .to_string()
        .contains("64 KiB"));
    assert!(payload(&contexts, None).is_err());
}

#[test]
fn text_samples_only_appear_in_explicit_single_item_payloads() {
    let samples = json!([{"entryId":1,"text":"合成授权样本"}]);
    let contexts = [
        context(1, "D:/Example/a.log"),
        context(2, "D:/Example/b.log"),
    ];
    assert!(payload(&contexts, Some(&samples)).is_err());
    let value: Value =
        serde_json::from_str(&payload(&contexts[..1], Some(&samples)).unwrap()).unwrap();
    assert_eq!(value["authorizedTextSamples"], samples);
    let value: Value = serde_json::from_str(&payload(&contexts[..1], None).unwrap()).unwrap();
    assert!(value.get("authorizedTextSamples").is_none());
}
