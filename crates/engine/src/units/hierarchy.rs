use cleaner_domain::ApplicationUnit;
use cleaner_platform::{normalize, within};
use std::collections::HashMap;

pub(super) fn nest(units: Vec<ApplicationUnit>, root: &str) -> Vec<ApplicationUnit> {
    let locations: HashMap<_, _> = units
        .iter()
        .enumerate()
        .flat_map(|(index, unit)| {
            unit.components
                .iter()
                .map(move |c| (normalize(&c.path), index))
        })
        .collect();
    let mut children = vec![vec![]; units.len()];
    let mut roots = vec![];
    for (index, unit) in units.iter().enumerate() {
        let mut owner = None;
        if let Some(first) = unit.components.first() {
            let key = normalize(&first.path);
            let mut next = super::parent(&key);
            while let Some(path) = next {
                if let Some(&candidate) = locations.get(path) {
                    let ancestor = &units[candidate];
                    let scan_remainder = ancestor.kind == "unassigned"
                        && ancestor
                            .components
                            .iter()
                            .any(|c| normalize(&c.path) == root);
                    if candidate != index
                        && !scan_remainder
                        && unit.components.iter().all(|c| {
                            ancestor.components.iter().any(|a| {
                                normalize(&c.path) != normalize(&a.path) && within(&c.path, &a.path)
                            })
                        })
                    {
                        owner = Some(candidate);
                        break;
                    }
                }
                next = super::parent(path);
            }
        }
        if let Some(owner) = owner {
            children[owner].push(index);
        } else {
            roots.push(index);
        }
    }
    fn take(
        index: usize,
        pool: &mut [Option<ApplicationUnit>],
        links: &[Vec<usize>],
    ) -> ApplicationUnit {
        let mut unit = pool[index]
            .take()
            .expect("each unit has one directory parent");
        for &child in &links[index] {
            let child = take(child, pool, links);
            unit.logical_bytes = unit.logical_bytes.saturating_add(child.logical_bytes);
            unit.occupied_bytes = unit.occupied_bytes.saturating_add(child.occupied_bytes);
            unit.file_count = unit.file_count.saturating_add(child.file_count);
            unit.estimated |= child.estimated;
            unit.complete &= child.complete;
            unit.children.push(child);
        }
        sort(&mut unit.children);
        unit
    }
    let mut pool: Vec<_> = units.into_iter().map(Some).collect();
    let mut result: Vec<_> = roots
        .into_iter()
        .map(|i| take(i, &mut pool, &children))
        .collect();
    sort(&mut result);
    result
}

fn sort(units: &mut [ApplicationUnit]) {
    units.sort_by(|a, b| {
        b.occupied_bytes
            .cmp(&a.occupied_bytes)
            .then(a.name.cmp(&b.name))
    });
}

pub(super) fn matches(unit: &ApplicationUnit, query: &str) -> bool {
    query.is_empty()
        || unit.name.to_lowercase().contains(query)
        || unit
            .components
            .iter()
            .any(|c| c.path.to_lowercase().contains(query))
        || unit.children.iter().any(|c| matches(c, query))
}

pub(super) fn counts(units: &[ApplicationUnit]) -> (usize, usize) {
    units.iter().fold((0, 0), |(apps, uncertain), u| {
        let (child_apps, child_uncertain) = counts(&u.children);
        (
            apps + child_apps + usize::from(u.kind == "application"),
            uncertain
                + child_uncertain
                + usize::from(matches!(
                    u.kind.as_str(),
                    "possible_application" | "unassigned"
                )),
        )
    })
}
