//! Spill large candidate sets and sorted views into an automatically removed SQLite file.
use super::{
    query, AnalysisFilter, Assessment, Candidate, FileRecord, SuggestionGroup, SuggestionIndex,
    SuggestionPage, SuggestionQuery,
};
use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use std::{collections::VecDeque, sync::Mutex};

struct View {
    key: query::Key,
    slot: usize,
    groups: Vec<SuggestionGroup>,
    total: usize,
}

struct Data {
    connection: Connection,
    views: VecDeque<View>,
}

pub(super) struct DiskIndex {
    data: Mutex<Data>,
}

impl DiskIndex {
    pub fn new() -> Result<Self> {
        // An empty filename asks SQLite for a private on-disk database deleted on close.
        let connection = Connection::open("")?;
        connection.execute_batch("PRAGMA cache_size=-2048; PRAGMA temp_store=FILE; PRAGMA mmap_size=0;
            CREATE TABLE candidates(pos INTEGER PRIMARY KEY,id INTEGER,path TEXT,key TEXT,is_dir INTEGER,
                occupied BLOB,estimated INTEGER,changed INTEGER,assessment TEXT,grp INTEGER,risk TEXT,known INTEGER);
            BEGIN")?;
        Ok(Self {
            data: Mutex::new(Data {
                connection,
                views: VecDeque::new(),
            }),
        })
    }

    pub fn insert(&mut self, file: &Candidate, assessment: &Assessment) -> Result<()> {
        self.data.get_mut().unwrap().connection.prepare_cached(
            "INSERT INTO candidates(id,path,key,is_dir,occupied,estimated,changed,assessment,grp,risk,known)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)"
        )?.execute(params![file.id,file.path,file.key,file.is_dir,file.occupied.to_be_bytes().as_slice(),
            file.estimated,file.latest_change,serde_json::to_string(assessment)?,file.group as i64,
            assessment.risk,assessment.rule_id.is_some() || assessment.owner.is_some()])?;
        Ok(())
    }

    pub fn finish(&mut self) -> Result<()> {
        self.data.get_mut().unwrap().connection.execute_batch(
            "CREATE INDEX candidates_tree ON candidates(replace(key,char(92),char(1))); COMMIT",
        )?;
        Ok(())
    }

    fn with_view<T>(
        &self,
        index: &SuggestionIndex,
        query: &SuggestionQuery,
        analysis: Option<&AnalysisFilter>,
        read: impl FnOnce(&Connection, &View) -> Result<T>,
    ) -> Result<T> {
        let key = query::key(index, query, analysis)?;
        let mut data = self.data.lock().unwrap();
        if let Some(position) = data.views.iter().position(|view| view.key == key) {
            let view = data.views.remove(position).unwrap();
            data.views.push_front(view);
        } else {
            if data.views.len() == 4 {
                data.views.pop_back();
            }
            // A previous failed build can leave a gap anywhere in the four slots.
            let slot = (0..4)
                .find(|slot| data.views.iter().all(|view| view.slot != *slot))
                .unwrap();
            let view = build(&mut data.connection, index, key, slot)?;
            data.views.push_front(view);
        }
        read(&data.connection, data.views.front().unwrap())
    }

    pub fn page(
        &self,
        index: &SuggestionIndex,
        query: &SuggestionQuery,
        analysis: Option<&AnalysisFilter>,
    ) -> Result<SuggestionPage> {
        let (groups, total, selected) =
            self.with_view(index, query, analysis, |connection, view| {
                Ok((
                    view.groups.clone(),
                    view.total,
                    select(
                        connection,
                        view.slot,
                        query.offset,
                        query.limit.clamp(1, 100),
                        false,
                    )?,
                ))
            })?;
        Ok(SuggestionPage {
            groups,
            total,
            items: load(index, selected)?,
        })
    }

    pub fn selection(
        &self,
        index: &SuggestionIndex,
        query: &SuggestionQuery,
        analysis: Option<&AnalysisFilter>,
    ) -> Result<Vec<FileRecord>> {
        let selected = self.with_view(index, query, analysis, |connection, view| {
            select(connection, view.slot, 0, 501, true)
        })?;
        if selected.len() > 500 {
            bail!("这一类超过 500 项，请先缩小搜索范围或按页选择");
        }
        load(index, selected)
    }
}

fn select(
    connection: &Connection,
    slot: usize,
    offset: usize,
    limit: usize,
    allowed_only: bool,
) -> Result<Vec<(i64, String)>> {
    let sql = format!("SELECT c.id,c.assessment FROM view_{slot} v JOIN candidates c ON c.pos=v.pos {} ORDER BY v.rank LIMIT ?1 OFFSET ?2",
        if allowed_only { "WHERE c.risk<>'protected'" } else { "" });
    let mut statement = connection.prepare(&sql)?;
    let selected = statement
        .query_map(
            params![limit as i64, offset.min(i64::MAX as usize) as i64],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
        .collect::<rusqlite::Result<_>>()?;
    Ok(selected)
}

fn load(index: &SuggestionIndex, selected: Vec<(i64, String)>) -> Result<Vec<FileRecord>> {
    let connection = index.store.connection()?;
    let mut statement = connection.prepare_cached(&format!(
        "SELECT {} FROM entries WHERE scan_id=?1 AND id=?2",
        super::FIELDS
    ))?;
    selected
        .into_iter()
        .map(|(id, assessment)| {
            let mut file = statement.query_row(params![index.scan_id, id], super::decode)?;
            file.assessment = serde_json::from_str(&assessment)?;
            Ok(file)
        })
        .collect()
}

fn build(
    connection: &mut Connection,
    index: &SuggestionIndex,
    key: query::Key,
    slot: usize,
) -> Result<View> {
    let tx = connection.transaction()?;
    tx.execute_batch(&format!(
        "DROP TABLE IF EXISTS view_{slot};
        CREATE TABLE view_{slot}(rank INTEGER PRIMARY KEY,pos INTEGER);
        CREATE TEMP TABLE visible(pos INTEGER PRIMARY KEY);"
    ))?;
    let risk = match key.risk.as_str() {
        "protected" => "risk='protected'",
        "low" => "risk='low'",
        "unknown" => "risk NOT IN ('protected','keep') AND known=0",
        "known" => "risk NOT IN ('protected','keep') AND known=1",
        _ => "risk NOT IN ('protected','keep')",
    };
    // A separator must sort before filename characters: otherwise Cache-copy
    // separates Cache from Cache\child and would prematurely pop the ancestor stack.
    let sql = format!("SELECT pos,id,key,is_dir,grp,occupied,estimated,path FROM candidates WHERE {risk} AND instr(key,?1)>0 ORDER BY replace(key,char(92),char(1))");
    let mut statement = tx.prepare(&sql)?;
    let mut rows = statement.query([&key.search])?;
    let mut insert = tx.prepare("INSERT INTO visible VALUES(?1)")?;
    let mut ancestors: Vec<(String, usize)> = Vec::new();
    let mut groups = index.groups.clone();
    let mut total = 0;
    while let Some(row) = rows.next()? {
        let pos: i64 = row.get(0)?;
        let id: i64 = row.get(1)?;
        let path: String = row.get(2)?;
        let original_path: String = row.get(7)?;
        let is_dir: bool = row.get(3)?;
        if key
            .analysis
            .as_ref()
            .is_some_and(|filter| !filter.matches(id))
            || key.recycled.contains(&original_path)
            || (is_dir && key.recycled.affects_directory(&original_path))
        {
            continue;
        }
        let group: usize = row.get(4)?;
        while ancestors.last().is_some_and(|(parent, _)| {
            !path
                .strip_prefix(parent.as_str())
                .is_some_and(|suffix| suffix.starts_with('\\'))
        }) {
            ancestors.pop();
        }
        let covered = ancestors
            .iter()
            .any(|(_, ancestor_group)| *ancestor_group == group);
        if is_dir && !covered {
            ancestors.push((path, group));
        }
        if covered {
            continue;
        }
        let occupied: Vec<u8> = row.get(5)?;
        let occupied = u64::from_be_bytes(
            occupied
                .try_into()
                .map_err(|_| anyhow::anyhow!("建议大小记录无效"))?,
        );
        groups[group].count += 1;
        groups[group].occupied_bytes = groups[group].occupied_bytes.saturating_add(occupied);
        groups[group].estimated |= row.get::<_, bool>(6)?;
        if key.group.as_ref().is_none_or(|id| &groups[group].id == id) {
            insert.execute([pos])?;
            total += 1;
        }
    }
    drop(rows);
    drop(statement);
    drop(insert);
    let order = match key.sort.as_str() {
        "name" => "c.path,c.id",
        "activity" | "activity_desc" => "c.changed DESC,c.id",
        "activity_asc" => "c.changed,c.id",
        _ => "c.occupied DESC,c.id",
    };
    tx.execute_batch(&format!("INSERT INTO view_{slot}(pos) SELECT c.pos FROM visible v JOIN candidates c ON c.pos=v.pos ORDER BY {order}; DROP TABLE visible;"))?;
    tx.commit()?;
    groups.retain(|group| group.count > 0);
    groups.sort_by(|a, b| {
        (b.id == "large-files")
            .cmp(&(a.id == "large-files"))
            .then(b.recognized.cmp(&a.recognized))
            .then(b.occupied_bytes.cmp(&a.occupied_bytes))
            .then(a.name.cmp(&b.name))
    });
    Ok(View {
        key,
        slot,
        groups,
        total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_view_build_preserves_other_cached_views() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::Store::open(dir.path().join("views.sqlite")).unwrap();
        store
            .save_scan(&cleaner_domain::Scan {
                id: "s".into(),
                root: r"D:\fixture".into(),
                status: "complete".into(),
                ..Default::default()
            })
            .unwrap();
        let file = FileRecord {
            id: 1,
            path: r"D:\fixture\file.bin".into(),
            complete: true,
            assessment: Assessment {
                risk: "low".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        crate::store::Store::insert_batch(&mut store.connection().unwrap(), "s", &[file.clone()])
            .unwrap();
        let index = SuggestionIndex::from_records_with_budget(
            &store,
            "s",
            Default::default(),
            [Ok(file)].into_iter(),
            1,
        )
        .unwrap();
        let query = |sort: &str, search: &str| SuggestionQuery {
            sort: sort.into(),
            search: search.into(),
            limit: 100,
            ..Default::default()
        };
        for sort in ["size", "name", "activity_asc", "activity_desc", "size"] {
            assert_eq!(
                super::query::page(&index, &query(sort, "")).unwrap().total,
                1
            );
        }
        let disk = index.disk.as_ref().unwrap();
        disk.data
            .lock()
            .unwrap()
            .connection
            .execute_batch("PRAGMA query_only=ON")
            .unwrap();
        assert!(super::query::page(&index, &query("size", "missing")).is_err());
        disk.data
            .lock()
            .unwrap()
            .connection
            .execute_batch("PRAGMA query_only=OFF")
            .unwrap();
        assert_eq!(
            super::query::page(&index, &query("size", "missing"))
                .unwrap()
                .total,
            0
        );
        let cached = super::query::page(&index, &query("activity_desc", "")).unwrap();
        assert_eq!((cached.total, cached.items.len()), (1, 1));
    }
}
