use super::*;
use cleaner_engine::analysis_queue::CandidateQueue;

pub(super) struct Prepared {
    pub queue: CandidateQueue,
    pub cached: u32,
    pub queued: u32,
}

pub(super) fn collect(
    state: &Shared,
    scan: &Scan,
    settings: &Settings,
    budget: &Budget,
) -> Result<Prepared, String> {
    let check = || {
        if budget.cancel.load(Ordering::Relaxed) {
            Err("AI 分析已取消".to_owned())
        } else {
            Ok(())
        }
    };
    check()?;
    let minimum_bytes = settings.llm.minimum_bytes.max(MINIMUM_BATCH_BYTES);
    let policy = SafetyPolicy::new(settings.clone());
    let (_, apps) = state.application_index(&scan.id, &policy)?;
    let rules = RuleSet::load(settings.community_enabled).map_err(error)?;
    let classifier = Classifier::new(scan, &rules, &policy, &apps);
    let recycled = RecycledTargets::load(&state.store, scan).map_err(error)?;
    let mut result = Prepared {
        queue: CandidateQueue::new().map_err(error)?,
        cached: 0,
        queued: 0,
    };
    let mut after = None;
    loop {
        check()?;
        let mut page = state
            .store
            .analysis_candidate_page(&scan.id, minimum_bytes, after, budget.cancel.clone())
            .map_err(error)?;
        if page.is_empty() {
            break;
        }
        after = page.last().map(|file| (file.logical_bytes, file.id));
        for file in &mut page {
            check()?;
            classifier.apply(file);
        }
        page.retain(|file| {
            !recycled.contains(&file.path) && eligible(file, &policy, &apps, minimum_bytes)
        });
        result.queued = result.queued.saturating_add(page.len() as u32);
        // Revalidation is bounded separately so cancellation is checked between small groups.
        for chunk in page.chunks(20) {
            check()?;
            let ids: Vec<_> = chunk.iter().map(|file| file.id).collect();
            let reusable = crate::analysis_results::reusable_ids(state, &scan.id, &ids)?;
            for file in chunk {
                check()?;
                if reusable.contains(&file.id) {
                    result.cached = result.cached.saturating_add(1);
                } else {
                    result.queue.push(file).map_err(error)?;
                }
            }
        }
        let mut progress = state.progress.lock().unwrap();
        progress.queued = result.queued;
        progress.finished = result.cached;
    }
    check()?;
    result.queue.finish().map_err(error)?;
    Ok(result)
}
