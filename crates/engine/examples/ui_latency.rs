//! Read-only timings for operations used while navigating the desktop UI.
//! Never prints file names, settings, application names or credentials.
use cleaner_domain::EntryQuery;
use cleaner_engine::{rules::RuleSet, safety::SafetyPolicy, store::Store};
use cleaner_platform::{filesystem, inventory};
use std::time::Instant;

fn timed<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let value = f();
    println!("{name}: {:.3} ms", start.elapsed().as_secs_f64() * 1000.0);
    value
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("existing database path required"))?;
    anyhow::ensure!(
        std::path::Path::new(&path).is_file(),
        "existing database required"
    );
    let store = Store { path: path.into() };
    for pass in 1..=3 {
        println!("pass {pass}");
        let volumes = timed("volume enumeration", filesystem::volumes);
        println!("volumes: {}", volumes.len());
        timed("access policy", inventory::last_access_policy);
        let scans = timed("scan summaries", || store.scans())?;
        let Some(scan) = scans.iter().find(|s| s.status == "complete") else {
            return Ok(());
        };
        let settings = timed("settings read", || store.settings())?;
        let rules = timed("rule compilation", || {
            RuleSet::load(settings.community_enabled)
        })?;
        let policy = timed("safety context", || SafetyPolicy::new(settings));
        let apps = timed("snapshot app list", || store.apps(&scan.id))?;
        println!("snapshot: {} files, {} apps", scan.files, apps.len());
        for (name, parent, suggestions) in [
            ("directory page", Some(scan.root.clone()), false),
            ("suggestions page", None, true),
        ] {
            let mut page = timed(name, || {
                store.query(&EntryQuery {
                    scan_id: scan.id.clone(),
                    parent,
                    suggestions,
                    limit: 100,
                    sort: Some("size".into()),
                    ..Default::default()
                })
            })?;
            timed("reclassify 100 rows", || {
                for file in &mut page.items {
                    file.assessment = rules.classify(file, &policy, &apps);
                }
            });
            timed("serialize page", || serde_json::to_vec(&page))?;
        }
    }
    Ok(())
}
