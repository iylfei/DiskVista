use super::{generic_container, ApplicationOrigin, OriginKind};
use crate::{application_index::ApplicationIndex, safety::SafetyPolicy, store::Store};
use anyhow::Result;
use cleaner_domain::Evidence;
use cleaner_platform::{normalize, within};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeMap;

fn parent(path: &str) -> Option<&str> {
    path.rsplit_once('\\').map(|(parent, _)| parent)
}

fn layout_parent(name: &str) -> bool {
    matches!(
        name,
        "bin"
            | "bin32"
            | "bin64"
            | "binaries"
            | "x64"
            | "x86"
            | "win32"
            | "win64"
            | "release"
            | "debug"
            | "runtime"
            | "runtimes"
    )
}

fn embedded_dependency(path: &str) -> bool {
    path.split('\\').any(|part| {
        matches!(
            part,
            "node_modules" | ".git" | ".venv" | "venv" | "site-packages"
        )
    })
}

fn directory_name(c: &Connection, scan: &str, path: &str) -> Result<Option<String>> {
    Ok(c
        .prepare_cached(
            "SELECT json_extract(data,'$.name') FROM entries WHERE scan_id=?1 AND path_key=?2 AND is_dir=1",
        )?
        .query_row(params![scan, path], |r| r.get(0))
        .optional()?)
}

fn contextual_name(c: &Connection, scan: &str, path: &str, name: String) -> Result<String> {
    let version = name.trim_start_matches(['v', 'V']);
    let needs_context = name.eq_ignore_ascii_case("optimized")
        || (version.bytes().any(|b| b.is_ascii_digit())
            && version
                .bytes()
                .all(|b| b.is_ascii_digit() || b"._-".contains(&b)));
    if !needs_context {
        return Ok(name);
    }
    let mut names = vec![name];
    let mut ancestor = parent(path);
    for _ in 0..2 {
        let Some(path) = ancestor else { break };
        let generic = generic_container(path);
        if generic && names.len() > 1 {
            break;
        }
        let Some(name) = directory_name(c, scan, path)? else {
            break;
        };
        names.push(name);
        if generic {
            break;
        }
        ancestor = parent(path);
    }
    names.reverse();
    Ok(names.join(" / "))
}

pub(super) fn add(
    store: &Store,
    scan: &str,
    policy: &SafetyPolicy,
    index: &mut ApplicationIndex,
) -> Result<()> {
    let c = store.connection()?;
    c.execute_batch("BEGIN DEFERRED")?;
    let root: String = c.query_row("SELECT root FROM scans WHERE id=?1", [scan], |r| r.get(0))?;
    let root = normalize(&root);
    let paths: Vec<_> = index
        .origins()
        .filter(|origin| origin.kind == OriginKind::Installation)
        .map(|origin| origin.path.clone())
        .collect();
    for path in &paths {
        if !within(path, &root)
            || paths
                .iter()
                .filter(|other| *other != path && within(other, path))
                .take(2)
                .count()
                < 2
        {
            continue;
        }
        let has_executable: bool = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM entries WHERE scan_id=?1 AND parent_key=?2 AND is_dir=0 AND path_key LIKE '%.exe')",
            params![scan, path],
            |r| r.get(0),
        )?;
        if !has_executable {
            index.remove_origin(path);
        }
    }

    let mut statement = c.prepare(
        "SELECT DISTINCT parent_key FROM entries WHERE scan_id=?1 AND is_dir=0 AND path_key LIKE '%.exe'
         AND (COALESCE(json_extract(data,'$.attributes'),0) & ?2)=0 ORDER BY parent_key",
    )?;
    let executable_parents = statement
        .query_map(
            params![
                scan,
                cleaner_platform::filesystem::REPARSE
                    | cleaner_platform::filesystem::OFFLINE
                    | cleaner_platform::filesystem::RECALL
            ],
            |r| r.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let windows = normalize(&std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()));
    let mut candidates = BTreeMap::new();
    for original in executable_parents {
        if index.origin(&original).is_some()
            || embedded_dependency(&original)
            || within(&original, &windows)
        {
            continue;
        }
        let mut path = original.as_str();
        for _ in 0..4 {
            if !layout_parent(path.rsplit('\\').next().unwrap_or("")) {
                break;
            }
            let Some(next) = parent(path) else { break };
            if !within(next, &root)
                || !policy.specific_install_root(next)
                || generic_container(next)
            {
                break;
            }
            path = next;
        }
        if generic_container(path)
            || layout_parent(path.rsplit('\\').next().unwrap_or(""))
            || !policy.specific_install_root(path)
            || index.origin(path).is_some()
            || index.origins().any(|origin| within(&origin.path, path))
        {
            continue;
        }
        let Some(name) = directory_name(&c, scan, path)? else {
            continue;
        };
        let name = contextual_name(&c, scan, path, name)?;
        let origin = ApplicationOrigin {
                path: path.into(),
                application_key: format!("possible_application:{path}"),
                name,
                kind: OriginKind::Portable,
                confidence: "low",
                evidence: Evidence {
                    source: "扫描中的可执行文件布局".into(),
                    detail: format!(
                        "此目录内的 {original} 含可执行文件；可能是便携应用，也可能是安装材料或工具集合，未执行程序，未确认创建进程"
                    ),
                },
            };
        if path != original {
            let mut component = origin.clone();
            component.path = original.clone();
            candidates.insert(component.path.clone(), component);
        }
        candidates.insert(path.to_owned(), origin);
    }
    for (_, origin) in candidates {
        if index
            .origin(&origin.path)
            .is_none_or(|parent| parent.application_key == origin.application_key)
        {
            index.add_origin(origin);
        }
    }
    Ok(())
}
