use crate::{batch::BatchItemReply, batch_context::MAX_BATCH_ITEMS};
use anyhow::{anyhow, bail, Result};
use cleaner_domain::{AnalysisContext, DeletionAdvice, HistoryMatch, ModelAssessment};
use serde::Deserialize;
use serde_json::{json, value::RawValue, Value};
use std::collections::{HashMap, HashSet};

pub const MAX_REASON_CHARACTERS: usize = 120;
const MAX_OUTPUT_BYTES: usize = 65_536;

pub fn schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "items": {
                "type": "array",
                "maxItems": MAX_BATCH_ITEMS,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "entryId": {"type": "integer"},
                        "recommendation": {"type": "string", "enum": ["consider_delete", "keep", "review"]},
                        "reason": {"type": "string", "minLength": 1, "maxLength": MAX_REASON_CHARACTERS},
                        "historyMatchIds": {"type": "array", "maxItems": 6, "items": {"type": "string"}}
                    },
                    "required": ["entryId", "recommendation", "reason", "historyMatchIds"]
                }
            }
        },
        "required": ["items"]
    })
}

pub(crate) fn schema_for(contexts: &[AnalysisContext]) -> Value {
    let mut value = schema();
    value["properties"]["items"]["maxItems"] = json!(contexts.len());
    value["properties"]["items"]["items"]["properties"]["entryId"]["enum"] = json!(contexts
        .iter()
        .map(|context| context.entry_id)
        .collect::<Vec<_>>());
    value
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchEnvelope {
    items: Vec<Box<RawValue>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryIdentity {
    entry_id: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CompactAssessment {
    entry_id: i64,
    recommendation: DeletionAdvice,
    reason: String,
    history_match_ids: Vec<String>,
}

pub fn validate_batch(text: &str, contexts: &[AnalysisContext]) -> Result<Vec<BatchItemReply>> {
    if text.len() > MAX_OUTPUT_BYTES {
        bail!("AI 输出超过长度限制");
    }
    let text = text.trim();
    let text = text
        .strip_prefix("```json")
        .and_then(|text| text.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(text);
    let envelope: BatchEnvelope =
        serde_json::from_str(text).map_err(|_| anyhow!("AI 未返回约定的批量 JSON 结构"))?;
    if envelope.items.len() > MAX_BATCH_ITEMS {
        bail!("AI 返回的项目数量超过限制");
    }
    let expected: HashMap<_, _> = contexts
        .iter()
        .map(|context| (context.entry_id, context))
        .collect();
    if expected.len() != contexts.len() {
        bail!("分析输入包含重复的文件编号");
    }
    let mut results = HashMap::new();
    for item in envelope.items {
        let Ok(identity) = serde_json::from_str::<EntryIdentity>(item.get()) else {
            continue;
        };
        let Some(context) = expected.get(&identity.entry_id) else {
            continue;
        };
        if let std::collections::hash_map::Entry::Occupied(mut entry) =
            results.entry(identity.entry_id)
        {
            *entry.get_mut() = Err("AI 重复返回该文件，结果已拒绝".into());
            continue;
        }
        let assessment = serde_json::from_str::<CompactAssessment>(item.get())
            .map_err(|_| "AI 未返回约定的删除建议结构".to_owned())
            .and_then(|assessment| validate_item(assessment, context));
        results.insert(identity.entry_id, assessment);
    }
    Ok(contexts
        .iter()
        .map(|context| BatchItemReply {
            entry_id: context.entry_id,
            assessment: results
                .remove(&context.entry_id)
                .unwrap_or_else(|| Err("AI 未返回该文件的分析结果".into())),
        })
        .collect())
}

fn validate_item(
    item: CompactAssessment,
    context: &AnalysisContext,
) -> std::result::Result<ModelAssessment, String> {
    if item.entry_id != context.entry_id {
        return Err("AI 文件编号与分析输入不一致".into());
    }
    if item.reason.trim().is_empty()
        || item.reason.chars().count() > MAX_REASON_CHARACTERS
        || item.reason.chars().any(char::is_control)
    {
        return Err("AI 简短理由缺失、超过120字或包含换行".into());
    }
    let allowed: HashMap<_, _> = context
        .history_references
        .iter()
        .map(|reference| (reference.id.as_str(), reference))
        .collect();
    let mut seen = HashSet::new();
    if item.history_match_ids.len() > 6
        || item.history_match_ids.iter().any(|id| {
            !id.starts_with("history:") || !allowed.contains_key(id.as_str()) || !seen.insert(id)
        })
    {
        return Err("AI 引用了该文件未提供的历史记录、重复记录或超过限制".into());
    }
    let reason = item.reason.trim().to_owned();
    let history_matches = item
        .history_match_ids
        .iter()
        .map(|id| HistoryMatch {
            history_id: id.clone(),
            reason: allowed[id.as_str()]
                .match_basis
                .join("；")
                .chars()
                .take(600)
                .collect(),
        })
        .collect();
    Ok(ModelAssessment {
        deletion_advice: item.recommendation,
        reason,
        evidence: item.history_match_ids,
        history_matches,
    })
}

#[cfg(test)]
mod tests;
