//! Keep synchronous filesystem/SQLite work off both the window thread and async workers.
pub async fn read<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(crate::state::error)?
}

#[cfg(test)]
mod tests {
    #[test]
    fn read_does_not_run_on_the_calling_thread() {
        let caller = std::thread::current().id();
        let worker =
            tauri::async_runtime::block_on(super::read(|| Ok(std::thread::current().id())))
                .unwrap();
        assert_ne!(caller, worker);
    }

    #[test]
    fn read_preserves_errors() {
        let result =
            tauri::async_runtime::block_on(super::read(|| Err::<(), _>("read failed".into())));
        assert_eq!(result.unwrap_err(), "read failed");
    }
}
