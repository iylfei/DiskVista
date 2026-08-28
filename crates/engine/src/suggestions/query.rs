use super::*;
use crate::recycled_targets::RecycledTargets;
use anyhow::bail;

#[derive(PartialEq, Eq)]
struct Key {
    search: String,
    risk: String,
    group: Option<String>,
    sort: String,
    analysis: Option<AnalysisFilter>,
    recycled: RecycledTargets,
}
pub(super) struct View {
    key: Key,
    groups: Vec<SuggestionGroup>,
    indices: Vec<usize>,
}

fn matches(index: &SuggestionIndex, file: &Candidate, key: &Key) -> bool {
    if key.recycled.contains(&file.path)
        || (file.is_dir && key.recycled.affects_directory(&file.path))
    {
        return false;
    }
    let a = &index.assessments[file.assessment];
    let allowed = !matches!(a.risk.as_str(), "protected" | "keep");
    let risk = match key.risk.as_str() {
        "protected" => a.risk == "protected",
        "low" => a.risk == "low",
        "unknown" => allowed && a.rule_id.is_none() && a.owner.is_none(),
        "known" => allowed && (a.rule_id.is_some() || a.owner.is_some()),
        _ => allowed,
    };
    risk && key
        .analysis
        .as_ref()
        .is_none_or(|filter| filter.matches(file.id))
        && (key.search.is_empty() || file.key.contains(&key.search))
}

fn build_view(index: &SuggestionIndex, key: Key) -> View {
    let filtered: Vec<_> = index
        .entries
        .iter()
        .enumerate()
        .filter(|(_, f)| matches(index, f, &key))
        .collect();
    let directories: HashMap<_, _> = filtered
        .iter()
        .filter(|(_, f)| f.is_dir)
        .map(|(_, f)| (f.key.as_str(), f.group))
        .collect();
    let candidates: Vec<_> = filtered
        .into_iter()
        .filter(|(_, file)| {
            !file
                .key
                .match_indices('\\')
                .any(|(end, _)| directories.get(&file.key[..end]) == Some(&file.group))
        })
        .collect();
    let mut groups = index.groups.clone();
    for (_, f) in &candidates {
        let group = &mut groups[f.group];
        group.count += 1;
        group.occupied_bytes = group.occupied_bytes.saturating_add(f.occupied);
        group.estimated |= f.estimated;
    }
    groups.retain(|g| g.count > 0);
    groups.sort_by(|a, b| {
        (b.id == "large-files")
            .cmp(&(a.id == "large-files"))
            .then(b.recognized.cmp(&a.recognized))
            .then(b.occupied_bytes.cmp(&a.occupied_bytes))
            .then(a.name.cmp(&b.name))
    });
    let mut indices: Vec<_> = candidates
        .into_iter()
        .filter(|(_, f)| {
            key.group
                .as_ref()
                .is_none_or(|id| index.groups[f.group].id == *id)
        })
        .map(|(i, _)| i)
        .collect();
    indices.sort_by(|&a, &b| {
        let (a, b) = (&index.entries[a], &index.entries[b]);
        let order = match key.sort.as_str() {
            "name" => a.path.cmp(&b.path),
            "activity" | "activity_desc" => b.latest_change.cmp(&a.latest_change),
            "activity_asc" => a.latest_change.cmp(&b.latest_change),
            _ => b.occupied.cmp(&a.occupied),
        };
        order.then(a.id.cmp(&b.id))
    });
    View {
        key,
        indices,
        groups,
    }
}

fn with_view<T>(
    index: &SuggestionIndex,
    query: &SuggestionQuery,
    analysis: Option<&AnalysisFilter>,
    read: impl FnOnce(&View) -> T,
) -> Result<T> {
    let scan = index.store.require_finished(&index.scan_id)?;
    let key = Key {
        search: normalize(&query.search),
        risk: query.risk.clone(),
        group: query.group.clone(),
        sort: query.sort.clone(),
        analysis: analysis.cloned(),
        recycled: RecycledTargets::load(&index.store, &scan)?,
    };
    let mut cache = index.views.lock().unwrap();
    if let Some(position) = cache.iter().position(|v| v.key == key) {
        let view = cache.remove(position).unwrap();
        cache.push_front(view);
    } else {
        if cache.len() == 4 {
            cache.pop_back();
        }
        cache.push_front(build_view(index, key));
    }
    Ok(read(cache.front().unwrap()))
}

pub fn page(index: &SuggestionIndex, query: &SuggestionQuery) -> Result<SuggestionPage> {
    page_with_analysis(index, query, None)
}

pub fn page_with_analysis(
    index: &SuggestionIndex,
    query: &SuggestionQuery,
    analysis: Option<&AnalysisFilter>,
) -> Result<SuggestionPage> {
    AnalysisFilter::validate(&query.analysis_status, analysis)?;
    let (groups, total, selected) = with_view(index, query, analysis, |view| {
        (
            view.groups.clone(),
            view.indices.len(),
            view.indices
                .iter()
                .skip(query.offset)
                .take(query.limit.clamp(1, 100))
                .copied()
                .collect::<Vec<_>>(),
        )
    })?;
    Ok(SuggestionPage {
        groups,
        total,
        items: index.load(&selected)?,
    })
}

pub fn selection(index: &SuggestionIndex, query: &SuggestionQuery) -> Result<Vec<FileRecord>> {
    selection_with_analysis(index, query, None)
}

pub fn selection_with_analysis(
    index: &SuggestionIndex,
    query: &SuggestionQuery,
    analysis: Option<&AnalysisFilter>,
) -> Result<Vec<FileRecord>> {
    AnalysisFilter::validate(&query.analysis_status, analysis)?;
    if !query
        .group
        .as_ref()
        .is_some_and(|id| id.starts_with("rule:"))
    {
        bail!("请逐项选择用途未识别的大文件");
    }
    let selected: Vec<_> = with_view(index, query, analysis, |view| {
        view.indices
            .iter()
            .copied()
            .filter(|&i| index.assessments[index.entries[i].assessment].risk != "protected")
            .take(501)
            .collect()
    })?;
    if selected.len() > 500 {
        bail!("这一类超过 500 项，请先缩小搜索范围或按页选择");
    }
    index.load(&selected)
}
