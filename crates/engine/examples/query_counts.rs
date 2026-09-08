//! Read-only directory-page comparison using an existing scan. No filesystem scan or cleanup.
use anyhow::Result;
use cleaner_domain::EntryQuery;
use cleaner_engine::store::{CountCache, Store};
use std::time::Instant;

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    anyhow::ensure!(args.len() == 2, "usage: query_counts DATABASE SCAN_ID");
    let store = Store {
        path: args[0].clone().into(),
    };
    let connection = store.connection()?;
    let mut statement = connection.prepare("SELECT parent_key,count(*) AS n FROM entries INDEXED BY entries_parent WHERE scan_id=?1 GROUP BY parent_key ORDER BY n DESC LIMIT 5")?;
    let parents = statement
        .query_map([&args[1]], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut results = Vec::new();
    for (parent, count) in parents {
        let cache = CountCache::default();
        let mut plain = Vec::new();
        let mut cached = Vec::new();
        for sample in 0..21 {
            let query = EntryQuery {
                scan_id: args[1].clone(),
                parent: Some(parent.clone()),
                offset: (sample % 3) * 100,
                limit: 100,
                sort: Some("size".into()),
                ..Default::default()
            };
            // Alternate order to reduce a systematic first-reader cache advantage.
            let mut answers = Vec::new();
            for enabled in if sample % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let started = Instant::now();
                let page = if enabled {
                    store.query_cached(&query, &cache)?
                } else {
                    store.query(&query)?
                };
                let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                if sample > 0 {
                    if enabled {
                        cached.push(elapsed);
                    } else {
                        plain.push(elapsed);
                    }
                }
                answers.push((
                    page.total,
                    page.items.iter().map(|file| file.id).collect::<Vec<_>>(),
                ));
            }
            anyhow::ensure!(answers[0] == answers[1], "cached and uncached pages differ");
        }
        results.push(
            serde_json::json!({"parent":parent,"entries":count,"uncachedMedianMs":median(plain),
            "cachedMedianMs":median(cached),"cache":cache.stats(),"identicalPages":true}),
        );
    }
    println!("{}", serde_json::to_string_pretty(&results)?);
    Ok(())
}
