use crate::state::{error, Shared};

// Called with the mutations lock and analysis lease, before allocating new budgets.
pub(crate) fn prepare(state: &Shared, scan: &str) -> Result<(), String> {
    state.store.restart_analysis(scan).map_err(error)?;
    let mut budgets = state.budgets.lock().unwrap();
    budgets.remove(scan);
    state.context_previews.lock().unwrap().clear();
    state.sample_previews.lock().unwrap().clear();
    Ok(())
}
