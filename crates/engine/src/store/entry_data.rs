use cleaner_domain::{Assessment, FileRecord};
use serde::{ser::SerializeStruct, Serialize, Serializer};

pub(super) struct Metadata<'a>(pub &'a FileRecord);

impl Serialize for Metadata<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let file = self.0;
        let mut data = serializer.serialize_struct("FileRecord", 21)?;
        macro_rules! field {
            ($name:literal, $field:ident) => {
                data.serialize_field($name, &file.$field)?
            };
        }
        field!("id", id);
        field!("path", path);
        field!("parent", parent);
        field!("name", name);
        field!("isDir", is_dir);
        field!("logicalBytes", logical_bytes);
        field!("allocatedBytes", allocated_bytes);
        field!("modified", modified);
        field!("modifiedTicks", modified_ticks);
        field!("latestChange", latest_change);
        field!("accessed", accessed);
        field!("created", created);
        field!("identity", identity);
        field!("attributes", attributes);
        field!("links", links);
        field!("fileCount", file_count);
        field!("issue", issue);
        field!("enumerated", enumerated);
        field!("complete", complete);
        field!("hasBlockedChildren", has_blocked_children);
        // The authoritative assessment has its own column; keep the empty shape for older readers.
        data.serialize_field("assessment", &Assessment::default())?;
        data.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    #[test]
    fn compact_and_legacy_rows_return_the_same_complete_record() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("records.sqlite")).unwrap();
        let file = FileRecord {
            path: "D:\\fixture\\file.bin".into(),
            parent: "D:\\fixture".into(),
            name: "file.bin".into(),
            logical_bytes: 42,
            allocated_bytes: Some(4096),
            identity: Some("id".into()),
            modified: 12,
            modified_ticks: 12345,
            latest_change: 12,
            accessed: 10,
            created: 1,
            attributes: 32,
            links: 2,
            file_count: 1,
            enumerated: true,
            complete: true,
            assessment: Assessment {
                purpose: "说明".repeat(500),
                risk: "review".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let legacy = serde_json::to_string(&file).unwrap();
        let compact = serde_json::to_string(&Metadata(&file)).unwrap();
        assert!(compact.len() + 2000 < legacy.len());
        Store::insert_batch(
            &mut store.connection().unwrap(),
            "s",
            std::slice::from_ref(&file),
        )
        .unwrap();
        let actual = store.by_path("s", &file.path).unwrap();
        let mut expected = file.clone();
        expected.id = actual.id;
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        store
            .connection()
            .unwrap()
            .execute(
                "UPDATE entries SET data=?1 WHERE id=?2",
                rusqlite::params![legacy, actual.id],
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(store.entry("s", actual.id).unwrap()).unwrap(),
            serde_json::to_value(actual).unwrap()
        );
    }
}
