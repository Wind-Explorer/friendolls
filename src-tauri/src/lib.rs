mod application;
mod cursor;
mod db;
mod friends;
mod images;
mod interactions;
mod keypair;
mod live_data;
mod network;
mod onboarding;
mod profile;
mod puppet;
mod remotes;
mod scene_configuration;
mod settings;
mod skins;
mod ufa;
mod ui;
mod updater;
mod user;

async fn launch_app(handle: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    updater::init(handle).await;
    db::init(handle).await?;
    settings::init(handle).await?;
    scene_configuration::init(handle).await?;
    keypair::init(handle).await?;
    interactions::init(handle);
    cursor::init(handle);
    network::init(handle).await?;
    ufa::init(handle);
    puppet::init(handle)?;
    application::init(handle).await?;
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

    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(ui::activation_policy::ActivationPolicy::default());

    builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            specta_builder.mount_events(app);
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                ui::splashscreen::open(&handle).await.unwrap();
                let launch_result = launch_app(&handle).await.map_err(|error| error.to_string());

                if let Err(error) = launch_result {
                    eprintln!("failed to launch application: {error}");
                    handle.exit(1);
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_, e| {
            if let tauri::RunEvent::ExitRequested { code, api, .. } = e {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
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
            remotes::reorder_remotes,
            profile::get_profile,
            profile::update_profile,
            profile::reset_profile_skin,
            keypair::get_public_key,
            network::list_statuses,
            network::list_friend_statuses,
            network::resolve_friend_display_name,
            live_data::list_live_data,
            skins::resolve_skin,
            images::pick_and_send_image,
            images::send_image_bytes,
            interactions::send_interaction,
            puppet::list_puppet_states,
            scene_configuration::get_scene_configuration,
            scene_configuration::update_scene_configuration,
            onboarding::complete_onboarding,
            settings::autostart::get_autostart_enabled,
            settings::autostart::set_autostart_enabled,
            settings::get_locale_settings,
            settings::set_locale_preference,
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
            puppet::PuppetStatesChanged,
            scene_configuration::SceneConfigurationChanged,
            settings::LocaleChanged,
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
