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
    if std::env::args().nth(2).as_deref() == Some("--analysis") {
        let scans = timed("scan summaries", || store.scans())?;
        let scan = scans
            .iter()
            .find(|scan| scan.status == "complete")
            .ok_or_else(|| anyhow::anyhow!("no finished scan"))?;
        println!("snapshot: {} files", scan.files);
        let settings = timed("analysis settings", || store.settings())?;
        timed("analysis safety policy", || {
            SafetyPolicy::new(settings.clone())
        });
        timed("analysis app snapshot", || store.apps(&scan.id))?;
        let candidates = timed("analysis candidates", || {
            store.query(&EntryQuery {
                scan_id: scan.id.clone(),
                uncertain_only: true,
                minimum_bytes: settings.llm.minimum_bytes,
                limit: 5,
                ..Default::default()
            })
        })?;
        for (index, file) in candidates.items.iter().enumerate() {
            match timed(&format!("snapshot context {}", index + 1), || {
                cleaner_engine::context::build(&store, &scan.id, file.id)
            }) {
                Ok(context) => println!(
                    "metadata rows: {}, truncated: {}",
                    context.files.len(),
                    context.truncated
                ),
                Err(_) => println!("candidate excluded by safety policy"),
            }
        }
        return Ok(());
    }
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
        let apps = timed("application path index", || {
            cleaner_engine::application_index::ApplicationIndex::new(&apps, &policy)
        });
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
                    file.assessment = rules.classify_indexed(file, &policy, &apps);
                }
            });
            timed("serialize page", || serde_json::to_vec(&page))?;
        }
    }
    Ok(())
}
