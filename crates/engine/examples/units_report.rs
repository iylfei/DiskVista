//! Read-only inspection of an existing snapshot; never starts a scan or a cleanup.
use cleaner_engine::{store::Store, units};
fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("database path required"))?;
    anyhow::ensure!(
        std::path::Path::new(&path).is_file(),
        "existing database required"
    );
    let store = Store { path: path.into() };
    let selected_scan = std::env::args().nth(2);
    let scan = store
        .scans()?
        .into_iter()
        .find(|s| s.status == "complete" && selected_scan.as_ref().is_none_or(|id| id == &s.id))
        .ok_or_else(|| anyhow::anyhow!("no completed snapshot"))?;
    let started = std::time::Instant::now();
    let units = units::build(&store, &scan.id)?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"scanId":scan.id,"scanStatus":scan.status,"scanFiles":scan.files,"scanRoot":scan.root,"elapsedMs":started.elapsed().as_millis(),"units":units.len(),"sumLogical":units.iter().map(|u|u.logical_bytes).sum::<u64>(),"sumOccupied":units.iter().map(|u|u.occupied_bytes).sum::<u64>(),"scanLogical":scan.logical_bytes,"scanOccupied":scan.allocated_bytes,"top":units.iter().take(12).collect::<Vec<_>>()})
        )?
    );
    Ok(())
}
