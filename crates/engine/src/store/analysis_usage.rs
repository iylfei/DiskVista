use super::*;
use std::collections::HashMap;

impl Store {
    pub(super) fn discard_legacy_analyses(&self) -> Result<()> {
        self.connection()?.execute(
            "DELETE FROM analyses WHERE CASE WHEN json_valid(data)
             THEN COALESCE(json_extract(data,'$.formatVersion'),0) ELSE 0 END < ?",
            [ANALYSIS_FORMAT_VERSION],
        )?;
        Ok(())
    }

    pub fn hydrate_batch_usage(&self, results: &mut [AnalysisResult]) -> Result<()> {
        let connection = self.connection()?;
        let mut statement = connection.prepare_cached(
            "SELECT data FROM analyses WHERE scan_id=?1
             AND json_extract(data,'$.requestId')=?2
             AND (json_extract(data,'$.promptTokens') IS NOT NULL
                  OR json_extract(data,'$.completionTokens') IS NOT NULL)
             ORDER BY created DESC,id DESC LIMIT 1",
        )?;
        let mut usage = HashMap::new();
        for result in results {
            let Some(request_id) = result.request_id.as_ref() else {
                continue;
            };
            if result.request_item_count <= 1
                || result.prompt_tokens.is_some()
                || result.completion_tokens.is_some()
            {
                continue;
            }
            let key = (result.scan_id.clone(), request_id.clone());
            if let std::collections::hash_map::Entry::Vacant(entry) = usage.entry(key.clone()) {
                let json: Option<String> = statement
                    .query_row(params![&key.0, &key.1], |row| row.get(0))
                    .optional()?;
                let totals = json
                    .map(|json| serde_json::from_str::<AnalysisResult>(&json))
                    .transpose()?
                    .map(|row| (row.prompt_tokens, row.completion_tokens));
                entry.insert(totals);
            }
            if let Some(Some((prompt, completion))) = usage.get(&key) {
                result.prompt_tokens = *prompt;
                result.completion_tokens = *completion;
            }
        }
        Ok(())
    }
}
