use super::Store;
use anyhow::Result;
use rusqlite::{types::Value, Connection};
use std::{collections::VecDeque, path::PathBuf, sync::Mutex};

#[derive(Clone, PartialEq)]
pub(super) struct CountKey {
    pub conditions: String,
    pub arguments: Vec<Value>,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CountCacheStats {
    pub hits: u64,
    pub sql_counts: u64,
}

#[derive(Default)]
struct State {
    path: PathBuf,
    // data_version is meaningful only across reads on the same connection.
    observer: Option<Connection>,
    version: i64,
    generation: u64,
    values: VecDeque<(CountKey, u64)>,
    stats: CountCacheStats,
}

/// Small, process-local counts for completed directory snapshots. Writes from any
/// connection (including a scanner process) invalidate all retained counts.
#[derive(Default)]
pub struct CountCache {
    state: Mutex<State>,
}

impl CountCache {
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.generation = state.generation.wrapping_add(1);
        state.values.clear();
    }

    pub fn stats(&self) -> CountCacheStats {
        self.state.lock().unwrap().stats
    }

    pub(super) fn revision(&self, store: &Store) -> Result<u64> {
        let mut state = self.state.lock().unwrap();
        if state.observer.is_none() || state.path != store.path {
            state.observer = Some(store.connection()?);
            state.path.clone_from(&store.path);
            state.generation = state.generation.wrapping_add(1);
            state.values.clear();
        }
        let version =
            state
                .observer
                .as_ref()
                .unwrap()
                .query_row("PRAGMA data_version", [], |row| row.get(0))?;
        if state.version != version {
            state.version = version;
            state.generation = state.generation.wrapping_add(1);
            state.values.clear();
        }
        Ok(state.generation)
    }

    pub(super) fn get(&self, key: &CountKey, revision: u64) -> Option<u64> {
        let mut state = self.state.lock().unwrap();
        if state.generation != revision {
            return None;
        }
        let position = state.values.iter().position(|(stored, _)| stored == key)?;
        let value = state.values.remove(position).unwrap();
        let count = value.1;
        state.values.push_front(value);
        state.stats.hits += 1;
        Some(count)
    }

    pub(super) fn put(&self, key: CountKey, revision: u64, count: u64) {
        let mut state = self.state.lock().unwrap();
        if state.generation != revision {
            return;
        }
        state.values.retain(|(stored, _)| stored != &key);
        state.values.push_front((key, count));
        state.values.truncate(64);
    }

    pub(super) fn record_count(&self) {
        self.state.lock().unwrap().stats.sql_counts += 1;
    }
}
