use crate::{
    application_index::ApplicationIndex, rules::RuleSet, safety::SafetyPolicy, store::Store,
};
use anyhow::{anyhow, Result};
use cleaner_domain::{FileRecord, Scan};
use cleaner_platform::{filesystem, within};
use rusqlite::Connection;
use std::{
    collections::HashSet,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex,
    },
    time::{Duration, Instant},
};

#[allow(clippy::large_enum_variant)]
enum Event {
    Entry(FileRecord),
    Done(i64, Option<String>),
}

const EVENT_BATCH: usize = 512;
const WRITE_BATCH: usize = 2048;

pub(super) struct Walker<'a> {
    pub store: &'a Store,
    pub rules: &'a RuleSet,
    pub policy: &'a SafetyPolicy,
    pub apps: &'a ApplicationIndex,
    pub cancel: &'a AtomicBool,
    pub own_data: Option<String>,
}

impl Walker<'_> {
    pub fn run(&self, scan: &mut Scan, progress: &mut impl FnMut(&Scan)) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let concurrency = filesystem::volumes()
            .iter()
            .find(|v| within(&scan.root, &v.path))
            .map(|v| if v.removable { 1 } else { 4 })
            .unwrap_or(1);
        let (tasks, work) = mpsc::sync_channel::<FileRecord>(concurrency);
        let work = Mutex::new(work);
        let (events, results) = mpsc::sync_channel(EVENT_BATCH);
        std::thread::scope(|scope| {
            for _ in 0..concurrency {
                let events = events.clone();
                let work = &work;
                scope.spawn(move || loop {
                    let Ok(directory) = work.lock().unwrap().recv() else {
                        break;
                    };
                    if self.cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let result = filesystem::enumerate(Path::new(&directory.path), |mut file| {
                        if self.cancel.load(Ordering::Relaxed) {
                            return Ok(false);
                        }
                        // Directory-entry timestamps can lag handle timestamps after child creation.
                        if file.is_dir
                            && file.attributes
                                & (filesystem::REPARSE | filesystem::OFFLINE | filesystem::RECALL)
                                == 0
                        {
                            match filesystem::inspect(Path::new(&file.path)) {
                                Ok(fresh) if fresh.identity == file.identity => file = fresh,
                                _ => {
                                    file.issue = Some("目录身份变化或元数据无法读取".into());
                                    file.enumerated = true;
                                    file.complete = false;
                                }
                            }
                        }
                        Ok(events.send(Event::Entry(file)).is_ok())
                    });
                    let error = if self.cancel.load(Ordering::Relaxed) {
                        Some("扫描已取消，目录不完整".into())
                    } else {
                        result.err().map(|error| format!("{error:#}"))
                    };
                    if events.send(Event::Done(directory.id, error)).is_err() {
                        break;
                    }
                });
            }
            drop(events);
            let result = self.consume(scan, progress, concurrency, &tasks, &results);
            // Both channels must close before scoped joins, including on write errors/cancellation.
            drop(tasks);
            drop(results);
            if self.cancel.load(Ordering::Relaxed) {
                Ok(())
            } else {
                result
            }
        })
    }

    fn consume(
        &self,
        scan: &mut Scan,
        progress: &mut impl FnMut(&Scan),
        concurrency: usize,
        tasks: &mpsc::SyncSender<FileRecord>,
        results: &mpsc::Receiver<Event>,
    ) -> Result<()> {
        let mut connection = self.store.connection()?;
        // Give this scan's single writer room for index pages without expanding every UI connection.
        connection.pragma_update(None, "cache_size", -8192)?;
        let mut running = HashSet::new();
        let mut batch = Vec::with_capacity(WRITE_BATCH);
        let mut finished = Vec::new();
        let mut last = Instant::now();
        fill(&connection, &scan.id, concurrency, tasks, &mut running)?;
        while !running.is_empty() {
            if self.cancel.load(Ordering::Relaxed) {
                break;
            }
            let first = match results.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => Some(event),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(_) => return Err(anyhow!("扫描线程意外结束")),
            };
            // Bound both the pending write buffer and work between cancellation checks.
            let drain = EVENT_BATCH.min(WRITE_BATCH - batch.len());
            for event in first.into_iter().chain(results.try_iter().take(drain - 1)) {
                match event {
                    Event::Entry(mut file) => {
                        file.assessment =
                            self.rules.classify_indexed(&file, self.policy, self.apps);
                        if file.is_dir {
                            scan.directories += 1;
                            file.complete = false;
                        } else {
                            scan.files += 1;
                            scan.logical_bytes =
                                scan.logical_bytes.saturating_add(file.logical_bytes);
                            scan.allocated_bytes = scan
                                .allocated_bytes
                                .saturating_add(file.allocated_bytes.unwrap_or(0));
                        }
                        let own_data = self
                            .own_data
                            .as_ref()
                            .is_some_and(|p| within(&file.path, p));
                        if file.attributes
                            & (filesystem::REPARSE | filesystem::OFFLINE | filesystem::RECALL)
                            != 0
                            || own_data
                        {
                            file.enumerated = true;
                            file.complete = false;
                            file.issue = Some(
                                if own_data {
                                    "应用自身索引目录不展开"
                                } else {
                                    "链接、挂载点或云占位项不展开"
                                }
                                .into(),
                            );
                        }
                        scan.issues += u64::from(file.issue.is_some());
                        batch.push(file);
                    }
                    Event::Done(id, error) => finished.push((id, error)),
                }
            }
            if batch.len() >= WRITE_BATCH
                || !finished.is_empty()
                || last.elapsed() >= Duration::from_millis(250)
            {
                if !batch.is_empty() || !finished.is_empty() {
                    Store::insert_scan_batch(&mut connection, &scan.id, &batch, &finished)?;
                    batch.clear();
                    for (id, _) in finished.drain(..) {
                        running.remove(&id);
                    }
                }
                if !self.cancel.load(Ordering::Relaxed) {
                    fill(&connection, &scan.id, concurrency, tasks, &mut running)?;
                }
            }
            if last.elapsed() >= Duration::from_millis(250) {
                self.store.save_scan(scan)?;
                progress(scan);
                last = Instant::now();
            }
        }
        Ok(())
    }
}

fn fill(
    connection: &Connection,
    scan: &str,
    concurrency: usize,
    tasks: &mpsc::SyncSender<FileRecord>,
    running: &mut HashSet<i64>,
) -> Result<()> {
    if running.len() >= concurrency {
        return Ok(());
    }
    // At most `concurrency` pending rows are already running; read just enough to fill idle slots.
    for directory in Store::pending_from(connection, scan, concurrency * 2)? {
        if running.insert(directory.id) {
            tasks
                .send(directory)
                .map_err(|_| anyhow!("扫描线程已停止"))?;
            if running.len() == concurrency {
                break;
            }
        }
    }
    Ok(())
}
