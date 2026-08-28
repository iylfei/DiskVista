use super::{boundary, decode, node, parent, Boundary, Node, NODE};
use crate::{
    application_index::ApplicationIndex,
    application_origins::weaker_confidence,
    rules::{Rule, RuleSet},
};
use anyhow::Result;
use cleaner_platform::within;
use rusqlite::{params, Connection};
use std::collections::BTreeMap;

fn role(rule: &Rule) -> &'static str {
    match rule.category.as_str() {
        "cache" => "cache",
        "temporary" => "temporary",
        "diagnostic" => "diagnostic",
        _ => "application_data",
    }
}

pub(super) fn add(
    c: &Connection,
    scan: &str,
    root: &Node,
    rules: &RuleSet,
    index: &ApplicationIndex,
    boundaries: &mut BTreeMap<String, Boundary>,
) -> Result<()> {
    let mut candidates = BTreeMap::<String, Node>::new();
    if rules.matching_rule(&root.key).is_some() {
        candidates.insert(root.key.clone(), root.clone());
    }
    for (_, path, matcher) in rules.directory_roots() {
        let Some(wildcard) = path.find(['*', '?', '[', '{']) else {
            if within(path, &root.key) {
                if let Some(n) = node(c, scan, path)? {
                    candidates.insert(n.key.clone(), n);
                }
            }
            continue;
        };
        let prefix = parent(&path[..wildcard]).unwrap_or("");
        let scope = if within(&root.key, prefix) {
            root.key.as_str()
        } else if within(prefix, &root.key) {
            prefix
        } else {
            continue;
        };
        let mut statement = c.prepare(&format!(
            "SELECT {NODE} FROM entries WHERE scan_id=?1 AND is_dir=1
             AND (path_key=?2 OR (path_key>=?3 AND path_key<?4)) ORDER BY path_key"
        ))?;
        for row in statement.query_map(
            params![scan, scope, format!("{scope}\\"), format!("{scope}]")],
            decode,
        )? {
            let n = row?;
            if matcher.is_match(n.key.replace('\\', "/"))
                && !parent(&n.key).is_some_and(|p| matcher.is_match(p.replace('\\', "/")))
            {
                candidates.insert(n.key.clone(), n);
            }
        }
    }
    for (_, n) in candidates {
        let Some(rule) = rules.matching_rule(&n.key) else {
            continue;
        };
        if boundaries
            .get(&n.key)
            .is_some_and(|b| matches!(b.role, "installation" | "user_data"))
        {
            continue;
        }
        let confidence = if rule.community { "medium" } else { "high" };
        let linked = index
            .origin(&n.key)
            .or_else(|| index.named_origin(&rule.owner));
        let mut b = boundary(
            n,
            rule.owner.clone(),
            "application_data",
            confidence,
            role(rule),
            format!(
                "{}：{} · {}；{}",
                if rule.community {
                    "社区规则"
                } else {
                    "内置规则"
                },
                rule.name,
                rule.id,
                rule.purpose
            ),
        );
        if let Some(origin) = linked {
            b.key = origin.application_key.clone();
            b.name = origin.name.clone();
            b.confidence = weaker_confidence(confidence, origin.confidence);
            b.evidence.push_str(&format!(
                "；{}：{}",
                origin.evidence.source, origin.evidence.detail
            ));
        }
        boundaries.insert(b.node.key.clone(), b);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_roles_do_not_treat_all_application_data_as_cache() {
        let rules = RuleSet::load(false).unwrap();
        assert_eq!(
            role(rules.rules.iter().find(|r| r.id == "user-temp").unwrap()),
            "temporary"
        );
        assert_eq!(
            role(rules.rules.iter().find(|r| r.id == "crash-dumps").unwrap()),
            "diagnostic"
        );
        assert_eq!(
            role(rules.rules.iter().find(|r| r.id == "chrome-cache").unwrap()),
            "cache"
        );
    }
}
