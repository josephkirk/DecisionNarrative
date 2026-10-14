pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::evaluate_tension_arc_rust,
            commands::compute_salience_rust
        ])
        .run(tauri::generate_context!())
        .expect("error while building KINESIS tauri desktop application");
}
