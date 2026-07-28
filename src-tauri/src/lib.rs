mod cursor;
mod ufa;

fn launch_app(app: &tauri::App) {
    let handle = app.handle();
    crate::ufa::init();
    crate::cursor::init(handle);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![])
        .setup(|app| {
            launch_app(app);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_, _| ());
}
