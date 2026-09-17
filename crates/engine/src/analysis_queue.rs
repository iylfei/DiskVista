//! A disk-backed queue keeps directory grouping without retaining every FileRecord.
use anyhow::Result;
use cleaner_domain::FileRecord;
use cleaner_platform::normalize;
use rusqlite::{params, Connection, OptionalExtension};

pub struct CandidateQueue {
    connection: Connection,
    remaining: usize,
}

impl CandidateQueue {
    pub fn new() -> Result<Self> {
        let connection = Connection::open("")?;
        connection.execute_batch(
            "PRAGMA cache_size=-2048; PRAGMA temp_store=FILE; PRAGMA mmap_size=0;
            CREATE TABLE queue(rank INTEGER PRIMARY KEY,id INTEGER,parent TEXT);
            CREATE INDEX queue_parent ON queue(parent,rank); BEGIN",
        )?;
        Ok(Self {
            connection,
            remaining: 0,
        })
    }

    /// Insert in descending size order, breaking ties by entry ID.
    pub fn push(&mut self, file: &FileRecord) -> Result<()> {
        let parent = normalize(if file.parent.is_empty() {
            file.path
                .rsplit_once(['\\', '/'])
                .map_or("", |(parent, _)| parent)
        } else {
            &file.parent
        });
        self.connection
            .prepare_cached("INSERT INTO queue(id,parent) VALUES(?1,?2)")?
            .execute(params![file.id, parent])?;
        self.remaining += 1;
        Ok(())
    }

    pub fn finish(&mut self) -> Result<()> {
        self.connection.execute_batch("COMMIT")?;
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.remaining == 0
    }

    pub fn take(&mut self, limit: usize) -> Result<Vec<i64>> {
        let limit = limit.clamp(1, 20);
        let tx = self.connection.transaction()?;
        let mut batch = Vec::new();
        while batch.len() < limit {
            let parent: Option<String> = tx
                .query_row(
                    "SELECT parent FROM queue ORDER BY rank LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(parent) = parent else { break };
            let mut statement = tx.prepare_cached(
                "SELECT rank,id FROM queue WHERE parent=?1 ORDER BY rank LIMIT ?2",
            )?;
            let files = statement
                .query_map(params![parent, (limit - batch.len()) as i64], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let mut remove = tx.prepare_cached("DELETE FROM queue WHERE rank=?1")?;
            for (rank, id) in files {
                remove.execute([rank])?;
                batch.push(id);
            }
        }
        tx.commit()?;
        self.remaining -= batch.len();
        Ok(batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_neighbors_are_grouped_even_when_far_apart_in_size_order() {
        let mut queue = CandidateQueue::new().unwrap();
        for (id, parent) in [(1, "A"), (2, "B"), (3, "A"), (4, "C")] {
            queue
                .push(&FileRecord {
                    id,
                    parent: parent.into(),
                    ..Default::default()
                })
                .unwrap();
        }
        queue.finish().unwrap();
        assert_eq!(queue.take(3).unwrap(), [1, 3, 2]);
        assert_eq!(queue.take(3).unwrap(), [4]);
        assert!(queue.is_empty());
    }
}
