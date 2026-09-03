use tauri::{
    AppHandle, Manager, Wry,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

use super::control_panel;

const OPEN_CONTROL_PANEL_ID: &str = "open-control-panel";
const QUIT_ID: &str = "quit";

struct TrayMenuState {
    open_control_panel: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

pub fn init(handle: &AppHandle) {
    let open_control_panel = MenuItem::with_id(
        handle,
        OPEN_CONTROL_PANEL_ID,
        crate::settings::text(handle, crate::settings::NativeText::OpenControlPanel),
        true,
        None::<&str>,
    )
    .expect("Open Control Panel tray menu item should be created");
    let quit = MenuItem::with_id(
        handle,
        QUIT_ID,
        crate::settings::text(handle, crate::settings::NativeText::Quit),
        true,
        None::<&str>,
    )
    .expect("Quit tray menu item should be created");
    let menu = Menu::with_items(handle, &[&open_control_panel, &quit])
        .expect("Tray menu should be created");

    let mut builder = TrayIconBuilder::new()
        .tooltip("Friendolls")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN_CONTROL_PANEL_ID => {
                if let Err(error) = control_panel::show(app) {
                    eprintln!("Failed to show control panel: {error}");
                }
            }
            QUIT_ID => app.exit(0),
            _ => {}
        });

    if let Some(icon) = handle.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(handle).expect("Tray icon should be created");
    handle.manage(TrayMenuState {
        open_control_panel,
        quit,
    });
}

pub fn refresh_locale(handle: &AppHandle) {
    let Some(items) = handle.try_state::<TrayMenuState>() else {
        return;
    };
    let _ = items.open_control_panel.set_text(crate::settings::text(
        handle,
        crate::settings::NativeText::OpenControlPanel,
    ));
    let _ = items.quit.set_text(crate::settings::text(
        handle,
        crate::settings::NativeText::Quit,
    ));
}
