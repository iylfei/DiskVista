use cleaner_domain::{AnalysisContext, FileRecord, LlmSettings};
use cleaner_llm::{
    batch_metadata_bytes, validate_batch_contexts, MAX_BATCH_ITEMS, MAX_BATCH_METADATA_BYTES,
};
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, VecDeque},
};

pub(super) struct CandidateQueue {
    directories: Vec<VecDeque<(usize, FileRecord)>>,
    heads: BinaryHeap<Reverse<(usize, usize)>>,
}

impl CandidateQueue {
    pub fn new(files: VecDeque<FileRecord>) -> Self {
        let mut directories: Vec<VecDeque<(usize, FileRecord)>> = Vec::new();
        let mut ids = HashMap::new();
        for (rank, file) in files.into_iter().enumerate() {
            let id = *ids.entry(parent(&file)).or_insert_with(|| {
                directories.push(VecDeque::new());
                directories.len() - 1
            });
            directories[id].push_back((rank, file));
        }
        let heads = directories
            .iter()
            .enumerate()
            .filter_map(|(id, files)| files.front().map(|(rank, _)| Reverse((*rank, id))))
            .collect();
        Self { directories, heads }
    }

    pub fn is_empty(&self) -> bool {
        self.heads.is_empty()
    }
}

pub(crate) fn item_limit(settings: &LlmSettings) -> usize {
    if settings.token_parameter == "none" {
        MAX_BATCH_ITEMS
    } else {
        ((settings.max_output_tokens.saturating_sub(128) / 512) as usize).clamp(1, MAX_BATCH_ITEMS)
    }
}

pub(super) fn take(queue: &mut CandidateQueue, limit: usize) -> Vec<FileRecord> {
    let mut batch = Vec::new();
    while batch.len() < limit {
        let Some(Reverse((_, directory))) = queue.heads.pop() else {
            break;
        };
        let files = &mut queue.directories[directory];
        while batch.len() < limit {
            let Some((_, file)) = files.pop_front() else {
                break;
            };
            batch.push(file);
        }
        if let Some((rank, _)) = files.front() {
            queue.heads.push(Reverse((*rank, directory)));
        }
    }
    batch
}

fn parent(file: &FileRecord) -> String {
    cleaner_platform::normalize(if file.parent.is_empty() {
        file.path
            .rsplit_once(['\\', '/'])
            .map_or("", |(parent, _)| parent)
    } else {
        &file.parent
    })
}

pub(super) fn pack(contexts: Vec<AnalysisContext>) -> Vec<Result<Vec<AnalysisContext>, String>> {
    let mut batches = Vec::new();
    let mut current = Vec::new();
    for context in contexts {
        if let Err(error) = validate_batch_contexts(std::slice::from_ref(&context)) {
            batches.push(Err(error.to_string()));
            continue;
        }
        current.push(context);
        if current.len() > MAX_BATCH_ITEMS
            || !batch_metadata_bytes(&current).is_ok_and(|size| size <= MAX_BATCH_METADATA_BYTES)
        {
            let next = current.pop().expect("new context was added");
            if !current.is_empty() {
                batches.push(Ok(std::mem::take(&mut current)));
            }
            current.push(next);
        }
    }
    if !current.is_empty() {
        batches.push(Ok(current));
    }
    batches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn largest_file_starts_each_batch_and_directory_neighbors_travel_together() {
        let mut queue = CandidateQueue::new(VecDeque::from(
            [
                (1, "D:\\A\\one.bin"),
                (2, "D:\\B\\two.bin"),
                (3, "D:\\A\\three.bin"),
                (4, "D:\\C\\four.bin"),
            ]
            .map(|(id, path)| FileRecord {
                id,
                path: path.into(),
                ..Default::default()
            }),
        ));
        assert_eq!(
            take(&mut queue, 3)
                .iter()
                .map(|file| file.id)
                .collect::<Vec<_>>(),
            [1, 3, 2]
        );
        assert_eq!(take(&mut queue, 3)[0].id, 4);
        assert!(queue.is_empty());
    }

    #[test]
    fn small_output_limits_reduce_batch_size_without_changing_user_settings() {
        let mut settings = LlmSettings::default();
        assert_eq!(item_limit(&settings), 3);
        settings.max_output_tokens = 8192;
        assert_eq!(item_limit(&settings), 15);
        settings.max_output_tokens = 16384;
        assert_eq!(item_limit(&settings), 20);
        settings.max_output_tokens = 1;
        assert_eq!(item_limit(&settings), 1);
        settings.token_parameter = "none".into();
        assert_eq!(item_limit(&settings), 20);
    }

    #[test]
    fn large_metadata_splits_without_losing_or_duplicating_files() {
        let contexts: Vec<AnalysisContext> = (1..=3)
            .map(|id| AnalysisContext {
                scan_id: "s".into(),
                entry_id: id,
                fingerprint: id.to_string(),
                path: format!("D:\\Fixture\\{id}.bin"),
                logical_bytes: 200_000_000,
                file_count: 1,
                modified: 0,
                accessed: 0,
                evidence: vec![],
                files: vec![],
                history_references: vec![],
                truncated: false,
                note: format!("{id}{}", "x".repeat(30_000)),
            })
            .collect();
        let batches = pack(contexts)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(batches.iter().map(Vec::len).collect::<Vec<_>>(), [2, 1]);
        assert_eq!(
            batches
                .iter()
                .flatten()
                .map(|context| context.entry_id)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert!(batches
            .iter()
            .all(|batch| validate_batch_contexts(batch).is_ok()));
    }
}
