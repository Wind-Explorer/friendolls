use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use crate::db::{self, AppDatabase};

const SETTINGS_ID: i64 = 1;
pub const SYSTEM_LOCALE_PREFERENCE: &str = "system";
pub const BASE_LOCALE: &str = "en";
pub const SUPPORTED_LOCALES: &[&str] = &["en", "zh-CN"];

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct AppSettings {
    pub onboarding_done: bool,
    pub locale_preference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct LocaleChanged {
    pub preference: String,
    pub locale: String,
}

pub struct SettingsState {
    locale: RwLock<LocaleChanged>,
}

#[derive(Debug, Clone, Copy)]
pub enum NativeText {
    OpenControlPanel,
    Quit,
    OutdatedTitle,
    OutdatedMessage,
    CloseFriendolls,
    Images,
    OnboardingTitle,
    SceneTitle,
}

pub async fn init(handle: &AppHandle) -> Result<(), String> {
    let settings = get(&handle.state()).await.map_err(db::command_error)?;
    handle.manage(SettingsState {
        locale: RwLock::new(locale_settings(&settings.locale_preference)),
    });
    Ok(())
}

pub async fn get(database: &AppDatabase) -> Result<AppSettings, sqlx::Error> {
    sqlx::query_as::<_, AppSettings>(
        "SELECT onboarding_done, locale_preference FROM app_settings WHERE id = ?1",
    )
    .bind(SETTINGS_ID)
    .fetch_one(database.pool())
    .await
}

pub async fn set_onboarding_done(
    database: &AppDatabase,
    onboarding_done: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE app_settings SET onboarding_done = ?1 WHERE id = ?2")
        .bind(onboarding_done)
        .bind(SETTINGS_ID)
        .execute(database.pool())
        .await?;
    Ok(())
}

fn locale_settings(preference: &str) -> LocaleChanged {
    let locale = if preference == SYSTEM_LOCALE_PREFERENCE {
        resolve_locale(sys_locale::get_locale().as_deref())
    } else {
        resolve_locale(Some(preference))
    };
    LocaleChanged {
        preference: preference.to_owned(),
        locale,
    }
}

fn resolve_locale(requested: Option<&str>) -> String {
    let requested = requested.unwrap_or_default().replace('_', "-");
    if let Some(locale) = SUPPORTED_LOCALES
        .iter()
        .find(|locale| locale.eq_ignore_ascii_case(&requested))
    {
        return (*locale).to_owned();
    }

    let requested_language = requested.split('-').next().unwrap_or_default();
    SUPPORTED_LOCALES
        .iter()
        .find(|locale| {
            locale
                .split('-')
                .next()
                .is_some_and(|language| language.eq_ignore_ascii_case(requested_language))
        })
        .copied()
        .unwrap_or(BASE_LOCALE)
        .to_owned()
}

fn is_supported_preference(preference: &str) -> bool {
    preference == SYSTEM_LOCALE_PREFERENCE || SUPPORTED_LOCALES.contains(&preference)
}

pub fn current(handle: &AppHandle) -> LocaleChanged {
    handle
        .state::<SettingsState>()
        .locale
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

pub fn text(handle: &AppHandle, key: NativeText) -> &'static str {
    localized_text(&current(handle).locale, key)
}

pub fn system_text(key: NativeText) -> &'static str {
    localized_text(&resolve_locale(sys_locale::get_locale().as_deref()), key)
}

fn localized_text(locale: &str, key: NativeText) -> &'static str {
    match (locale, key) {
        ("zh-CN", NativeText::OpenControlPanel) => "打开控制面板",
        ("zh-CN", NativeText::Quit) => "退出",
        ("zh-CN", NativeText::OutdatedTitle) => "Friendolls 版本过旧",
        ("zh-CN", NativeText::OutdatedMessage) => {
            "此设备上的数据由较新版本的 Friendolls 创建，当前版本无法安全地打开这些数据。\n\n请下载并安装最新版 Friendolls，然后重新打开应用。你的数据未被更改。"
        }
        ("zh-CN", NativeText::CloseFriendolls) => "关闭 Friendolls",
        ("zh-CN", NativeText::Images) => "图片",
        ("zh-CN", NativeText::OnboardingTitle) => "Friendolls 安装向导",
        ("zh-CN", NativeText::SceneTitle) => "场景",
        (_, NativeText::OpenControlPanel) => "Open Control Panel",
        (_, NativeText::Quit) => "Quit",
        (_, NativeText::OutdatedTitle) => "Friendolls is out of date",
        (_, NativeText::OutdatedMessage) => {
            "Your on-device data was created by a newer version of Friendolls and cannot be opened safely by this version.\n\nDownload and install the latest version of Friendolls, then reopen the app. Your data has not been changed."
        }
        (_, NativeText::CloseFriendolls) => "Close Friendolls",
        (_, NativeText::Images) => "Images",
        (_, NativeText::OnboardingTitle) => "Friendolls Setup",
        (_, NativeText::SceneTitle) => "Scene",
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_locale_settings(handle: AppHandle) -> LocaleChanged {
    current(&handle)
}

#[tauri::command]
#[specta::specta]
pub async fn set_locale_preference(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    preference: String,
) -> Result<LocaleChanged, String> {
    if !is_supported_preference(&preference) {
        return Err("Unsupported locale preference".to_owned());
    }

    sqlx::query("UPDATE app_settings SET locale_preference = ?1 WHERE id = ?2")
        .bind(&preference)
        .bind(SETTINGS_ID)
        .execute(database.pool())
        .await
        .map_err(db::command_error)?;

    let next = locale_settings(&preference);
    *handle
        .state::<SettingsState>()
        .locale
        .write()
        .unwrap_or_else(|error| error.into_inner()) = next.clone();
    next.clone().emit(&handle).map_err(db::command_error)?;
    crate::ui::refresh_locale(&handle);
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_exact_and_language_only_locales() {
        assert_eq!(resolve_locale(Some("zh-CN")), "zh-CN");
        assert_eq!(resolve_locale(Some("zh_SG")), "zh-CN");
        assert_eq!(resolve_locale(Some("en-GB")), "en");
    }

    #[test]
    fn unsupported_locales_fall_back_to_english() {
        assert_eq!(resolve_locale(Some("ja-JP")), "en");
        assert_eq!(resolve_locale(None), "en");
    }
}
