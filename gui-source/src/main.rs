//! Public native Win32 front end. Hardware polling stays on a worker thread;
//! this file owns only window lifetime and message dispatch. Drawing and sensor
//! history deliberately live in separate modules to keep UI changes isolated.
#![windows_subsystem = "windows"]

mod feedback;
mod gui_i18n;
mod gui_layout;
mod gui_settings;
mod gui_state;
mod gui_view;
mod sensor_model;
mod settings;
mod updates;

use gpu_shark::{SysInfo, dll_library_path, fetch_data_from_dll, load_driver_library};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicIsize, Ordering},
    mpsc,
};
use std::{thread, time::Duration};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH, DeleteDC,
    DeleteObject, EndPaint, FF_DONTCARE, FW_BOLD, FW_NORMAL, HDC, InvalidateRect, MM_ANISOTROPIC,
    MM_TEXT, OUT_DEFAULT_PRECIS, PAINTSTRUCT, SRCCOPY, SelectObject, SetBkMode, SetMapMode,
    SetViewportExtEx, SetWindowExtEx, TRANSPARENT, UpdateWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW,
    DefWindowProcW, DestroyWindow, DispatchMessageW, ES_AUTOVSCROLL, ES_MULTILINE, GetClientRect,
    GetMessageW, GetWindowTextW, ICON_SMALL, IDC_ARROW, IDI_APPLICATION, IMAGE_ICON, LR_SHARED,
    LoadCursorW, LoadIconW, LoadImageW, MSG, MoveWindow, PostMessageW, PostQuitMessage,
    RegisterClassW, SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, ShowWindow,
    TranslateMessage, WM_APP, WM_CLOSE, WM_CTLCOLOREDIT, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_PAINT, WM_SETICON, WNDCLASSW, WS_BORDER, WS_CAPTION,
    WS_CHILD, WS_MINIMIZEBOX, WS_SYSMENU, WS_VISIBLE, WS_VSCROLL,
};

const WM_APP_SNAPSHOT: u32 = WM_APP + 1;
const WM_APP_FEEDBACK: u32 = WM_APP + 2;
const WM_APP_UPDATE: u32 = WM_APP + 3;
const WM_APP_UPDATE_APPLIED: u32 = WM_APP + 4;
static SHARED: OnceLock<Arc<Mutex<Option<Snapshot>>>> = OnceLock::new();
static STOP_TX: OnceLock<mpsc::Sender<()>> = OnceLock::new();
static SENSOR_HISTORY: OnceLock<Mutex<gui_state::SensorHistory>> = OnceLock::new();
static FEEDBACK_VISIBLE: AtomicBool = AtomicBool::new(false);
static ABOUT_VISIBLE: AtomicBool = AtomicBool::new(false);
static FEEDBACK_EDIT: AtomicIsize = AtomicIsize::new(0);
static FEEDBACK_CONTACT: AtomicIsize = AtomicIsize::new(0);
static EDIT_BACKGROUND_BRUSH: OnceLock<isize> = OnceLock::new();
static ABOUT_ICON: OnceLock<isize> = OnceLock::new();
static HEADER_NAV: OnceLock<Mutex<gui_view::HeaderNavLayout>> = OnceLock::new();
static FEEDBACK_STATUS: OnceLock<Mutex<FeedbackStatus>> = OnceLock::new();
static FEEDBACK_CONSENT: AtomicBool = AtomicBool::new(false);
static FEEDBACK_SENDING: AtomicBool = AtomicBool::new(false);
static UPDATE_STATUS: OnceLock<Mutex<updates::UpdateStatus>> = OnceLock::new();
static UPDATE_CHECK_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
enum Snapshot {
    Data(SysInfo),
    LoadFailed(String),
    FetchError(String),
}

#[derive(Clone, Default)]
enum FeedbackStatus {
    #[default]
    Empty,
    Required,
    Sending,
    Accepted(String),
    Rejected(String),
    PayloadTooLarge,
    RateLimited,
    Server,
    Network(String),
}

impl FeedbackStatus {
    fn localized(&self, language: gui_i18n::Language) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Required => language.feedback_required().into(),
            Self::Sending => language.feedback_sending_status().into(),
            Self::Accepted(report_id) => language.feedback_accepted(report_id),
            Self::Rejected(detail) => language.feedback_rejected(detail),
            Self::PayloadTooLarge => language.feedback_payload_too_large().into(),
            Self::RateLimited => language.feedback_rate_limited().into(),
            Self::Server => language.feedback_server_error().into(),
            Self::Network(error) => language.feedback_network_error(error),
        }
    }
}

pub(crate) fn wstr(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn draw_text(hdc: HDC, x: i32, y: i32, text: &str, color: u32) {
    let wide = wstr(text);
    unsafe {
        windows_sys::Win32::Graphics::Gdi::SetTextColor(hdc, color);
        windows_sys::Win32::Graphics::Gdi::TextOutW(
            hdc,
            x,
            y,
            wide.as_ptr(),
            (wide.len() - 1) as i32,
        );
    }
}

unsafe fn show_feedback(hwnd: HWND) {
    unsafe {
        ABOUT_VISIBLE.store(false, Ordering::Release);
        FEEDBACK_VISIBLE.store(true, Ordering::Release);
        let edit = FEEDBACK_EDIT.load(Ordering::Acquire);
        if edit == 0 {
            let control = CreateWindowExW(
                0,
                wstr("EDIT").as_ptr(),
                std::ptr::null(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_BORDER
                    | (ES_MULTILINE as u32)
                    | (ES_AUTOVSCROLL as u32)
                    | WS_VSCROLL,
                gui_view::FEEDBACK_MESSAGE_EDIT.left,
                gui_view::FEEDBACK_MESSAGE_EDIT.top,
                gui_view::FEEDBACK_MESSAGE_EDIT.right - gui_view::FEEDBACK_MESSAGE_EDIT.left,
                gui_view::FEEDBACK_MESSAGE_EDIT.bottom - gui_view::FEEDBACK_MESSAGE_EDIT.top,
                hwnd,
                0,
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );
            FEEDBACK_EDIT.store(control, Ordering::Release);
            let contact = CreateWindowExW(
                0,
                wstr("EDIT").as_ptr(),
                std::ptr::null(),
                WS_CHILD | WS_VISIBLE | WS_BORDER,
                gui_view::FEEDBACK_CONTACT_EDIT.left,
                gui_view::FEEDBACK_CONTACT_EDIT.top,
                gui_view::FEEDBACK_CONTACT_EDIT.right - gui_view::FEEDBACK_CONTACT_EDIT.left,
                gui_view::FEEDBACK_CONTACT_EDIT.bottom - gui_view::FEEDBACK_CONTACT_EDIT.top,
                hwnd,
                0,
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );
            FEEDBACK_CONTACT.store(contact, Ordering::Release);
        } else {
            ShowWindow(edit, SW_SHOW);
            let contact = FEEDBACK_CONTACT.load(Ordering::Acquire);
            if contact != 0 {
                ShowWindow(contact, SW_SHOW);
            }
        }
        layout_feedback_controls(hwnd);
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
}

unsafe fn hide_feedback_controls() {
    unsafe {
        let edit = FEEDBACK_EDIT.load(Ordering::Acquire);
        if edit != 0 {
            ShowWindow(edit, 0);
        }
        let contact = FEEDBACK_CONTACT.load(Ordering::Acquire);
        if contact != 0 {
            ShowWindow(contact, 0);
        }
    }
}

unsafe fn show_about(hwnd: HWND) {
    unsafe {
        FEEDBACK_VISIBLE.store(false, Ordering::Release);
        ABOUT_VISIBLE.store(true, Ordering::Release);
        hide_feedback_controls();
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
}

fn update_status_snapshot() -> updates::UpdateStatus {
    UPDATE_STATUS
        .get()
        .and_then(|slot| slot.lock().ok())
        .map(|value| value.clone())
        .unwrap_or(updates::UpdateStatus::Idle)
}

fn start_update_check(hwnd: HWND) {
    if UPDATE_CHECK_RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    if let Some(slot) = UPDATE_STATUS.get() {
        if let Ok(mut value) = slot.lock() {
            *value = updates::UpdateStatus::Checking;
        }
    }
    unsafe {
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
    thread::spawn(move || {
        let status = match updates::fetch_latest() {
            Ok(release) => {
                if updates::is_newer(&release.tag_name, updates::current_version()) {
                    updates::UpdateStatus::Available {
                        latest: release.tag_name,
                        url: release.html_url,
                    }
                } else {
                    updates::UpdateStatus::UpToDate
                }
            }
            Err(error) => updates::UpdateStatus::Failed(error),
        };
        if let Some(slot) = UPDATE_STATUS.get() {
            if let Ok(mut value) = slot.lock() {
                *value = status;
            }
        }
        UPDATE_CHECK_RUNNING.store(false, Ordering::Release);
        unsafe {
            PostMessageW(hwnd, WM_APP_UPDATE, 0, 0);
        }
    });
}

fn start_update_download(hwnd: HWND) {
    if UPDATE_CHECK_RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    if let Some(slot) = UPDATE_STATUS.get() {
        if let Ok(mut value) = slot.lock() {
            *value = updates::UpdateStatus::Downloading;
        }
    }
    unsafe {
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
    thread::spawn(move || {
        let root = updates::update_root().unwrap_or_else(|_| std::env::temp_dir());
        let status = match updates::prepare_install(&root) {
            Ok(prepared) => updates::UpdateStatus::ReadyToInstall {
                exe_path: prepared.exe_path.display().to_string(),
                url: prepared.page_url,
            },
            Err(detail) => updates::UpdateStatus::InstallFailed {
                detail,
                url: String::new(),
            },
        };
        if let Some(slot) = UPDATE_STATUS.get() {
            if let Ok(mut value) = slot.lock() {
                *value = status;
            }
        }
        UPDATE_CHECK_RUNNING.store(false, Ordering::Release);
        unsafe {
            PostMessageW(hwnd, WM_APP_UPDATE, 0, 0);
        }
    });
}

fn start_update_install(hwnd: HWND, new_exe: &str) {
    if UPDATE_CHECK_RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    let new_exe = new_exe.to_owned();
    thread::spawn(move || {
        let result = updates::apply_prepared(std::path::Path::new(&new_exe));
        let success = result.is_ok();
        if let Err(detail) = result {
            if let Some(slot) = UPDATE_STATUS.get() {
                if let Ok(mut value) = slot.lock() {
                    *value = updates::UpdateStatus::InstallFailed {
                        detail,
                        url: String::new(),
                    };
                }
            }
        }
        UPDATE_CHECK_RUNNING.store(false, Ordering::Release);
        unsafe {
            PostMessageW(hwnd, WM_APP_UPDATE_APPLIED, usize::from(success), 0);
        }
    });
}

fn launch_updated_app() {
    let Ok(current) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(&current).spawn();
}

fn open_release_page(url: &str) {
    let verb = wstr("open");
    let file = wstr(url);
    unsafe {
        ShellExecuteW(
            0,
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOW,
        );
    }
}

unsafe fn layout_feedback_controls(hwnd: HWND) {
    unsafe {
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut client);
        let sx =
            |value| gui_layout::scale_to_client(value, gui_layout::BASE_CLIENT_WIDTH, client.right);
        let sy = |value| {
            gui_layout::scale_to_client(value, gui_layout::BASE_CLIENT_HEIGHT, client.bottom)
        };
        let edit = FEEDBACK_EDIT.load(Ordering::Acquire);
        if edit != 0 {
            let rect = &gui_view::FEEDBACK_MESSAGE_EDIT;
            MoveWindow(
                edit,
                sx(rect.left),
                sy(rect.top),
                sx(rect.right - rect.left),
                sy(rect.bottom - rect.top),
                1,
            );
        }
        let contact = FEEDBACK_CONTACT.load(Ordering::Acquire);
        if contact != 0 {
            let rect = &gui_view::FEEDBACK_CONTACT_EDIT;
            MoveWindow(
                contact,
                sx(rect.left),
                sy(rect.top),
                sx(rect.right - rect.left),
                sy(rect.bottom - rect.top),
                1,
            );
        }
    }
}

unsafe fn logical_mouse_point(hwnd: HWND, lparam: LPARAM) -> (i32, i32) {
    unsafe {
        let physical_x = (lparam as u32 & 0xffff) as u16 as i16 as i32;
        let physical_y = ((lparam as u32 >> 16) & 0xffff) as u16 as i16 as i32;
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut client);
        gui_layout::to_logical_point(physical_x, physical_y, client.right, client.bottom)
    }
}

unsafe fn read_control_text(handle: HWND, capacity: usize) -> String {
    unsafe {
        let mut buffer = vec![0u16; capacity];
        let count = if handle == 0 {
            0
        } else {
            GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32)
        };
        String::from_utf16_lossy(&buffer[..count.max(0) as usize])
    }
}

unsafe fn submit_feedback(hwnd: HWND) {
    unsafe {
        if FEEDBACK_SENDING.swap(true, Ordering::AcqRel) {
            return;
        }
        let note = read_control_text(FEEDBACK_EDIT.load(Ordering::Acquire), 8192);
        let contact = read_control_text(FEEDBACK_CONTACT.load(Ordering::Acquire), 512);
        let consent = FEEDBACK_CONSENT.load(Ordering::Acquire);
        if note.trim().is_empty() || !consent {
            FEEDBACK_SENDING.store(false, Ordering::Release);
            if let Some(slot) = FEEDBACK_STATUS.get() {
                if let Ok(mut value) = slot.lock() {
                    *value = FeedbackStatus::Required;
                }
            }
            return;
        }
        let snapshot = SHARED
            .get()
            .and_then(|slot| slot.lock().ok())
            .and_then(|slot| slot.clone());
        let info = match snapshot {
            Some(Snapshot::Data(info)) => Some(info),
            _ => None,
        };
        let locale = if gui_settings::is_russian() {
            "ru"
        } else {
            "en"
        };
        if let Some(slot) = FEEDBACK_STATUS.get() {
            if let Ok(mut value) = slot.lock() {
                *value = FeedbackStatus::Sending;
            }
        }
        InvalidateRect(hwnd, std::ptr::null(), 0);
        thread::spawn(move || {
            let result = feedback::submit(info.as_ref(), &note, Some(&contact), locale, consent);
            let status = match result {
                Ok(report_id) => FeedbackStatus::Accepted(report_id),
                Err(feedback::SubmitError::InvalidPayload(detail)) => {
                    FeedbackStatus::Rejected(detail)
                }
                Err(feedback::SubmitError::PayloadTooLarge) => FeedbackStatus::PayloadTooLarge,
                Err(feedback::SubmitError::RateLimited) => FeedbackStatus::RateLimited,
                Err(feedback::SubmitError::Server) => FeedbackStatus::Server,
                Err(feedback::SubmitError::Network(error)) => FeedbackStatus::Network(error),
            };
            if let Some(slot) = FEEDBACK_STATUS.get() {
                if let Ok(mut value) = slot.lock() {
                    *value = status;
                }
            }
            FEEDBACK_SENDING.store(false, Ordering::Release);
            PostMessageW(hwnd, WM_APP_FEEDBACK, 0, 0);
        });
    }
}

unsafe fn paint(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let output_hdc = BeginPaint(hwnd, &mut ps);
        let mut physical_client: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut physical_client);
        let client = RECT {
            left: 0,
            top: 0,
            right: gui_layout::BASE_CLIENT_WIDTH,
            bottom: gui_layout::BASE_CLIENT_HEIGHT,
        };
        // Draw the complete frame off-screen, then copy it in one operation.
        // This removes the visible background flash caused by 1 Hz updates.
        let hdc = CreateCompatibleDC(output_hdc);
        let bitmap =
            CreateCompatibleBitmap(output_hdc, physical_client.right, physical_client.bottom);
        let old_bitmap = SelectObject(hdc, bitmap);
        SetMapMode(hdc, MM_ANISOTROPIC);
        SetWindowExtEx(
            hdc,
            gui_layout::BASE_CLIENT_WIDTH,
            gui_layout::BASE_CLIENT_HEIGHT,
            std::ptr::null_mut(),
        );
        SetViewportExtEx(
            hdc,
            physical_client.right,
            physical_client.bottom,
            std::ptr::null_mut(),
        );
        let brush = windows_sys::Win32::Graphics::Gdi::CreateSolidBrush(gui_view::BACKGROUND);
        windows_sys::Win32::Graphics::Gdi::FillRect(hdc, &client, brush);
        DeleteObject(brush);
        SetBkMode(hdc, TRANSPARENT);
        let main_font = CreateFontW(
            -16,
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH as u32) | (FF_DONTCARE as u32),
            wstr("Segoe UI").as_ptr(),
        );
        let bold_font = CreateFontW(
            -16,
            0,
            0,
            0,
            FW_BOLD as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH as u32) | (FF_DONTCARE as u32),
            wstr("Segoe UI").as_ptr(),
        );
        let old_font = SelectObject(hdc, main_font);
        let snapshot = SHARED
            .get()
            .and_then(|slot| slot.lock().ok())
            .and_then(|slot| slot.clone());
        let language = gui_i18n::Language::from_russian(gui_settings::is_russian());
        let name = match &snapshot {
            Some(Snapshot::Data(info)) => info.gpu_name.as_deref().unwrap_or("Unknown GPU"),
            _ => "GPU Shark",
        };
        let previous = SelectObject(hdc, bold_font);
        draw_text(hdc, 16, 18, "GPU SHARK", gui_view::accent());
        let feedback_text = language.text(gui_i18n::Key::Feedback);
        let settings_text = language.text(gui_i18n::Key::Settings);
        let about_text = language.text(gui_i18n::Key::About);
        let header_nav = gui_view::header_nav_layout(
            client.right,
            gui_view::text_width(hdc, about_text),
            gui_view::text_width(hdc, settings_text),
            gui_view::text_width(hdc, feedback_text),
        );
        if let Some(slot) = HEADER_NAV.get() {
            if let Ok(mut current) = slot.lock() {
                *current = header_nav;
            }
        }
        draw_text(
            hdc,
            header_nav.feedback.0,
            18,
            feedback_text,
            gui_view::accent(),
        );
        draw_text(
            hdc,
            header_nav.settings.0,
            18,
            settings_text,
            gui_view::accent(),
        );
        draw_text(hdc, header_nav.about.0, 18, about_text, gui_view::accent());
        if matches!(&snapshot, Some(Snapshot::Data(info)) if info.gpu_name.is_none()) {
            draw_text(
                hdc,
                16,
                102,
                language.text(gui_i18n::Key::UnknownGpu),
                gui_view::rgb(246, 211, 45),
            );
        }
        gui_view::clipped(
            hdc,
            16,
            51,
            name,
            gui_view::rgb(255, 255, 255),
            client.right - 32,
        );
        SelectObject(hdc, previous);
        let refresh_hint = language.refresh_hint(gui_settings::refresh_interval_ms());
        draw_text(hdc, 16, 78, &refresh_hint, gui_view::rgb(190, 190, 190));
        if FEEDBACK_VISIBLE.load(Ordering::Acquire) {
            let status = FEEDBACK_STATUS
                .get()
                .and_then(|slot| slot.lock().ok())
                .map(|value| value.localized(language))
                .unwrap_or_default();
            gui_view::draw_feedback_form(
                hdc,
                &client,
                &status,
                language,
                FEEDBACK_CONSENT.load(Ordering::Acquire),
                FEEDBACK_SENDING.load(Ordering::Acquire),
            );
        } else if ABOUT_VISIBLE.load(Ordering::Acquire) {
            let instance = GetModuleHandleW(std::ptr::null());
            let icon = *ABOUT_ICON.get_or_init(|| {
                let resource_icon = LoadImageW(
                    instance,
                    1usize as *const u16,
                    IMAGE_ICON,
                    128,
                    128,
                    LR_SHARED,
                );
                if resource_icon != 0 {
                    resource_icon
                } else {
                    LoadIconW(0, IDI_APPLICATION)
                }
            });
            gui_view::draw_about(
                hdc,
                &client,
                icon,
                language,
                concat!("v", env!("CARGO_PKG_VERSION")),
                &update_status_snapshot(),
                gui_settings::current().check_updates,
            );
        } else {
            match &snapshot {
                Some(Snapshot::Data(info)) => {
                    let history = SENSOR_HISTORY.get().and_then(|slot| slot.lock().ok());
                    if let Some(history) = history {
                        gui_view::draw_dashboard(hdc, &client, info, &history, bold_font, language);
                    }
                }
                Some(Snapshot::LoadFailed(error)) => gui_view::draw_unavailable(
                    hdc,
                    &client,
                    &format!("Could not load {}: {error}", dll_library_path()),
                    language,
                ),
                Some(Snapshot::FetchError(error)) => {
                    gui_view::draw_unavailable(hdc, &client, error, language)
                }
                None => gui_view::draw_unavailable(
                    hdc,
                    &client,
                    "Подключение к локальному источнику телеметрии…",
                    language,
                ),
            }
        }
        SelectObject(hdc, old_font);
        DeleteObject(bold_font);
        DeleteObject(main_font);
        SetMapMode(hdc, MM_TEXT);
        BitBlt(
            output_hdc,
            0,
            0,
            physical_client.right,
            physical_client.bottom,
            hdc,
            0,
            0,
            SRCCOPY,
        );
        SelectObject(hdc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(hdc);
        EndPaint(hwnd, &ps);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    _wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_APP_SNAPSHOT => {
                InvalidateRect(hwnd, std::ptr::null(), 0);
                0
            }
            WM_APP_FEEDBACK => {
                InvalidateRect(hwnd, std::ptr::null(), 0);
                0
            }
            WM_APP_UPDATE => {
                InvalidateRect(hwnd, std::ptr::null(), 0);
                0
            }
            WM_APP_UPDATE_APPLIED => {
                if _wparam == 1 {
                    launch_updated_app();
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                } else {
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
                0
            }
            WM_PAINT => {
                paint(hwnd);
                0
            }
            WM_LBUTTONDOWN => {
                let (x, y) = logical_mouse_point(hwnd, lparam);
                if y <= 54 {
                    if let Some(nav) = HEADER_NAV.get().and_then(|slot| slot.lock().ok()) {
                        if gui_view::header_link_contains(nav.feedback, x) {
                            show_feedback(hwnd);
                            return 0;
                        }
                        if gui_view::header_link_contains(nav.settings, x) {
                            gui_settings::show(hwnd);
                            return 0;
                        }
                        if gui_view::header_link_contains(nav.about, x) {
                            show_about(hwnd);
                            return 0;
                        }
                    }
                }
                if FEEDBACK_VISIBLE.load(Ordering::Acquire) {
                    if gui_view::point_in_rect(&gui_view::FEEDBACK_CONSENT_HIT, x, y) {
                        FEEDBACK_CONSENT.fetch_xor(true, Ordering::AcqRel);
                    } else if gui_view::point_in_rect(&gui_view::FEEDBACK_SUBMIT_HIT, x, y) {
                        submit_feedback(hwnd);
                    }
                    if x >= 850 && y <= 170 {
                        FEEDBACK_VISIBLE.store(false, Ordering::Release);
                        hide_feedback_controls();
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }
                if ABOUT_VISIBLE.load(Ordering::Acquire) {
                    if x >= 850 && y <= 170 {
                        ABOUT_VISIBLE.store(false, Ordering::Release);
                    } else if gui_view::point_in_rect(&gui_view::UPDATE_ACTION_HIT, x, y) {
                        match update_status_snapshot() {
                            updates::UpdateStatus::Available { .. }
                            | updates::UpdateStatus::InstallFailed { .. } => {
                                start_update_download(hwnd);
                            }
                            updates::UpdateStatus::ReadyToInstall { exe_path, .. } => {
                                start_update_install(hwnd, &exe_path);
                            }
                            updates::UpdateStatus::Checking
                            | updates::UpdateStatus::Downloading => {}
                            _ => start_update_check(hwnd),
                        }
                    } else if gui_view::point_in_rect(&gui_view::UPDATE_PAGE_HIT, x, y) {
                        if let updates::UpdateStatus::Available { url, .. }
                        | updates::UpdateStatus::ReadyToInstall { url, .. }
                        | updates::UpdateStatus::InstallFailed { url, .. } =
                            update_status_snapshot()
                        {
                            if !url.is_empty() {
                                open_release_page(&url);
                            }
                        }
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }
                let snapshot = SHARED
                    .get()
                    .and_then(|slot| slot.lock().ok())
                    .and_then(|slot| slot.clone());
                if let Some(Snapshot::Data(info)) = snapshot {
                    if x >= 576 && x <= 964 && (244..=634).contains(&y) {
                        if x >= 895 && y <= 264 {
                            if let Some(mut h) = SENSOR_HISTORY.get().and_then(|s| s.lock().ok()) {
                                h.reset();
                            }
                        }
                    } else if let Some(sensor) = gui_view::sensor_at_point(&info, x, y) {
                        if let Some(mut h) = SENSOR_HISTORY.get().and_then(|s| s.lock().ok()) {
                            h.select(&sensor);
                        }
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
                0
            }
            WM_LBUTTONDBLCLK => {
                if FEEDBACK_VISIBLE.load(Ordering::Acquire) || ABOUT_VISIBLE.load(Ordering::Acquire)
                {
                    return 0;
                }
                let (x, y) = logical_mouse_point(hwnd, lparam);
                let snapshot = SHARED
                    .get()
                    .and_then(|slot| slot.lock().ok())
                    .and_then(|slot| slot.clone());
                if let Some(Snapshot::Data(info)) = snapshot {
                    if let Some(sensor) = gui_view::sensor_at_point(&info, x, y) {
                        if let Some(mut h) = SENSOR_HISTORY.get().and_then(|s| s.lock().ok()) {
                            h.select_maximum(&sensor);
                        }
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
                0
            }
            WM_ERASEBKGND => 1,
            WM_CTLCOLOREDIT => {
                let control_hdc = _wparam as HDC;
                windows_sys::Win32::Graphics::Gdi::SetTextColor(
                    control_hdc,
                    gui_view::rgb(255, 255, 255),
                );
                windows_sys::Win32::Graphics::Gdi::SetBkColor(
                    control_hdc,
                    gui_view::rgb(36, 36, 36),
                );
                *EDIT_BACKGROUND_BRUSH.get_or_init(|| CreateSolidBrush(gui_view::rgb(36, 36, 36)))
            }
            WM_DPICHANGED => {
                let suggested = &*(lparam as *const RECT);
                SetWindowPos(
                    hwnd,
                    0,
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
                layout_feedback_controls(hwnd);
                InvalidateRect(hwnd, std::ptr::null(), 0);
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                if let Some(tx) = STOP_TX.get() {
                    let _ = tx.send(());
                }
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, _wparam, lparam),
        }
    }
}

fn worker_main(hwnd: HWND, shared: Arc<Mutex<Option<Snapshot>>>, stop: mpsc::Receiver<()>) {
    let library = match load_driver_library() {
        Ok(library) => library,
        Err(error) => {
            if let Ok(mut slot) = shared.lock() {
                *slot = Some(Snapshot::LoadFailed(error));
            }
            unsafe {
                PostMessageW(hwnd, WM_APP_SNAPSHOT, 0, 0);
            }
            return;
        }
    };
    loop {
        let snapshot = match fetch_data_from_dll(&library) {
            Ok(info) => {
                if let Some(mut h) = SENSOR_HISTORY.get().and_then(|s| s.lock().ok()) {
                    let sensors = gui_view::ordered_sensors(&info);
                    h.record(&sensors);
                }
                Snapshot::Data(info)
            }
            Err(error) => Snapshot::FetchError(error.to_string()),
        };
        if let Ok(mut slot) = shared.lock() {
            *slot = Some(snapshot);
        }
        unsafe {
            PostMessageW(hwnd, WM_APP_SNAPSHOT, 0, 0);
        }
        if stop
            .recv_timeout(Duration::from_millis(gui_settings::refresh_interval_ms()))
            .is_ok()
        {
            break;
        }
    }
}

fn main() {
    gui_settings::initialize();
    updates::cleanup_old_install();
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wstr("GpuSharkMonitorWindow");
        let resource_icon = LoadIconW(instance, 1usize as *const u16);
        let window_icon = if resource_icon != 0 {
            resource_icon
        } else {
            LoadIconW(0, IDI_APPLICATION)
        };
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hIcon: window_icon,
            hCursor: LoadCursorW(0, IDC_ARROW),
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        let title = wstr(&format!("GPU Shark {}", env!("CARGO_PKG_VERSION")));
        let style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let dpi = GetDpiForSystem().max(gui_layout::DEFAULT_DPI);
        let mut base_window_rect = RECT {
            left: 0,
            top: 0,
            right: gui_layout::BASE_CLIENT_WIDTH,
            bottom: gui_layout::BASE_CLIENT_HEIGHT,
        };
        AdjustWindowRectEx(&mut base_window_rect, style, 0, 0);
        let window_width =
            gui_layout::scale_for_dpi(base_window_rect.right - base_window_rect.left, dpi);
        let window_height =
            gui_layout::scale_for_dpi(base_window_rect.bottom - base_window_rect.top, dpi);
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            window_width,
            window_height,
            0,
            0,
            instance,
            std::ptr::null(),
        );
        if hwnd == 0 {
            return;
        }
        let icon = window_icon;
        if icon != 0 {
            windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(
                hwnd,
                WM_SETICON,
                ICON_SMALL as usize,
                icon,
            );
        }
        let dark: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const i32).cast(),
            std::mem::size_of_val(&dark) as u32,
        );
        let shared = Arc::new(Mutex::new(None));
        let (tx, rx) = mpsc::channel();
        let _ = SHARED.set(shared.clone());
        let _ = STOP_TX.set(tx);
        let _ = SENSOR_HISTORY.set(Mutex::new(gui_state::SensorHistory::default()));
        let _ = FEEDBACK_STATUS.set(Mutex::new(FeedbackStatus::default()));
        let _ = HEADER_NAV.set(Mutex::new(gui_view::HeaderNavLayout {
            about: (0, 0),
            settings: (0, 0),
            feedback: (0, 0),
        }));
        let _ = UPDATE_STATUS.set(Mutex::new(updates::UpdateStatus::Idle));
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        if gui_settings::current().check_updates {
            start_update_check(hwnd);
        }
        let worker = thread::Builder::new()
            .name("gpu-shark-telemetry".into())
            .spawn(move || worker_main(hwnd, shared, rx))
            .expect("telemetry worker");
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if let Some(tx) = STOP_TX.get() {
            let _ = tx.send(());
        }
        let _ = worker.join();
    }
}
