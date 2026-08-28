use cleaner_domain::ApplicationUnit;
pub(super) fn nest(mut units: Vec<ApplicationUnit>, _root: &str) -> Vec<ApplicationUnit> {
    // Boundaries already contain residual bytes. Keeping independent applications
    // at the top level must not add their bytes back to physical parent containers.
    sort(&mut units);
    units
}

fn sort(units: &mut [ApplicationUnit]) {
    units.sort_by(|a, b| {
        (a.kind == "unassigned")
            .cmp(&(b.kind == "unassigned"))
            .then(
                b.occupied_bytes
                    .cmp(&a.occupied_bytes)
                    .then(a.name.cmp(&b.name)),
            )
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
