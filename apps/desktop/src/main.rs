#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod ai;
mod ai_batch;
mod analysis_results;
mod background;
mod cleanup_checks;
mod commands;
mod locations;
mod mutations;
mod operations;
mod scan_analysis;
mod scans;
mod settings_store;
mod snapshot_cache;
mod space_map;
mod state;
mod suggestions;
mod window;
use tauri::Manager;
fn main() {
    let Ok(Some(_instance)) = cleaner_platform::process::single_instance() else {
        return;
    };
    tauri::Builder::default()
        .setup(|app| {
            let root = app.path().app_local_data_dir()?;
            let store = cleaner_engine::store::Store::open(root.join("index.sqlite"))?;
            app.manage(state::AppState::new(store));
            if let Some(window) = app.get_webview_window("main") {
                // A monitor-query failure must not prevent opening the local application.
                let _ = window::fit_startup(&window);
                window.show()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::runtime_status,
            commands::choose_folder,
            commands::start_scan,
            commands::cancel_scan,
            commands::get_scan,
            scans::delete_scan,
            commands::query_entries,
            suggestions::cleanup_suggestions,
            suggestions::select_suggestion_group,
            commands::application_units,
            commands::space_map,
            commands::entry_detail,
            mutations::save_settings,
            mutations::set_annotation,
            commands::rules,
            operations::preview_cleanup,
            operations::cleanup_check_progress,
            operations::cancel_cleanup_check,
            operations::execute_cleanup,
            operations::cancel_cleanup,
            operations::history_page,
            operations::open_location,
            operations::open_system,
            ai::llm_context,
            ai::preview_samples,
            ai::analyze,
            scan_analysis::analyze_scan,
            ai::cancel_analysis,
            analysis_results::analysis_results,
            analysis_results::analysis_summaries,
            ai::test_connection
        ])
        .run(tauri::generate_context!())
        .expect("无法启动 DiskVista");
}
