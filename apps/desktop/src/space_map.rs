use crate::state::AppState;
use cleaner_domain::EntryQuery;

pub(crate) fn read(
    state: &AppState,
    scan_id: &str,
    parent: &str,
) -> Result<serde_json::Value, String> {
    state.with_classification(scan_id, |classifier, _| {
        let mut root = state.store.by_path(scan_id, parent)?;
        let mut children = state.classified_queries.query_unclassified(
            &state.store,
            &EntryQuery {
                scan_id: scan_id.into(),
                parent: Some(root.path.clone()),
                limit: 24,
                ..Default::default()
            },
        )?;
        classifier.apply(&mut root);
        for file in &mut children.items {
            classifier.apply(file);
        }
        Ok(serde_json::json!({"parent":root,"items":children.items,"total":children.total}))
    })
}

#[cfg(test)]
mod tests;
