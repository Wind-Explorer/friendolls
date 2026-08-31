use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

pub async fn init(app: &AppHandle) {
    update_app(app).await
}

async fn update_app(app: &AppHandle) {
    if let Some(update) = match match app.updater() {
        Ok(it) => it,
        Err(err) => {
            println!("failed to get updater: {err:?}");
            return;
        }
    }
    .check()
    .await
    {
        Ok(it) => it,
        Err(err) => {
            println!("failed to check for update: {err:?}");
            return;
        }
    } {
        let mut downloaded = 0;

        match update
            .download_and_install(
                |chunk_length, content_length| {
                    downloaded += chunk_length;
                    println!("downloaded {downloaded} from {content_length:?}");
                },
                || {
                    println!("download finished");
                },
            )
            .await
        {
            Ok(it) => it,
            Err(err) => {
                println!("failed to install update: {err:?}");
                return;
            }
        };

        println!("update installed");
        app.restart();
    }
}
