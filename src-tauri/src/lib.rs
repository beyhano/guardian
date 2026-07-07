mod process_manager;

use tauri::Manager;

#[tauri::command]
async fn get_processes(
    manager: tauri::State<'_, process_manager::ProcessManager>,
) -> Result<Vec<process_manager::ProcessInfo>, String> {
    Ok(manager.get_processes().await)
}

#[tauri::command]
async fn start_process(
    id: String,
    manager: tauri::State<'_, process_manager::ProcessManager>,
) -> Result<(), String> {
    manager.start_process(&id).await
}

#[tauri::command]
async fn stop_process(
    id: String,
    manager: tauri::State<'_, process_manager::ProcessManager>,
) -> Result<(), String> {
    manager.stop_process(&id).await
}

#[tauri::command]
async fn add_process(
    config: process_manager::ProcessConfig,
    manager: tauri::State<'_, process_manager::ProcessManager>,
) -> Result<(), String> {
    manager.add_process(config).await
}

#[tauri::command]
async fn remove_process(
    id: String,
    manager: tauri::State<'_, process_manager::ProcessManager>,
) -> Result<(), String> {
    manager.remove_process(&id).await
}

#[tauri::command]
async fn get_process_logs(
    id: String,
    max_lines: usize,
    manager: tauri::State<'_, process_manager::ProcessManager>,
) -> Result<Vec<String>, String> {
    manager.get_process_logs(&id, max_lines)
}

#[tauri::command]
async fn update_process(
    id: String,
    config: process_manager::ProcessConfig,
    manager: tauri::State<'_, process_manager::ProcessManager>,
) -> Result<(), String> {
    manager.update_process(&id, config).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let manager = process_manager::ProcessManager::new(app_handle);
            
            // Load configuration files and auto-start active processes
            let manager_clone = manager.clone();
            tauri::async_runtime::block_on(async move {
                let _ = manager_clone.load_configs().await;
                let _ = manager_clone.auto_start_processes().await;
            });
            
            app.manage(manager);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_processes,
            start_process,
            stop_process,
            add_process,
            remove_process,
            get_process_logs,
            update_process
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

