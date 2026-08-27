//! Application boundaries are an accounting layer above the file index.
//! All reads use the selected snapshot; this does not scan outside the user's root.
use crate::{safety::SafetyPolicy, store::Store};
use anyhow::Result;
use cleaner_domain::*;
use cleaner_platform::{normalize, within};
use rusqlite::{params, Connection, OptionalExtension, Row};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
mod hierarchy;

const NODE: &str = "id,path_key,parent_key,json_extract(data,'$.path'),json_extract(data,'$.name'),logical,allocated,file_count,complete,risk";

#[derive(Clone)]
struct Node {
    id: i64,
    key: String,
    path: String,
    name: String,
    logical: u64,
    allocated: Option<u64>,
    files: u64,
    complete: bool,
    risk: String,
}
fn decode(r: &Row<'_>) -> rusqlite::Result<Node> {
    Ok(Node {
        id: r.get(0)?,
        key: r.get(1)?,
        path: r.get(3)?,
        name: r.get(4)?,
        logical: r.get(5)?,
        allocated: r.get(6)?,
        files: r.get(7)?,
        complete: r.get(8)?,
        risk: r.get(9)?,
    })
}
fn node(c: &Connection, scan: &str, path: &str) -> Result<Option<Node>> {
    Ok(c.prepare_cached(&format!(
        "SELECT {NODE} FROM entries WHERE scan_id=?1 AND path_key=?2"
    ))?
    .query_row(params![scan, normalize(path)], decode)
    .optional()?)
}
fn parent(path: &str) -> Option<&str> {
    let i = path.rfind('\\')?;
    if i <= 2 {
        (path.len() > 2).then_some(&path[..2])
    } else {
        Some(&path[..i])
    }
}
fn ancestor<'a>(path: &str, boundaries: &'a BTreeMap<String, Boundary>) -> Option<&'a Boundary> {
    let mut next = Some(path);
    while let Some(p) = next {
        if let Some(b) = boundaries.get(p) {
            return Some(b);
        }
        next = parent(p);
    }
    None
}
#[derive(Clone)]
struct Boundary {
    node: Node,
    key: String,
    name: String,
    kind: &'static str,
    confidence: &'static str,
    role: &'static str,
    evidence: String,
}
fn boundary(
    node: Node,
    name: String,
    kind: &'static str,
    confidence: &'static str,
    role: &'static str,
    evidence: String,
) -> Boundary {
    Boundary {
        key: format!("{kind}:{}", node.key),
        node,
        name,
        kind,
        confidence,
        role,
        evidence,
    }
}

pub fn build(store: &Store, scan_id: &str) -> Result<Vec<ApplicationUnit>> {
    let c = store.connection()?;
    // Keep all reads in one coherent snapshot while a worker is committing batches.
    c.execute_batch("BEGIN DEFERRED")?;
    let root_path: String = c.query_row("SELECT root FROM scans WHERE id=?1", [scan_id], |r| {
        r.get(0)
    })?;
    let Some(root) = node(&c, scan_id, &root_path)? else {
        return Ok(vec![]);
    };
    let settings = store.settings()?;
    let policy = SafetyPolicy::new(settings.clone());
    let apps = store.apps(scan_id)?;
    let mut boundaries = BTreeMap::<String, Boundary>::new();
    for app in &apps {
        if !policy.specific_install_root(&app.install_location) {
            continue;
        }
        let location = if within(&root_path, &app.install_location) {
            &root_path
        } else {
            &app.install_location
        };
        if !within(location, &root_path) {
            continue;
        }
        let Some(n) = node(&c, scan_id, location)? else {
            continue;
        };
        let confidence = if app.source.contains("快捷方式") {
            "medium"
        } else {
            "high"
        };
        let b = boundary(
            n,
            app.name.clone(),
            "application",
            confidence,
            "installation",
            format!(
                "{}：{}；只统计本次扫描范围，不代表可直接删除",
                app.source, app.install_location
            ),
        );
        // Duplicate uninstall/MSIX/shortcut records for the same root count only once.
        if boundaries
            .get(&b.node.key)
            .is_none_or(|old| old.confidence != "high" && confidence == "high")
        {
            boundaries.insert(b.node.key.clone(), b);
        }
    }
    // An overly broad installer record must not claim a multi-application container.
    let shared: Vec<_> = boundaries
        .keys()
        .filter(|p| {
            boundaries
                .keys()
                .filter(|q| *q != *p && within(q, p))
                .take(2)
                .count()
                >= 2
        })
        .cloned()
        .collect();
    for key in shared {
        // Launchers may legitimately contain other apps, but also own their own
        // executable. Keep their residual bytes as a separate application.
        let owns_executable: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM entries WHERE scan_id=?1 AND parent_key=?2 AND is_dir=0 AND path_key LIKE '%.exe')", params![scan_id,key], |r| r.get(0))?;
        if !owns_executable {
            boundaries.remove(&key);
        }
    }

    // Only explicit user labels may join arbitrary locations into one unit.
    for (path, label) in &settings.labels {
        if !within(path, &root_path) {
            continue;
        }
        if let Some(n) = node(&c, scan_id, path)? {
            let mut b = boundary(
                n,
                label.clone(),
                "application_data",
                "high",
                "user_data",
                "用户手动标注；用途与删除风险仍独立判断".into(),
            );
            b.key = format!("label:{}", label.to_lowercase());
            boundaries.insert(b.node.key.clone(), b);
        }
    }
    // Cache rules define data roots, not an application for every individual file.
    let mut rules = c.prepare(&format!("SELECT {NODE},owner,rule_id FROM entries e WHERE scan_id=?1 AND is_dir=1 AND rule_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM entries p WHERE p.scan_id=e.scan_id AND p.path_key=e.parent_key AND p.rule_id=e.rule_id)"))?;
    let rows = rules.query_map([scan_id], |r| {
        Ok((decode(r)?, r.get::<_, Option<String>>(10)?))
    })?;
    for row in rows {
        let (n, owner) = row?;
        if boundaries.contains_key(&n.key) {
            continue;
        }
        let Some(owner) = owner else {
            continue;
        };
        let matches: Vec<_> = boundaries
            .values()
            .filter(|b| {
                b.kind == "application"
                    && b.role == "installation"
                    && b.name.eq_ignore_ascii_case(&owner)
            })
            .collect();
        let mut b = boundary(
            n,
            owner.clone(),
            "application_data",
            "high",
            "cache",
            "匹配到已知应用数据规则；不因归属而放宽清理保护".into(),
        );
        if let [app] = matches.as_slice() {
            b.key = app.key.clone();
            b.kind = app.kind;
            b.name = app.name.clone();
            b.confidence = app.confidence;
        } else {
            b.key = format!("data:{}", owner.to_lowercase());
        }
        boundaries.insert(b.node.key.clone(), b);
    }

    // For portable/unregistered software, executable layout is a low-confidence clue.
    // No executables are run, no binary contents or private configuration are read.
    let mut exe_parents = c.prepare("SELECT DISTINCT parent_key FROM entries WHERE scan_id=?1 AND is_dir=0 AND path_key LIKE '%.exe'")?;
    let mut candidates = BTreeMap::new();
    for key in exe_parents.query_map([scan_id], |r| r.get::<_, String>(0))? {
        let mut key = key?;
        if ancestor(&key, &boundaries).is_some() {
            continue;
        }
        for _ in 0..4 {
            let name = key.rsplit('\\').next().unwrap_or("");
            if !matches!(
                name,
                "bin"
                    | "bin64"
                    | "win64"
                    | "win32"
                    | "binaries"
                    | "x64"
                    | "x86"
                    | "release"
                    | "debug"
            ) {
                break;
            }
            let Some(p) = parent(&key) else {
                break;
            };
            if !within(p, &root_path) {
                break;
            }
            key = p.into();
        }
        let Some(n) = node(&c, scan_id, &key)? else {
            continue;
        };
        if policy.system_roots.iter().any(|p| within(&n.path, p))
            || !policy.specific_install_root(&n.path)
            || matches!(
                n.name.to_lowercase().as_str(),
                "games" | "apps" | "downloads" | "tools" | "common" | "steamapps"
            )
            || boundaries.keys().any(|p| within(p, &n.key))
        {
            continue;
        }
        candidates.insert(n.key.clone(), boundary(n.clone(), n.name.clone(), "possible_application", "low", "unconfirmed", "包含可执行文件的目录结构线索；可能是独立应用，也可能是安装包或工具集合，未确认真实产品名".into()));
    }
    // Collapse nested helpers/resources into the outer candidate application.
    for (key, candidate) in candidates {
        if ancestor(&key, &boundaries).is_none() {
            boundaries.insert(key, candidate);
        }
    }

    // Preserve all remaining bytes. Containers are residual groups, not guessed apps.
    let mut children = c.prepare(&format!(
        "SELECT {NODE} FROM entries WHERE scan_id=?1 AND parent_key=?2 AND is_dir=1"
    ))?;
    for n in children.query_map(params![scan_id, root.key], decode)? {
        let n = n?;
        if ancestor(&n.key, &boundaries).is_some() {
            continue;
        }
        boundaries.entry(n.key.clone()).or_insert_with(|| {
            boundary(
                n.clone(),
                n.name.clone(),
                "unassigned",
                "low",
                "unclassified",
                "此目录中尚未归属的内容；已扣除其下独立识别的应用".into(),
            )
        });
    }
    boundaries.entry(root.key.clone()).or_insert_with(|| {
        boundary(
            root.clone(),
            "扫描根目录的其余内容".into(),
            "unassigned",
            "low",
            "unclassified",
            "扫描根目录中的未归属内容；不是一个已识别应用".into(),
        )
    });
    assemble(boundaries, &root.key)
}

fn assemble(boundaries: BTreeMap<String, Boundary>, root: &str) -> Result<Vec<ApplicationUnit>> {
    let mut deductions: HashMap<String, (u64, u64, u64)> = HashMap::new();
    for (path, b) in &boundaries {
        if let Some(a) = parent(path).and_then(|p| ancestor(p, &boundaries)) {
            let d = deductions.entry(a.node.key.clone()).or_default();
            d.0 = d.0.saturating_add(b.node.logical);
            d.1 =
                d.1.saturating_add(b.node.allocated.unwrap_or(b.node.logical));
            d.2 = d.2.saturating_add(b.node.files);
        }
    }
    let mut units = BTreeMap::<String, ApplicationUnit>::new();
    for (path, b) in boundaries {
        let d = deductions.get(&path).copied().unwrap_or_default();
        let logical = b.node.logical.saturating_sub(d.0);
        let occupied = b
            .node
            .allocated
            .unwrap_or(b.node.logical)
            .saturating_sub(d.1);
        let files = b.node.files.saturating_sub(d.2);
        if files == 0
            && logical == 0
            && occupied == 0
            && b.kind == "unassigned"
            && (path == root || !deductions.contains_key(&path))
        {
            continue;
        }
        let id = format!("{:x}", Sha256::digest(b.key.as_bytes()));
        let u = units.entry(b.key).or_insert_with(|| ApplicationUnit {
            id,
            name: b.name,
            kind: b.kind.into(),
            confidence: b.confidence.into(),
            logical_bytes: 0,
            occupied_bytes: 0,
            file_count: 0,
            estimated: false,
            complete: true,
            components: vec![],
            children: vec![],
        });
        u.logical_bytes = u.logical_bytes.saturating_add(logical);
        u.occupied_bytes = u.occupied_bytes.saturating_add(occupied);
        u.file_count = u.file_count.saturating_add(files);
        u.estimated |= b.node.allocated.is_none();
        u.complete &= b.node.complete;
        u.components.push(UnitComponent {
            entry_id: b.node.id,
            path: b.node.path,
            role: b.role.into(),
            evidence: b.evidence,
            logical_bytes: logical,
            occupied_bytes: occupied,
            file_count: files,
            protected: b.node.risk == "protected",
        });
    }
    Ok(hierarchy::nest(units.into_values().collect(), root))
}

pub fn page(
    units: &[ApplicationUnit],
    search: &str,
    offset: usize,
    limit: usize,
) -> ApplicationUnitPage {
    let (applications, uncertain) = hierarchy::counts(units);
    let occupied_bytes = units.iter().map(|u| u.occupied_bytes).sum();
    let estimated = units.iter().any(|u| u.estimated);
    let query = search.to_lowercase();
    let filtered: Vec<_> = units
        .iter()
        .filter(|u| hierarchy::matches(u, &query))
        .collect();
    ApplicationUnitPage {
        total: filtered.len(),
        applications,
        uncertain,
        occupied_bytes,
        estimated,
        items: filtered
            .into_iter()
            .skip(offset)
            .take(limit.clamp(1, 100))
            .cloned()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn n(path: &str, bytes: u64, files: u64) -> Node {
        Node {
            id: 1,
            key: normalize(path),
            path: path.into(),
            name: path.rsplit('\\').next().unwrap().into(),
            logical: bytes,
            allocated: Some(bytes),
            files,
            complete: true,
            risk: "protected".into(),
        }
    }
    #[test]
    fn nested_applications_are_independent_and_bytes_are_conserved() {
        let mut roots = BTreeMap::new();
        for (path, size, files, name, kind) in [
            ("D:\\Games", 310, 4, "Games", "unassigned"),
            ("D:\\Games\\Game A", 100, 1, "Game A", "application"),
            ("D:\\Games\\Game B", 200, 2, "Game B", "application"),
        ] {
            roots.insert(
                normalize(path),
                boundary(
                    n(path, size, files),
                    name.into(),
                    kind,
                    "high",
                    "installation",
                    String::new(),
                ),
            );
        }
        let result = assemble(roots, &normalize("D:\\Games")).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.iter().map(|u| u.occupied_bytes).sum::<u64>(), 310);
        assert_eq!(
            result
                .iter()
                .find(|u| u.name == "Game A")
                .unwrap()
                .occupied_bytes,
            100
        );
        assert_eq!(
            result
                .iter()
                .find(|u| u.name == "Games")
                .unwrap()
                .occupied_bytes,
            10
        );
    }
    #[test]
    fn same_application_components_merge_without_counting_twice() {
        let mut roots = BTreeMap::new();
        for (path, size, role) in [
            ("D:\\App", 120, "installation"),
            ("D:\\App\\Cache", 20, "cache"),
        ] {
            let mut b = boundary(
                n(path, size, 1),
                "App".into(),
                "application",
                "high",
                role,
                String::new(),
            );
            b.key = "one-app".into();
            roots.insert(normalize(path), b);
        }
        let result = assemble(roots, &normalize("D:\\App")).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].occupied_bytes, 120);
        assert_eq!(result[0].components.len(), 2);
        assert!(result[0].components.iter().all(|c| c.protected));
    }
    #[test]
    fn pagination_keeps_independent_same_named_installations() {
        let mut roots = BTreeMap::new();
        for path in ["D:\\Games\\Copy1", "D:\\Games\\Copy2"] {
            roots.insert(
                normalize(path),
                boundary(
                    n(path, 10, 1),
                    "Game".into(),
                    "application",
                    "high",
                    "installation",
                    String::new(),
                ),
            );
        }
        let result = assemble(roots, &normalize("D:\\Games")).unwrap();
        assert_ne!(result[0].id, result[1].id);
        let page = page(&result, "game", 1, 1);
        assert_eq!(page.total, 2);
        assert_eq!(page.items.len(), 1);
    }

    fn indexed_fixture(
        root: &str,
        paths: &[(&str, bool, u64)],
        apps: Vec<InstalledApp>,
    ) -> (tempfile::TempDir, Store) {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("units.sqlite")).unwrap();
        store
            .save_scan(&Scan {
                id: "s".into(),
                root: root.into(),
                status: "complete".into(),
                ..Default::default()
            })
            .unwrap();
        store.save_apps("s", &apps).unwrap();
        let records: Vec<_> = paths
            .iter()
            .map(|(p, dir, size)| FileRecord {
                path: (*p).into(),
                parent: parent(p).unwrap_or("").into(),
                name: p.rsplit('\\').next().unwrap().into(),
                is_dir: *dir,
                logical_bytes: *size,
                allocated_bytes: Some(*size),
                file_count: if *dir { 0 } else { 1 },
                complete: true,
                enumerated: true,
                assessment: Assessment {
                    risk: "review".into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .collect();
        Store::insert_batch(&mut store.connection().unwrap(), "s", &records).unwrap();
        store.aggregate("s").unwrap();
        (temp, store)
    }
    fn app(name: &str, path: &str) -> InstalledApp {
        InstalledApp {
            id: name.into(),
            name: name.into(),
            publisher: String::new(),
            install_location: path.into(),
            source: "test manifest".into(),
            last_used: None,
        }
    }
    #[test]
    fn manifest_boundaries_override_folder_names_and_helpers_do_not_split() {
        let (_temp, store) = indexed_fixture(
            "D:\\Games",
            &[
                ("D:\\Games", true, 0),
                ("D:\\Games\\123", true, 0),
                ("D:\\Games\\123\\a.exe", false, 40),
                ("D:\\Games\\123\\bin", true, 0),
                ("D:\\Games\\123\\bin\\helper.exe", false, 10),
                ("D:\\Games\\456", true, 0),
                ("D:\\Games\\456\\b.dat", false, 70),
            ],
            vec![
                app("First Game", "D:\\Games\\123"),
                app("Second Game", "D:\\Games\\456"),
            ],
        );
        let result = build(&store, "s").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result
                .iter()
                .find(|u| u.name == "First Game")
                .unwrap()
                .occupied_bytes,
            50
        );
        assert_eq!(result.iter().map(|u| u.occupied_bytes).sum::<u64>(), 120);
    }
    #[test]
    fn unregistered_binary_layout_is_only_a_candidate_and_promotes_bin() {
        let (_temp, store) = indexed_fixture(
            "D:\\Games",
            &[
                ("D:\\Games", true, 0),
                ("D:\\Games\\Portable", true, 0),
                ("D:\\Games\\Portable\\bin", true, 0),
                ("D:\\Games\\Portable\\bin\\app.exe", false, 80),
                ("D:\\Games\\readme.txt", false, 20),
            ],
            vec![],
        );
        let result = build(&store, "s").unwrap();
        let portable = result.iter().find(|u| u.name == "Portable").unwrap();
        assert_eq!(portable.kind, "possible_application");
        assert_eq!(portable.confidence, "low");
        assert_eq!(portable.occupied_bytes, 80);
        assert_eq!(result.iter().map(|u| u.occupied_bytes).sum::<u64>(), 100);
    }
    #[test]
    fn scanning_inside_one_application_does_not_turn_subdirectories_into_apps() {
        let (_temp, store) = indexed_fixture(
            "D:\\App\\Data",
            &[
                ("D:\\App\\Data", true, 0),
                ("D:\\App\\Data\\one", true, 0),
                ("D:\\App\\Data\\one\\a.bin", false, 55),
            ],
            vec![app("App", "D:\\App")],
        );
        let result = build(&store, "s").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "App");
        assert_eq!(result[0].occupied_bytes, 55);
    }
    #[test]
    fn rule_data_roots_merge_with_an_unambiguous_matching_application() {
        let (_temp, store) = indexed_fixture(
            "D:\\Scope",
            &[
                ("D:\\Scope", true, 0),
                ("D:\\Scope\\App", true, 0),
                ("D:\\Scope\\App\\app.exe", false, 80),
                ("D:\\Scope\\Cache", true, 0),
                ("D:\\Scope\\Cache\\c.bin", false, 20),
                ("D:\\Scope\\Cache2", true, 0),
                ("D:\\Scope\\Cache2\\c.bin", false, 10),
            ],
            vec![app("App", "D:\\Scope\\App")],
        );
        store
            .connection()
            .unwrap()
            .execute(
                "UPDATE entries SET owner='App',rule_id='cache' WHERE path_key LIKE '%cache%'",
                [],
            )
            .unwrap();
        let result = build(&store, "s").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].components.len(), 3);
        assert_eq!(result[0].occupied_bytes, 110);
    }

    #[test]
    fn runtime_stays_inside_its_directory_with_inclusive_totals_and_search_context() {
        let (_temp, store) = indexed_fixture(
            "D:\\Games",
            &[
                ("D:\\Games", true, 0),
                ("D:\\Games\\GPT-SoVITS", true, 0),
                ("D:\\Games\\GPT-SoVITS\\model.bin", false, 270),
                ("D:\\Games\\GPT-SoVITS\\runtime", true, 0),
                ("D:\\Games\\GPT-SoVITS\\runtime\\python.exe", false, 74),
                ("D:\\Games\\Other", true, 0),
                ("D:\\Games\\Other\\data.bin", false, 10),
            ],
            vec![],
        );
        let result = build(&store, "s").unwrap();
        assert_eq!(result.len(), 2);
        let gpt = &result[0];
        assert_eq!(gpt.name, "GPT-SoVITS");
        assert_eq!(gpt.occupied_bytes, 344);
        assert_eq!(gpt.file_count, 2);
        assert_eq!(gpt.components[0].occupied_bytes, 270);
        assert_eq!(gpt.children[0].name, "runtime");
        assert_eq!(gpt.children[0].occupied_bytes, 74);
        assert_eq!(gpt.children[0].kind, "possible_application");
        let filtered = page(&result, "runtime", 0, 100);
        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.items[0].name, "GPT-SoVITS");
        assert_eq!(filtered.occupied_bytes, 354);
    }

    #[test]
    fn a_container_with_no_residual_files_is_not_lost() {
        let (_temp, store) = indexed_fixture(
            "D:\\Games",
            &[
                ("D:\\Games", true, 0),
                ("D:\\Games\\Bundle", true, 0),
                ("D:\\Games\\Bundle\\runtime", true, 0),
                ("D:\\Games\\Bundle\\runtime\\python.exe", false, 74),
            ],
            vec![],
        );
        let result = build(&store, "s").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Bundle");
        assert_eq!(result[0].occupied_bytes, 74);
        assert_eq!(result[0].children[0].name, "runtime");
    }

    #[test]
    fn drive_root_totals_do_not_count_children_twice() {
        let (_temp, store) = indexed_fixture(
            "D:\\",
            &[
                ("D:\\", true, 0),
                ("D:\\App", true, 0),
                ("D:\\App\\app.exe", false, 80),
            ],
            vec![],
        );
        let result = build(&store, "s").unwrap();
        assert_eq!(result.iter().map(|u| u.occupied_bytes).sum::<u64>(), 80);
    }
}
