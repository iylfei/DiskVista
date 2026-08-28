//! Application boundaries are an accounting layer above the file index.
//! All reads use the selected snapshot; this does not scan outside the user's root.
use crate::{
    application_index::ApplicationIndex, application_origins::confidence_rank, rules::RuleSet,
    safety::SafetyPolicy, store::Store,
};
use anyhow::Result;
use cleaner_domain::*;
use cleaner_platform::{normalize, within};
use rusqlite::{params, Connection, OptionalExtension, Row};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
mod hierarchy;
mod rule_boundaries;

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
    let policy = SafetyPolicy::new(store.settings()?);
    let index = ApplicationIndex::for_scan(store, scan_id, &policy)?;
    build_indexed(store, scan_id, &policy, &index)
}

pub fn build_indexed(
    store: &Store,
    scan_id: &str,
    policy: &SafetyPolicy,
    index: &ApplicationIndex,
) -> Result<Vec<ApplicationUnit>> {
    let rules = RuleSet::load(policy.settings.community_enabled)?;
    build_with_rules(store, scan_id, policy, index, &rules)
}

fn build_with_rules(
    store: &Store,
    scan_id: &str,
    policy: &SafetyPolicy,
    index: &ApplicationIndex,
    rules: &RuleSet,
) -> Result<Vec<ApplicationUnit>> {
    let c = store.connection()?;
    // Keep all reads in one coherent snapshot while a worker is committing batches.
    c.execute_batch("BEGIN DEFERRED")?;
    let root_path: String = c.query_row("SELECT root FROM scans WHERE id=?1", [scan_id], |r| {
        r.get(0)
    })?;
    let Some(root) = node(&c, scan_id, &root_path)? else {
        return Ok(vec![]);
    };
    let settings = &policy.settings;
    let mut boundaries = BTreeMap::<String, Boundary>::new();
    for origin in index.origins() {
        let location = if within(&root_path, &origin.path) {
            &root_path
        } else {
            &origin.path
        };
        if !within(location, &root_path) {
            continue;
        }
        let Some(n) = node(&c, scan_id, location)? else {
            continue;
        };
        let mut b = boundary(
            n,
            origin.name.clone(),
            origin.unit_kind(),
            origin.confidence,
            origin.role(),
            format!("{}：{}", origin.evidence.source, origin.evidence.detail),
        );
        b.key = origin.application_key.clone();
        // When scanning inside an application, the deepest known boundary wins.
        boundaries.insert(b.node.key.clone(), b);
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
    rule_boundaries::add(&c, scan_id, &root, rules, index, &mut boundaries)?;

    // Preserve all remaining bytes. Containers are residual groups, not guessed apps.
    let mut children = c.prepare(&format!(
        "SELECT {NODE} FROM entries WHERE scan_id=?1 AND parent_key=?2 AND is_dir=1"
    ))?;
    for n in children.query_map(params![scan_id, root.key], decode)? {
        let n = n?;
        if n.key == root.key || ancestor(&n.key, &boundaries).is_some() {
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
    for b in boundaries.values_mut() {
        let file = FileRecord {
            path: b.node.path.clone(),
            name: b.node.name.clone(),
            is_dir: true,
            complete: b.node.complete,
            ..Default::default()
        };
        if !file.complete
            || policy.reason(&file).is_some()
            || index.installed_reason(&file).is_some()
        {
            b.node.risk = "protected".into();
        }
    }
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
        if files == 0 && logical == 0 && occupied == 0 && b.kind == "unassigned" {
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
        if b.kind == "application" {
            u.kind = b.kind.into();
        }
        if confidence_rank(b.confidence) < confidence_rank(&u.confidence) {
            u.confidence = b.confidence.into();
        }
        let mut evidence = b.evidence;
        if deductions.contains_key(&path) {
            evidence.push_str("；此处计入的大小已扣除单独归属的子目录；打开目录可查看全部内容");
        }
        u.components.push(UnitComponent {
            entry_id: b.node.id,
            path: b.node.path,
            role: b.role.into(),
            evidence,
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
    fn stale_rule_labels_do_not_override_current_rules() {
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
        assert_eq!(result.len(), 3);
        assert_eq!(result.iter().map(|u| u.occupied_bytes).sum::<u64>(), 110);
        assert_eq!(
            result
                .iter()
                .find(|u| u.name == "App")
                .unwrap()
                .occupied_bytes,
            80
        );
        assert!(result
            .iter()
            .flat_map(|u| &u.components)
            .all(|c| c.role != "cache"));
    }

    #[test]
    fn known_data_and_current_cache_rule_join_one_recorded_application() {
        let root = std::env::var("APPDATA").unwrap();
        let install = format!("{root}\\FixtureInstalledCode");
        let executable = format!("{install}\\code.exe");
        let data = format!("{root}\\Code");
        let user_file = format!("{data}\\settings.json");
        let cache = format!("{data}\\Cache");
        let cache_file = format!("{cache}\\c.bin");
        let (_temp, store) = indexed_fixture(
            &root,
            &[
                (&root, true, 0),
                (&install, true, 0),
                (&executable, false, 80),
                (&data, true, 0),
                (&user_file, false, 10),
                (&cache, true, 0),
                (&cache_file, false, 20),
            ],
            vec![app("Visual Studio Code", &install)],
        );
        let result = build(&store, "s").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Visual Studio Code");
        assert_eq!(result[0].kind, "application");
        assert_eq!(result[0].confidence, "medium");
        assert_eq!(result[0].occupied_bytes, 110);
        assert_eq!(result[0].components.len(), 3);
        assert_eq!(
            result[0]
                .components
                .iter()
                .find(|c| c.role == "cache")
                .unwrap()
                .occupied_bytes,
            20
        );
        assert!(result[0]
            .components
            .iter()
            .find(|c| c.path == data)
            .unwrap()
            .evidence
            .contains("已扣除"));
    }

    #[test]
    fn changing_active_rules_rebuilds_boundaries_without_rewriting_the_snapshot() {
        let (_temp, store) = indexed_fixture(
            "D:\\FixtureScope",
            &[
                ("D:\\FixtureScope", true, 0),
                ("D:\\FixtureScope\\Logs", true, 0),
                ("D:\\FixtureScope\\Logs\\failure.log", false, 20),
            ],
            vec![],
        );
        let mut rule = RuleSet::load(false)
            .unwrap()
            .rules
            .into_iter()
            .find(|r| r.id == "crash-dumps")
            .unwrap();
        rule.id = "fixture-community-diagnostic".into();
        rule.root = "D:\\FixtureScope\\Logs".into();
        rule.owner = "Fixture Application".into();
        rule.community = true;
        let enabled = RuleSet::test_rules(vec![rule]);
        let disabled = RuleSet::test_rules(vec![]);
        let policy = SafetyPolicy::new(Default::default());
        let index = ApplicationIndex::for_scan(&store, "s", &policy).unwrap();
        let enabled_result = build_with_rules(&store, "s", &policy, &index, &enabled).unwrap();
        assert_eq!(enabled_result[0].name, "Fixture Application");
        assert_eq!(enabled_result[0].confidence, "medium");
        assert_eq!(enabled_result[0].components[0].role, "diagnostic");
        let disabled_result = build_with_rules(&store, "s", &policy, &index, &disabled).unwrap();
        assert_eq!(disabled_result[0].kind, "unassigned");
        assert_eq!(
            disabled_result[0].occupied_bytes,
            enabled_result[0].occupied_bytes
        );
        assert!(store
            .by_path("s", "D:\\FixtureScope\\Logs")
            .unwrap()
            .assessment
            .rule_id
            .is_none());
    }

    #[test]
    fn independent_applications_stay_visible_above_larger_residual_containers() {
        let (_temp, store) = indexed_fixture(
            "D:\\",
            &[
                ("D:\\", true, 0),
                ("D:\\Mixed", true, 0),
                ("D:\\Mixed\\other.bin", false, 900),
                ("D:\\Mixed\\One", true, 0),
                ("D:\\Mixed\\One\\a.bin", false, 100),
                ("D:\\Mixed\\Two", true, 0),
                ("D:\\Mixed\\Two\\a.bin", false, 200),
            ],
            vec![
                app("One App", "D:\\Mixed\\One"),
                app("Two App", "D:\\Mixed\\Two"),
            ],
        );
        let result = build(&store, "s").unwrap();
        assert_eq!(
            result.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(),
            ["Two App", "One App", "Mixed"]
        );
        assert_eq!(result.iter().map(|u| u.occupied_bytes).sum::<u64>(), 1200);
        assert_eq!(result[2].occupied_bytes, 900);
        assert!(result.iter().all(|u| u.children.is_empty()));
    }

    #[test]
    fn portable_application_in_program_files_is_identified_but_stays_protected() {
        let root = std::env::var("ProgramFiles").unwrap();
        let app = format!("{root}\\FixturePortable");
        let executable = format!("{app}\\app.exe");
        let (_temp, store) = indexed_fixture(
            &root,
            &[(&root, true, 0), (&app, true, 0), (&executable, false, 80)],
            vec![],
        );
        let result = build(&store, "s").unwrap();
        assert_eq!(result[0].name, "FixturePortable");
        assert_eq!(result[0].kind, "possible_application");
        assert!(result[0].components[0].protected);
        let policy = SafetyPolicy::new(Default::default());
        let index = ApplicationIndex::for_scan(&store, "s", &policy).unwrap();
        let file = store.by_path("s", &executable).unwrap();
        let assessment = RuleSet::load(false)
            .unwrap()
            .classify_indexed(&file, &policy, &index);
        assert_eq!(assessment.owner.as_deref(), Some("FixturePortable"));
        assert_eq!(assessment.confidence, "low");
        assert_eq!(assessment.risk, "protected");
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
        assert!(gpt.children.is_empty());
        assert_eq!(gpt.components[1].path, "D:\\Games\\GPT-SoVITS\\runtime");
        assert_eq!(gpt.components[1].occupied_bytes, 74);
        assert_eq!(gpt.kind, "possible_application");
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
        assert!(result[0].children.is_empty());
        assert_eq!(result[0].components[1].path, "D:\\Games\\Bundle\\runtime");
    }

    #[test]
    fn vendor_data_alias_does_not_hide_an_independent_child_application() {
        let root = std::env::var("LOCALAPPDATA").unwrap();
        let vendor = format!("{root}\\Epic Games");
        let shared = format!("{vendor}\\shared.bin");
        let services = format!("{vendor}\\Epic Online Services");
        let executable = format!("{services}\\service.exe");
        let settings = format!("{services}\\settings.json");
        let (_temp, store) = indexed_fixture(
            &root,
            &[
                (&root, true, 0),
                (&vendor, true, 0),
                (&shared, false, 10),
                (&services, true, 0),
                (&executable, false, 20),
                (&settings, false, 30),
            ],
            vec![app("Epic Games Launcher", "D:\\Installed\\Epic Games")],
        );
        let policy = SafetyPolicy::new(Default::default());
        let index = ApplicationIndex::for_scan(&store, "s", &policy).unwrap();
        assert!(index.origin(&normalize(&shared)).is_none());
        let origin = index.origin(&normalize(&settings)).unwrap();
        assert_eq!(origin.name, "Epic Online Services");
        assert_eq!(origin.confidence, "low");
        assert!(index
            .installed_reason(&FileRecord {
                path: "D:\\Installed\\Epic Games\\Launcher\\launcher.exe".into(),
                ..Default::default()
            })
            .is_some());
        let result = build_indexed(&store, "s", &policy, &index).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "Epic Online Services");
        assert_eq!(result[0].occupied_bytes, 50);
        assert_eq!(result.iter().map(|u| u.occupied_bytes).sum::<u64>(), 60);
        assert!(result.iter().all(|u| u.name != "Epic Games Launcher"));
    }

    #[test]
    fn versioned_layout_names_use_recorded_context_without_merging_roots() {
        let (_temp, store) = indexed_fixture(
            "D:\\Apps",
            &[
                ("D:\\Apps", true, 0),
                ("D:\\Apps\\AppCore", true, 0),
                ("D:\\Apps\\AppCore\\1.0.0", true, 0),
                ("D:\\Apps\\AppCore\\1.0.0\\app.exe", false, 10),
                ("D:\\Apps\\AppCore\\2.0.0", true, 0),
                ("D:\\Apps\\AppCore\\2.0.0\\app.exe", false, 20),
                ("D:\\Apps\\AppCore\\Optimized", true, 0),
                ("D:\\Apps\\AppCore\\Optimized\\app.exe", false, 30),
            ],
            vec![],
        );
        let policy = SafetyPolicy::new(Default::default());
        let index = ApplicationIndex::for_scan(&store, "s", &policy).unwrap();
        let result = build_indexed(&store, "s", &policy, &index).unwrap();
        assert_eq!(result.len(), 3);
        for (version, bytes) in [("1.0.0", 10), ("2.0.0", 20), ("Optimized", 30)] {
            let path = format!("D:\\Apps\\AppCore\\{version}");
            let normalized = normalize(&path);
            let origin = index.origin(&normalized).unwrap();
            assert_eq!(origin.path, normalized);
            assert_eq!(
                origin.application_key,
                format!("possible_application:{normalized}")
            );
            let unit = result
                .iter()
                .find(|u| u.name == format!("AppCore / {version}"))
                .unwrap();
            assert_eq!(unit.confidence, "low");
            assert_eq!(unit.components[0].path, path);
            assert_eq!(unit.occupied_bytes, bytes);
        }
        assert_eq!(result.iter().map(|u| u.occupied_bytes).sum::<u64>(), 60);
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
