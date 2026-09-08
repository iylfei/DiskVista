use super::Store;
use anyhow::Result;
use rusqlite::Connection;
use std::sync::Mutex;

/// data_version is comparable only on the same connection. This observer never writes.
pub struct RevisionObserver {
    store: Store,
    connection: Mutex<Option<Connection>>,
}

impl RevisionObserver {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            connection: Mutex::new(None),
        }
    }

    pub fn revision(&self) -> Result<i64> {
        let mut connection = self.connection.lock().unwrap();
        if connection.is_none() {
            *connection = Some(self.store.connection()?);
        }
        Ok(connection
            .as_ref()
            .unwrap()
            .query_row("PRAGMA data_version", [], |row| row.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separate_connections_invalidate_but_reads_do_not() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("revision.sqlite")).unwrap();
        let observer = RevisionObserver::new(store.clone());
        let first = observer.revision().unwrap();
        assert_eq!(observer.revision().unwrap(), first);
        store.put("changed", &1).unwrap();
        let next = observer.revision().unwrap();
        assert_ne!(first, next);
        store.get::<i32>("changed").unwrap();
        assert_eq!(next, observer.revision().unwrap());
    }
}
