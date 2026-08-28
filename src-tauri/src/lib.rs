mod cursor;
mod db;
mod friends;
mod images;
mod interactions;
mod keypair;
mod live_data;
mod network;
mod profile;
mod remotes;
mod ufa;
mod ui;
mod user;

use tauri::Manager;

async fn launch_app(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();
    db::init(handle).await?;
    keypair::init(handle).await?;
    app.manage(interactions::InteractionState::default());
    network::init(handle).await?;
    app.manage(ufa::ForegroundAppState::default());
    app.manage(cursor::CursorState::default());
    ufa::init(handle);
    cursor::init(handle);
    ui::init(handle);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = specta_builder();

    #[cfg(debug_assertions)]
    specta_builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/lib/bindings.ts",
        )
        .expect("Failed to export TypeScript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
        .commands(tauri_specta::collect_commands![
            friends::create_friend,
            friends::list_friends,
            friends::get_friend,
            friends::delete_friend,
            remotes::create_remote,
            remotes::list_remotes,
            remotes::get_remote,
            remotes::update_remote,
            remotes::delete_remote,
            profile::get_profile,
            profile::update_profile,
            keypair::get_public_key,
            network::list_statuses,
            network::list_friend_statuses,
            network::resolve_friend_display_name,
            images::pick_and_send_image,
            images::send_image_bytes,
            interactions::send_interaction,
            ui::control_panel::open_action_window,
            ui::scene::update_scene_hitboxes,
        ])
        .events(tauri_specta::collect_events![
            friends::FriendsChanged,
            remotes::RemotesChanged,
            profile::ProfileChanged,
            network::NetworkStatusChanged,
            network::FriendStatusesChanged,
            cursor::CursorPositionChanged,
            ufa::ForegroundAppChanged,
            ufa::FriendForegroundAppChanged,
            interactions::FriendInteractionReceived,
        ])
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "run explicitly to regenerate frontend bindings"]
    fn export_typescript_bindings() {
        super::specta_builder()
            .export(
                specta_typescript::Typescript::default(),
                "../src/lib/bindings.ts",
            )
            .expect("export TypeScript bindings");
    }
}
