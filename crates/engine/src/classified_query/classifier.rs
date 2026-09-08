use crate::{application_index::ApplicationIndex, rules::RuleSet, safety::SafetyPolicy};
use anyhow::Result;
use cleaner_domain::{EntryQuery, FileRecord, Scan};
use cleaner_platform::normalize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub struct Classifier<'a> {
    pub(super) scan: &'a Scan,
    rules: &'a RuleSet,
    policy: &'a SafetyPolicy,
    apps: &'a ApplicationIndex,
    root: String,
    protected_ancestors: HashSet<String>,
}

impl<'a> Classifier<'a> {
    pub fn new(
        scan: &'a Scan,
        rules: &'a RuleSet,
        policy: &'a SafetyPolicy,
        apps: &'a ApplicationIndex,
    ) -> Self {
        let mut protected_ancestors = HashSet::new();
        let roots = policy
            .settings
            .protected_paths
            .iter()
            .chain(&policy.settings.ignored_paths)
            .map(String::as_str)
            .chain(apps.installation_roots());
        for root in roots {
            let root = normalize(root);
            for (end, _) in root.match_indices('\\') {
                protected_ancestors.insert(root[..end].to_owned());
            }
        }
        Self {
            scan,
            rules,
            policy,
            apps,
            root: normalize(&scan.root),
            protected_ancestors,
        }
    }

    pub fn apply(&self, file: &mut FileRecord) {
        file.assessment = self.rules.classify_indexed(file, self.policy, self.apps);
        let path = normalize(&file.path);
        let reason = if path == self.root {
            Some("扫描根目录不能整体清理")
        } else if !file.complete || file.has_blocked_children {
            Some("目标不完整或包含受保护后代，不可整体回收")
        } else if file.is_dir && self.protected_ancestors.contains(&path) {
            Some("此目录包含当前受保护的路径，不可整体回收")
        } else {
            None
        };
        if let Some(reason) = reason {
            file.assessment.risk = "protected".into();
            if file.assessment.protected_reason.is_none() {
                file.assessment.protected_reason = Some(reason.into());
                file.assessment.recommendation = reason.into();
            }
        }
    }

    pub(super) fn signature(&self, origin_key: &str) -> Result<String> {
        let roots: Vec<_> = self
            .rules
            .directory_roots()
            .map(|(rule, root, _)| (&rule.id, root))
            .collect();
        Ok(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                "classified-query-v1",
                origin_key,
                self.scan,
                &self.rules.version,
                &self.rules.rules,
                roots,
                &self.policy.settings.protected_paths,
                &self.policy.settings.unprotected_paths,
                &self.policy.settings.ignored_paths,
                &self.policy.settings.labels,
            ))?)
        ))
    }

    /// Earliest future age threshold that can invalidate this file's classification.
    pub fn next_change(&self, file: &FileRecord, started: i64) -> Option<i64> {
        self.rules.next_change(file, started)
    }

    pub(super) fn cacheable(&self) -> bool {
        matches!(
            self.scan.status.as_str(),
            "complete" | "cancelled" | "canceled" | "interrupted" | "failed"
        )
    }
}

pub(super) fn matches(file: &FileRecord, query: &EntryQuery) -> bool {
    let assessment = &file.assessment;
    let recognized = assessment.owner.is_some() || assessment.rule_id.is_some();
    if query.uncertain_only
        && (assessment.risk != "review"
            || assessment.rule_id.is_some()
            || assessment.confidence != "low")
    {
        return false;
    }
    if let Some(risk) = query.risk.as_deref().filter(|risk| !risk.is_empty()) {
        let matched = match risk {
            "unknown" => assessment.risk == "review" && !recognized,
            "known" => recognized,
            "known_review" => assessment.risk == "review" && recognized,
            _ => assessment.risk == risk,
        };
        if !matched {
            return false;
        }
    }
    if query
        .category
        .as_ref()
        .is_some_and(|category| category != &assessment.category)
        || query
            .owner
            .as_deref()
            .is_some_and(|owner| owner != assessment.owner.as_deref().unwrap_or("未知"))
    {
        return false;
    }
    if query.suggestions {
        if assessment.rule_id.is_none() && file.logical_bytes < 104_857_600 {
            return false;
        }
        if query.risk.as_deref() != Some("protected")
            && matches!(assessment.risk.as_str(), "protected" | "keep")
        {
            return false;
        }
    }
    true
}
