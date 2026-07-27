use super::types::AppMetadata;
use std::ffi::OsString;
use std::iter;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::ptr;
use tracing::{info, warn};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::Globalization::GetUserDefaultLangID;
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowTextW, GetWindowThreadProcessId,
    EVENT_SYSTEM_FOREGROUND, MSG, WINEVENT_OUTOFCONTEXT,
};

pub fn listen_for_active_app_changes<F>(callback: F)
where
    F: Fn(AppMetadata) + Send + 'static,
{
    // Run the hook on a background thread so we don't block the caller.
    std::thread::spawn(move || unsafe {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(win_event_proc::<F>),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );

        if hook.is_invalid() {
            eprintln!("Failed to set event hook");
            return;
        }

        // store callback in TLS for this thread
        CALLBACK.with(|cb| cb.replace(Some(Box::new(callback))));

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }

        let _ = UnhookWinEvent(hook);
    });
}

// RefCell is safe here because the callback is stored and accessed only within the dedicated event loop thread.
// No concurrent access occurs since WinEventHook runs on a single background thread.
thread_local! {
    static CALLBACK: std::cell::RefCell<Option<Box<dyn Fn(AppMetadata) + Send + 'static>>> =
        std::cell::RefCell::new(None);
}

unsafe extern "system" fn win_event_proc<F>(
    _h_win_event_hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dw_ms_event_time: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND {
        let names = get_active_app_metadata_windows(Some(hwnd));
        CALLBACK.with(|cb| {
            if let Some(ref cb) = *cb.borrow() {
                cb(names);
            }
        });
    }
}

/// Retrieves metadata for the active application on Windows, optionally using a provided window handle.
pub fn get_active_app_metadata_windows(hwnd_override: Option<HWND>) -> AppMetadata {
    unsafe {
        let hwnd = hwnd_override.unwrap_or_else(|| GetForegroundWindow());
        if hwnd.0 == std::ptr::null_mut() {
            warn!("No foreground window found");
            return AppMetadata {
                localized: None,
                unlocalized: None,
                app_icon_b64: None,
            };
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            warn!("Failed to get process ID for foreground window");
            return AppMetadata {
                localized: None,
                unlocalized: None,
                app_icon_b64: None,
            };
        }
        let process_handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) if !h.is_invalid() => h,
            _ => {
                warn!("Failed to open process {} for querying", pid);
                return AppMetadata {
                    localized: None,
                    unlocalized: None,
                    app_icon_b64: None,
                };
            }
        };

        let mut buffer: [u16; 1024] = [0; 1024];
        let size = GetModuleFileNameExW(process_handle, None, &mut buffer);
        let exe_path = if size > 0 {
            OsString::from_wide(&buffer[..size as usize])
        } else {
            warn!("Failed to get module file name for process {}", pid);
            return AppMetadata {
                localized: None,
                unlocalized: None,
                app_icon_b64: None,
            };
        };
        let exe_path_str = exe_path.to_string_lossy();
        let exe_name = Path::new(&*exe_path_str)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let is_uwp = exe_name.eq_ignore_ascii_case("ApplicationFrameHost.exe");
        let (localized, unlocalized) = if is_uwp {
            (
                get_window_title(hwnd),
                Some("ApplicationFrameHost".to_string()),
            )
        } else {
            let unlocalized = Path::new(&*exe_path_str)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());
            let localized = get_file_description(&exe_path_str).or_else(|| unlocalized.clone());
            (localized, unlocalized)
        };

        let app_icon_b64 = get_active_app_icon_b64(&exe_path_str);

        AppMetadata {
            localized,
            unlocalized,
            app_icon_b64,
        }
    }
}

fn get_window_title(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut buffer: [u16; 512] = [0; 512];
        let len = GetWindowTextW(hwnd, &mut buffer);
        if len == 0 {
            None
        } else {
            Some(
                OsString::from_wide(&buffer[..len as usize])
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    }
}

fn get_file_description(exe_path: &str) -> Option<String> {
    unsafe {
        let wide_path: Vec<u16> = exe_path.encode_utf16().chain(iter::once(0)).collect();
        let size = windows::Win32::Storage::FileSystem::GetFileVersionInfoSizeW(
            PCWSTR(wide_path.as_ptr()),
            None,
        );
        if size == 0 {
            return None;
        }
        let mut buffer: Vec<u8> = vec![0; size as usize];
        let result = windows::Win32::Storage::FileSystem::GetFileVersionInfoW(
            PCWSTR(wide_path.as_ptr()),
            0,
            size,
            buffer.as_mut_ptr() as *mut _,
        );
        if result.is_err() {
            return None;
        }
        let mut translations_ptr: *mut u8 = ptr::null_mut();
        let mut translations_len: u32 = 0;
        let translation_query: Vec<u16> = "\\VarFileInfo\\Translation"
            .encode_utf16()
            .chain(iter::once(0))
            .collect();
        let trans_result = windows::Win32::Storage::FileSystem::VerQueryValueW(
            buffer.as_ptr() as *const _,
            PCWSTR(translation_query.as_ptr()),
            &mut translations_ptr as *mut _ as *mut *mut _,
            &mut translations_len,
        );
        let system_lang_id = GetUserDefaultLangID();
        if trans_result.as_bool() && translations_len >= 4 {
            let translations = std::slice::from_raw_parts(
                translations_ptr as *const u16,
                (translations_len / 2) as usize,
            );
            let mut lang_attempts = Vec::new();
            for i in (0..translations.len()).step_by(2) {
                if i + 1 < translations.len() {
                    let lang = translations[i];
                    let codepage = translations[i + 1];
                    if lang == system_lang_id {
                        lang_attempts.insert(0, (lang, codepage));
                    } else {
                        lang_attempts.push((lang, codepage));
                    }
                }
            }
            for (lang, codepage) in lang_attempts {
                let query = format!(
                    "\\StringFileInfo\\{:04x}{:04x}\\FileDescription",
                    lang, codepage
                );
                let wide_query: Vec<u16> = query.encode_utf16().chain(iter::once(0)).collect();
                let mut value_ptr: *mut u8 = ptr::null_mut();
                let mut value_len: u32 = 0;
                let query_result = windows::Win32::Storage::FileSystem::VerQueryValueW(
                    buffer.as_ptr() as *const _,
                    PCWSTR(wide_query.as_ptr()),
                    &mut value_ptr as *mut _ as *mut *mut _,
                    &mut value_len,
                );
                if query_result.as_bool() && !value_ptr.is_null() && value_len > 0 {
                    let wide_str = std::slice::from_raw_parts(
                        value_ptr as *const u16,
                        (value_len as usize).saturating_sub(1),
                    );
                    let description = OsString::from_wide(wide_str).to_string_lossy().into_owned();
                    if !description.is_empty() {
                        return Some(description);
                    }
                }
            }
        }
        let fallback_query: Vec<u16> = "\\StringFileInfo\\040904b0\\FileDescription"
            .encode_utf16()
            .chain(iter::once(0))
            .collect();
        let mut value_ptr: *mut u8 = ptr::null_mut();
        let mut value_len: u32 = 0;
        let query_result = windows::Win32::Storage::FileSystem::VerQueryValueW(
            buffer.as_ptr() as *const _,
            PCWSTR(fallback_query.as_ptr()),
            &mut value_ptr as *mut _ as *mut *mut _,
            &mut value_len,
        );
        if query_result.as_bool() && !value_ptr.is_null() && value_len > 0 {
            let wide_str = std::slice::from_raw_parts(
                value_ptr as *const u16,
                (value_len as usize).saturating_sub(1),
            );
            Some(OsString::from_wide(wide_str).to_string_lossy().into_owned())
        } else {
            None
        }
    }
}

fn get_cached_icon(path: &str) -> Option<String> {
    super::icon_cache::get(path)
}

fn put_cached_icon(path: &str, value: String) {
    super::icon_cache::put(path, value);
}

/// RAII wrapper for Windows HICON handles to ensure proper cleanup.
struct IconHandle(windows::Win32::UI::WindowsAndMessaging::HICON);

impl Drop for IconHandle {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::DestroyIcon(self.0);
        }
    }
}

fn get_active_app_icon_b64(exe_path: &str) -> Option<String> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW,
        SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    // Check cache first
    if let Some(cached) = get_cached_icon(exe_path) {
        return Some(cached);
    }

    unsafe {
        let wide_path: Vec<u16> = exe_path.encode_utf16().chain(iter::once(0)).collect();
        let mut file_info: SHFILEINFOW = std::mem::zeroed();

        use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;

        let result = SHGetFileInfoW(
            PCWSTR(wide_path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut file_info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );

        if result == 0 || file_info.hIcon.is_invalid() {
            warn!("Failed to get icon for {}", exe_path);
            return None;
        }

        let icon_handle = IconHandle(file_info.hIcon);

        // Get icon info to extract bitmap
        let mut icon_info: ICONINFO = std::mem::zeroed();
        if GetIconInfo(icon_handle.0, &mut icon_info).is_err() {
            warn!("Failed to get icon info for {}", exe_path);
            return None;
        }

        // Get bitmap dimensions
        let mut bitmap: BITMAP = std::mem::zeroed();
        if GetObjectW(
            icon_info.hbmColor,
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut _ as *mut _),
        ) == 0
        {
            let _ = DeleteObject(icon_info.hbmColor);
            let _ = DeleteObject(icon_info.hbmMask);
            warn!("Failed to get bitmap object for {}", exe_path);
            return None;
        }

        let width = bitmap.bmWidth;
        let height = bitmap.bmHeight;

        // Create device contexts
        let hdc_screen = windows::Win32::Graphics::Gdi::GetDC(HWND(ptr::null_mut()));
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbm_new = CreateCompatibleBitmap(
            hdc_screen,
            super::types::ICON_SIZE as i32,
            super::types::ICON_SIZE as i32,
        );
        let hbm_old = SelectObject(hdc_mem, hbm_new);

        // Draw icon
        let _ = windows::Win32::UI::WindowsAndMessaging::DrawIconEx(
            hdc_mem,
            0,
            0,
            icon_handle.0,
            super::types::ICON_SIZE as i32,
            super::types::ICON_SIZE as i32,
            0,
            None,
            windows::Win32::UI::WindowsAndMessaging::DI_NORMAL,
        );

        // Prepare BITMAPINFO for GetDIBits
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = super::types::ICON_SIZE as i32;
        bmi.bmiHeader.biHeight = -(super::types::ICON_SIZE as i32); // Top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0 as u32;

        // Allocate buffer for pixel data
        let buffer_size = (super::types::ICON_SIZE * super::types::ICON_SIZE * 4) as usize;
        let mut buffer: Vec<u8> = vec![0; buffer_size];

        // Get bitmap bits
        let result = GetDIBits(
            hdc_mem,
            hbm_new,
            0,
            super::types::ICON_SIZE,
            Some(buffer.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        // Clean up
        let _ = SelectObject(hdc_mem, hbm_old);
        let _ = DeleteObject(hbm_new);
        let _ = DeleteDC(hdc_mem);
        let _ = windows::Win32::Graphics::Gdi::ReleaseDC(HWND(ptr::null_mut()), hdc_screen);
        let _ = DeleteObject(icon_info.hbmColor);
        let _ = DeleteObject(icon_info.hbmMask);

        if result == 0 {
            warn!("Failed to get bitmap bits for {}", exe_path);
            return None;
        }

        // Convert BGRA to RGBA
        for i in (0..buffer.len()).step_by(4) {
            buffer.swap(i, i + 2); // Swap B and R
        }

        // Encode as PNG using image crate
        let img = match image::RgbaImage::from_raw(
            super::types::ICON_SIZE,
            super::types::ICON_SIZE,
            buffer,
        ) {
            Some(img) => img,
            None => {
                warn!("Failed to create image from buffer for {}", exe_path);
                return None;
            }
        };

        let mut png_buffer = Vec::new();
        if let Err(e) = img.write_to(
            &mut std::io::Cursor::new(&mut png_buffer),
            image::ImageFormat::Png,
        ) {
            warn!("Failed to encode PNG for {}: {}", exe_path, e);
            return None;
        }

        let encoded = STANDARD.encode(&png_buffer);

        info!(
            "App icon captured: {}, PNG bytes: {}, base64 size: {} bytes",
            exe_path,
            png_buffer.len(),
            encoded.len()
        );

        if encoded.len() > super::types::MAX_ICON_B64_SIZE {
            warn!(
                "Icon for {} exceeds size limit ({} > {} bytes), skipping",
                exe_path,
                encoded.len(),
                super::types::MAX_ICON_B64_SIZE
            );
            return None;
        }

        put_cached_icon(exe_path, encoded.clone());

        Some(encoded)
    }
}
