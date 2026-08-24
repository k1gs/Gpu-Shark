//! Public native Win32 front end. Hardware polling stays on a worker thread;
//! this file owns only window lifetime and message dispatch. Drawing and sensor
//! history deliberately live in separate modules to keep UI changes isolated.
#![windows_subsystem = "windows"]

mod feedback;
mod gui_i18n;
mod gui_state;
mod gui_view;
mod sensor_model;

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
    CreateCompatibleDC, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH, DeleteDC, DeleteObject,
    EndPaint, FF_DONTCARE, FW_BOLD, FW_NORMAL, HDC, InvalidateRect, OUT_DEFAULT_PRECIS,
    PAINTSTRUCT, SRCCOPY, SelectObject, SetBkMode, TRANSPARENT, UpdateWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DestroyWindow, DispatchMessageW, ES_AUTOVSCROLL, ES_MULTILINE, GetClientRect, GetMessageW,
    GetWindowTextW, ICON_SMALL, IDC_ARROW, IDI_APPLICATION, LoadCursorW, LoadIconW, MSG,
    PostMessageW, PostQuitMessage, RegisterClassW, SW_SHOW, ShowWindow, TranslateMessage, WM_APP,
    WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_PAINT, WM_SETICON,
    WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_MINIMIZEBOX, WS_SYSMENU, WS_VISIBLE, WS_VSCROLL,
};

const WM_APP_SNAPSHOT: u32 = WM_APP + 1;
const WM_APP_FEEDBACK: u32 = WM_APP + 2;
const WINDOW_WIDTH: i32 = 980;
const WINDOW_HEIGHT: i32 = 660;
static SHARED: OnceLock<Arc<Mutex<Option<Snapshot>>>> = OnceLock::new();
static STOP_TX: OnceLock<mpsc::Sender<()>> = OnceLock::new();
static SENSOR_HISTORY: OnceLock<Mutex<gui_state::SensorHistory>> = OnceLock::new();
static FEEDBACK_VISIBLE: AtomicBool = AtomicBool::new(false);
static FEEDBACK_EDIT: AtomicIsize = AtomicIsize::new(0);
static FEEDBACK_CONTACT: AtomicIsize = AtomicIsize::new(0);
static FEEDBACK_STATUS: OnceLock<Mutex<String>> = OnceLock::new();
static FEEDBACK_CONSENT: AtomicBool = AtomicBool::new(false);
static FEEDBACK_SENDING: AtomicBool = AtomicBool::new(false);
static RUSSIAN_UI: AtomicBool = AtomicBool::new(true);

#[derive(Clone)]
enum Snapshot {
    Data(SysInfo),
    LoadFailed(String),
    FetchError(String),
}

fn wstr(text: &str) -> Vec<u16> {
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
                36,
                285,
                900,
                105,
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
                180,
                225,
                420,
                26,
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
        InvalidateRect(hwnd, std::ptr::null(), 0);
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
                    *value = "Введите сообщение и подтвердите согласие на отправку.".into();
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
        let locale = if RUSSIAN_UI.load(Ordering::Acquire) {
            "ru"
        } else {
            "en"
        };
        if let Some(slot) = FEEDBACK_STATUS.get() {
            if let Ok(mut value) = slot.lock() {
                *value = "Отправка…".into();
            }
        }
        InvalidateRect(hwnd, std::ptr::null(), 0);
        thread::spawn(move || {
            let result = feedback::submit(info.as_ref(), &note, Some(&contact), locale, consent);
            let status = match result {
                Ok(report_id) => format!("Отчёт принят. Номер: {report_id}"),
                Err(feedback::SubmitError::InvalidPayload(detail)) => {
                    format!("Отчёт отклонён: {detail}")
                }
                Err(feedback::SubmitError::PayloadTooLarge) => {
                    "Отчёт превышает 256 КБ (413).".into()
                }
                Err(feedback::SubmitError::RateLimited) => {
                    "Лимит исчерпан. Попробуйте позднее (429).".into()
                }
                Err(feedback::SubmitError::Server) => {
                    "Временная ошибка сервера. Попробуйте позднее.".into()
                }
                Err(feedback::SubmitError::Network(error)) => format!("Ошибка сети: {error}"),
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
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut client);
        // Draw the complete frame off-screen, then copy it in one operation.
        // This removes the visible background flash caused by 1 Hz updates.
        let hdc = CreateCompatibleDC(output_hdc);
        let bitmap = CreateCompatibleBitmap(output_hdc, client.right, client.bottom);
        let old_bitmap = SelectObject(hdc, bitmap);
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
        let language = gui_i18n::Language::from_russian(RUSSIAN_UI.load(Ordering::Acquire));
        let name = match &snapshot {
            Some(Snapshot::Data(info)) => info.gpu_name.as_deref().unwrap_or("Unknown GPU"),
            _ => "GPU Shark",
        };
        let previous = SelectObject(hdc, bold_font);
        draw_text(hdc, 16, 18, "GPU SHARK", gui_view::accent());
        draw_text(
            hdc,
            client.right - 145,
            18,
            language.text(gui_i18n::Key::Feedback),
            gui_view::accent(),
        );
        draw_text(
            hdc,
            client.right - 215,
            18,
            if RUSSIAN_UI.load(Ordering::Acquire) {
                "RU"
            } else {
                "EN"
            },
            gui_view::accent(),
        );
        if matches!(&snapshot, Some(Snapshot::Data(info)) if info.gpu_name.is_none()) {
            draw_text(
                hdc,
                16,
                102,
                language.text(gui_i18n::Key::UnknownGpu),
                gui_view::rgb(246, 211, 45),
            );
        }
        draw_text(hdc, 16, 51, name, gui_view::rgb(255, 255, 255));
        SelectObject(hdc, previous);
        draw_text(
            hdc,
            16,
            78,
            language.text(gui_i18n::Key::RefreshHint),
            gui_view::rgb(190, 190, 190),
        );
        if let Some(reason) = snapshot.as_ref().and_then(|snapshot| match snapshot {
            Snapshot::Data(info) => info.perfcap_reason.as_deref(),
            _ => None,
        }) {
            draw_text(
                hdc,
                576,
                78,
                &format!("PerfCap: {reason}"),
                gui_view::rgb(190, 190, 190),
            );
        }
        if FEEDBACK_VISIBLE.load(Ordering::Acquire) {
            let status = FEEDBACK_STATUS
                .get()
                .and_then(|slot| slot.lock().ok())
                .map(|value| value.clone())
                .unwrap_or_default();
            gui_view::draw_feedback_form(
                hdc,
                &client,
                &status,
                language,
                FEEDBACK_CONSENT.load(Ordering::Acquire),
                FEEDBACK_SENDING.load(Ordering::Acquire),
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
        BitBlt(
            output_hdc,
            0,
            0,
            client.right,
            client.bottom,
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
            WM_PAINT => {
                paint(hwnd);
                0
            }
            WM_LBUTTONDOWN => {
                let x = (lparam & 0xffff) as i32;
                let y = ((lparam >> 16) & 0xffff) as i32;
                if x >= 800 && y <= 54 {
                    show_feedback(hwnd);
                    return 0;
                }
                if (730..800).contains(&x) && y <= 54 {
                    RUSSIAN_UI.fetch_xor(true, Ordering::AcqRel);
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    return 0;
                }
                if FEEDBACK_VISIBLE.load(Ordering::Acquire) {
                    if x >= 30 && x <= 720 && (400..=438).contains(&y) {
                        FEEDBACK_CONSENT.fetch_xor(true, Ordering::AcqRel);
                    } else if x >= 30 && x <= 480 && (445..=490).contains(&y) {
                        submit_feedback(hwnd);
                    }
                    if x >= 850 && y <= 170 {
                        FEEDBACK_VISIBLE.store(false, Ordering::Release);
                        let edit = FEEDBACK_EDIT.load(Ordering::Acquire);
                        if edit != 0 {
                            ShowWindow(edit, 0);
                        }
                        let contact = FEEDBACK_CONTACT.load(Ordering::Acquire);
                        if contact != 0 {
                            ShowWindow(contact, 0);
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
                let x = (lparam & 0xffff) as i32;
                let y = ((lparam >> 16) & 0xffff) as i32;
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
                    h.record(&info.sensors);
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
        if stop.recv_timeout(Duration::from_secs(1)).is_ok() {
            break;
        }
    }
}

fn main() {
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
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
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
        let _ = FEEDBACK_STATUS.set(Mutex::new(String::new()));
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
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
