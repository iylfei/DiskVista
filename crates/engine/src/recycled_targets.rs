use crate::store::Store;
use anyhow::Result;
use cleaner_domain::Scan;
use cleaner_platform::normalize;
use std::collections::BTreeSet;

const HISTORY_QUERY: &str = "
    SELECT json_extract(data,'$.path'), json_extract(data,'$.snapshot.isDir')
    FROM history INDEXED BY history_time
    WHERE time>?1 AND json_extract(data,'$.status')='recycled'
    UNION ALL
    SELECT json_extract(data,'$.path'), json_extract(data,'$.snapshot.isDir')
    FROM history INDEXED BY history_recycled_scan
    WHERE time<=?1 AND json_extract(data,'$.scanId')=?2
      AND json_extract(data,'$.status')='recycled'";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecycledTargets {
    paths: BTreeSet<String>,
    directories: BTreeSet<String>,
    affected_directories: BTreeSet<String>,
}

impl RecycledTargets {
    pub fn load(store: &Store, scan: &Scan) -> Result<Self> {
        let completed = scan.finished.unwrap_or(scan.started).max(scan.started);
        let connection = store.connection()?;
        // Legacy history only has second-resolution timestamps. Equality cannot
        // distinguish a later scan of a restored file from an earlier snapshot.
        let mut statement = connection.prepare(HISTORY_QUERY)?;
        let rows = statement.query_map(rusqlite::params![completed, scan.id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<bool>>(1)?))
        })?;
        let mut result = Self::default();
        for row in rows {
            let (path, is_directory) = row?;
            let key = path_key(&path);
            if key.is_empty() {
                continue;
            }
            if is_directory == Some(true) {
                result.directories.insert(key.clone());
            }
            // A changed directory snapshot must not swallow its surviving
            // descendants into an aggregate that will fail cleanup validation.
            for (end, _) in key.match_indices('\\') {
                result.affected_directories.insert(key[..end].to_owned());
            }
            result.paths.insert(key);
        }
        Ok(result)
    }

    pub fn contains(&self, path: &str) -> bool {
        let key = path_key(path);
        self.paths.contains(&key)
            || key
                .match_indices('\\')
                .any(|(end, _)| self.directories.contains(&key[..end]))
    }

    pub fn affects_directory(&self, path: &str) -> bool {
        self.affected_directories.contains(&path_key(path))
    }
}

fn path_key(path: &str) -> String {
    let normalized = normalize(path);
    let extended_unc = path.starts_with(r"\\?\") && normalized.starts_with(r"unc\");
    let is_unc = extended_unc || normalized.starts_with(r"\\");
    let normalized = if extended_unc {
        &normalized[4..]
    } else {
        normalized.as_str()
    };
    if !normalized.contains("\\\\") {
        return if extended_unc {
            format!(r"\\{normalized}")
        } else {
            normalized.into()
        };
    }
    let collapsed = normalized
        .split('\\')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>()
        .join("\\");
    if is_unc {
        format!(r"\\{collapsed}")
    } else if normalized.starts_with('\\') {
        format!(r"\{collapsed}")
    } else {
        collapsed
    }
}

#[cfg(test)]
mod tests;
