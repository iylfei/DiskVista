use anyhow::{ensure, Result};
use rusqlite::types::Value;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AnalysisFilter {
    analyzed: BTreeSet<i64>,
    include_analyzed: bool,
}

impl AnalysisFilter {
    pub fn new(status: &str, analyzed: BTreeSet<i64>) -> Result<Option<Self>> {
        let include_analyzed = match status {
            "" => return Ok(None),
            "analyzed" => true,
            "unanalyzed" => false,
            _ => anyhow::bail!("未知的 AI 分析筛选条件"),
        };
        Ok(Some(Self {
            analyzed,
            include_analyzed,
        }))
    }

    pub fn validate(status: &str, filter: Option<&Self>) -> Result<()> {
        ensure!(
            match (status, filter) {
                ("", None) => true,
                ("analyzed", Some(filter)) => filter.include_analyzed,
                ("unanalyzed", Some(filter)) => !filter.include_analyzed,
                _ => false,
            },
            "AI 分析筛选需要当前有效结果"
        );
        Ok(())
    }

    pub fn matches(&self, id: i64) -> bool {
        self.analyzed.contains(&id) == self.include_analyzed
    }

    pub(crate) fn append_sql(&self, conditions: &mut String, args: &mut Vec<Value>) -> Result<()> {
        conditions.push_str(if self.include_analyzed {
            " AND id IN (SELECT value FROM json_each(?))"
        } else {
            " AND id NOT IN (SELECT value FROM json_each(?))"
        });
        args.push(serde_json::to_string(&self.analyzed)?.into());
        Ok(())
    }
}
