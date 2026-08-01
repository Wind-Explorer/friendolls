mod cursor;
mod db;
mod friends;
mod ufa;
mod windowing;

async fn launch_app(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();
    db::init(handle).await?;
    ufa::init();
    cursor::init(handle);
    windowing::init(handle);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = tauri_specta::Builder::<tauri::Wry>::new()
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
        .commands(tauri_specta::collect_commands![
            friends::create_friend,
            friends::list_friends,
            friends::get_friend,
            friends::delete_friend
        ])
        .events(tauri_specta::collect_events![friends::FriendsChanged]);

    #[cfg(debug_assertions)]
    specta_builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/lib/bindings.ts",
        )
        .expect("Failed to export TypeScript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            specta_builder.mount_events(app);
            tauri::async_runtime::block_on(launch_app(app))?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_, _| ());
}
