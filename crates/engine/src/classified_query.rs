mod cache;
mod classifier;
mod scope;

pub use cache::{ClassificationChanged, QueryCache};
pub use classifier::Classifier;

use cleaner_domain::EntryQuery;

pub fn needs_classification(query: &EntryQuery) -> bool {
    query.risk.as_ref().is_some_and(|risk| !risk.is_empty())
        || query.owner.is_some()
        || query.category.is_some()
        || query.uncertain_only
        || query.suggestions
        || !query.analysis_status.is_empty()
}

#[cfg(test)]
mod tests;
