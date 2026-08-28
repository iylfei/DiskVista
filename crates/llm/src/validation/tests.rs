use super::*;
use crate::client::synthetic_context;
use cleaner_domain::HistoryReference;

fn contexts() -> Vec<AnalysisContext> {
    (1..=3)
        .map(|id| AnalysisContext {
            entry_id: id,
            ..synthetic_context()
        })
        .collect()
}

fn item(id: i64) -> Value {
    json!({"entryId":id,"recommendation":"review","reason":"用途尚不明确，先确认是否仍需使用。","historyMatchIds":[]})
}

fn history(id: &str, basis: &str) -> HistoryReference {
    HistoryReference {
        id: id.into(),
        path: "%USERPROFILE%/Example/old.bin".into(),
        bytes: 100,
        recycled_at: 1,
        owner: None,
        category: None,
        match_basis: vec![basis.into()],
    }
}

#[test]
fn reordered_items_are_mapped_by_id_and_missing_items_fail_individually() {
    let result =
        validate_batch(&json!({"items":[item(3),item(1)]}).to_string(), &contexts()).unwrap();
    assert_eq!(
        result.iter().map(|item| item.entry_id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(result[0].assessment.is_ok());
    assert!(result[1]
        .assessment
        .as_ref()
        .unwrap_err()
        .contains("未返回"));
    assert!(result[2].assessment.is_ok());
}

#[test]
fn unknown_and_duplicate_ids_never_replace_another_result() {
    let result = validate_batch(
        &json!({"items":[item(999),item(2),item(1),item(2),item(2)]}).to_string(),
        &contexts(),
    )
    .unwrap();
    assert!(result[0].assessment.is_ok());
    assert!(result[1].assessment.as_ref().unwrap_err().contains("重复"));
    assert!(result[2]
        .assessment
        .as_ref()
        .unwrap_err()
        .contains("未返回"));
    assert_eq!(result.len(), 3);
}

#[test]
fn invalid_identity_types_and_duplicate_json_keys_cannot_reassign_items() {
    let text = r#"{"items":[{"entryId":1,"entryId":2,"recommendation":"review","reason":"不可串项","historyMatchIds":[]},{"entryId":"3","recommendation":"keep","reason":"编号类型错误","historyMatchIds":[]}]}"#;
    let result = validate_batch(text, &contexts()).unwrap();
    assert!(result.iter().all(|item| item.assessment.is_err()));
    let text = r#"{"items":[{"entryId":1,"recommendation":"review","reason":"一个理由","reason":"重复字段","historyMatchIds":[]}]}"#;
    assert!(validate_batch(text, &contexts()).unwrap()[0]
        .assessment
        .is_err());
}

#[test]
fn invalid_recommendation_reason_and_extra_fields_only_fail_their_item() {
    for (field, value) in [
        ("recommendation", json!("delete_now")),
        ("reason", json!("")),
        ("reason", json!("\n不能换行")),
        ("reason", json!("字".repeat(121))),
        ("reason", json!(42)),
        ("command", json!("remove everything")),
        ("historyMatchIds", json!(null)),
    ] {
        let mut bad = item(2);
        bad[field] = value;
        let result = validate_batch(
            &json!({"items":[item(1),bad,item(3)]}).to_string(),
            &contexts(),
        )
        .unwrap();
        assert!(result[0].assessment.is_ok(), "{field}");
        assert!(result[1].assessment.is_err(), "{field}");
        assert!(result[2].assessment.is_ok(), "{field}");
    }
}

#[test]
fn compact_output_maps_to_readable_persistence_without_inventing_long_analysis() {
    let mut value = item(1);
    value["recommendation"] = json!("consider_delete");
    value["reason"] = json!("字".repeat(120));
    let result = validate_batch(&json!({"items":[value]}).to_string(), &contexts()).unwrap();
    let assessment = result[0].assessment.as_ref().unwrap();
    assert_eq!(assessment.deletion_advice, DeletionAdvice::ConsiderDelete);
    assert_eq!(assessment.reason.chars().count(), 120);
    assert!(assessment.history_matches.is_empty() && assessment.evidence.is_empty());
}

#[test]
fn history_is_validated_against_each_item_and_keeps_local_match_basis() {
    let mut inputs = contexts();
    inputs[0].history_references = vec![history("history:a", "相同应用目录")];
    inputs[1].history_references = vec![history("history:b", "本地文件名线索")];
    let mut first = item(1);
    first["historyMatchIds"] = json!(["history:a"]);
    let mut second = item(2);
    second["historyMatchIds"] = json!(["history:a"]);
    let result = validate_batch(
        &json!({"items":[first,second,item(3)]}).to_string(),
        &inputs,
    )
    .unwrap();
    let first = result[0].assessment.as_ref().unwrap();
    assert_eq!(first.evidence, vec!["history:a"]);
    assert_eq!(first.history_matches[0].reason, "相同应用目录");
    assert!(result[1].assessment.is_err());
    assert!(result[2].assessment.is_ok());

    for ids in [
        json!(["history:invented"]),
        json!(["history:a", "history:a"]),
        json!(["summary"]),
        json!(vec!["history:a"; 7]),
    ] {
        let mut bad = item(1);
        bad["historyMatchIds"] = ids;
        assert!(
            validate_batch(&json!({"items":[bad]}).to_string(), &inputs).unwrap()[0]
                .assessment
                .is_err()
        );
    }
}

#[test]
fn malformed_or_excessive_envelopes_fail_globally_without_salvaging_prefixes() {
    for text in [
        "not JSON".to_owned(),
        "{\"items\":[".to_owned(),
        "{\"items\":[],\"items\":[]}".to_owned(),
        json!({"items":[],"command":"ignored"}).to_string(),
        json!({"items":vec![Value::Null; 21]}).to_string(),
        json!({"items":null}).to_string(),
        " ".repeat(MAX_OUTPUT_BYTES + 1),
    ] {
        assert!(validate_batch(&text, &contexts()).is_err());
    }
    assert!(validate_batch("{\"items\":[]}", &contexts())
        .unwrap()
        .iter()
        .all(|item| item.assessment.is_err()));
}
