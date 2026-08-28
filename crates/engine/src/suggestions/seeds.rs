use crate::{rules::RuleSet, store::FIELDS};
use cleaner_platform::within;
use rusqlite::types::Value;

pub(super) fn query(scan: &str, root: &str, rules: &RuleSet) -> (String, Vec<Value>) {
    let mut ranges: Vec<_> = rules
        .directory_roots()
        .filter_map(|(_, path, _)| {
            let scope = match path.find(['*', '?', '[', '{']) {
                Some(wildcard) => path[..wildcard].rsplit_once('\\')?.0,
                None => path,
            };
            if within(root, scope) {
                Some(root.to_owned())
            } else if within(scope, root) {
                Some(scope.to_owned())
            } else {
                None
            }
        })
        .collect();
    ranges.sort();
    let mut scopes: Vec<String> = Vec::new();
    for range in ranges {
        if !scopes.iter().any(|scope| within(&range, scope)) {
            scopes.push(range);
        }
    }
    let mut sql = format!(
        "SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND rule_id IS NOT NULL
         UNION ALL SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND rule_id IS NULL AND is_dir=0 AND logical>=104857600"
    );
    let mut parameters = vec![Value::from(scan.to_owned())];
    for scope in scopes {
        let base = parameters.len() + 1;
        // These ranges exclude both original branches and each other; the current
        // classifier still decides which rows actually match an enabled rule.
        sql.push_str(&format!(
            " UNION ALL SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND rule_id IS NULL
              AND (is_dir=1 OR logical<104857600)
              AND (path_key=?{base} OR (path_key>=?{} AND path_key<?{}))",
            base + 1,
            base + 2,
        ));
        parameters.extend([
            Value::from(scope.clone()),
            Value::from(format!("{scope}\\")),
            Value::from(format!("{scope}]")),
        ]);
    }
    (sql, parameters)
}
