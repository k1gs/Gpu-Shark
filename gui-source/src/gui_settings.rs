use crate::gui_layout;
use crate::gui_view;
use crate::settings::{self, AccentTheme, AppSettings, UiLanguage};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_USE_IMMERSIVE_DARK_MODE, DwmGetColorizationColor, DwmSetWindowAttribute,
};
use windows_sys::Win32::Graphics::Gdi::{
    CreateSolidBrush, DEFAULT_GUI_FONT, GetStockObject, HBRUSH, SetBkColor, SetTextColor,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, BM_GETCHECK, BM_SETCHECK, BN_CLICKED, BS_AUTOCHECKBOX, BS_PUSHBUTTON,
    CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBS_DROPDOWNLIST, CS_HREDRAW, CS_VREDRAW,
    CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow, GW_OWNER, GetClientRect,
    GetWindow, ICON_SMALL, IDC_ARROW, IDI_APPLICATION, LoadCursorW, LoadIconW, MoveWindow,
    RegisterClassW, SW_RESTORE, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW, SetForegroundWindow,
    SetWindowPos, SetWindowTextW, ShowWindow, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORSTATIC,
    WM_DESTROY, WM_DPICHANGED, WM_SETFONT, WM_SETICON, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_SYSMENU,
    WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

const CLASS_NAME: &str = "GpuSharkSettingsWindow";
const CLIENT_WIDTH: i32 = 520;
const CLIENT_HEIGHT: i32 = 380;
const ID_APPLY: usize = 1001;
const ID_CANCEL: usize = 1002;
const ID_RESTORE: usize = 1003;

static CURRENT: OnceLock<RwLock<AppSettings>> = OnceLock::new();
static STATUS: OnceLock<Mutex<String>> = OnceLock::new();
static SETTINGS_WINDOW: AtomicIsize = AtomicIsize::new(0);
static LANGUAGE_COMBO: AtomicIsize = AtomicIsize::new(0);
static REFRESH_COMBO: AtomicIsize = AtomicIsize::new(0);
static ACCENT_COMBO: AtomicIsize = AtomicIsize::new(0);
static CHECK_UPDATES_BOX: AtomicIsize = AtomicIsize::new(0);
static STATUS_LABEL: AtomicIsize = AtomicIsize::new(0);
static BACKGROUND_BRUSH: AtomicIsize = AtomicIsize::new(0);
static CONTROL_HANDLES: OnceLock<Mutex<Vec<HWND>>> = OnceLock::new();

pub fn initialize() {
    let outcome = settings::load();
    apply_accent(outcome.settings.accent);
    let _ = CURRENT.set(RwLock::new(outcome.settings));
    let _ = STATUS.set(Mutex::new(outcome.warning.unwrap_or_default()));
}

pub fn current() -> AppSettings {
    CURRENT
        .get()
        .and_then(|settings| settings.read().ok())
        .map(|settings| settings.clone())
        .unwrap_or_default()
}

pub fn is_russian() -> bool {
    current().language == UiLanguage::Russian
}

pub fn refresh_interval_ms() -> u64 {
    current().refresh_interval_ms
}

pub unsafe fn show(owner: HWND) {
    unsafe {
        let existing = SETTINGS_WINDOW.load(Ordering::Acquire);
        if existing != 0 {
            ShowWindow(existing, SW_RESTORE);
            SetForegroundWindow(existing);
            return;
        }

        let instance = GetModuleHandleW(std::ptr::null());
        let class = wstr(CLASS_NAME);
        let resource_icon = LoadIconW(instance, 1usize as *const u16);
        let icon = if resource_icon != 0 {
            resource_icon
        } else {
            LoadIconW(0, IDI_APPLICATION)
        };
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_wnd_proc),
            hInstance: instance,
            hIcon: icon,
            hCursor: LoadCursorW(0, IDC_ARROW),
            hbrBackground: background_brush(),
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);

        let selected = current();
        let title = if selected.language == UiLanguage::Russian {
            "Настройки GPU Shark"
        } else {
            "GPU Shark Settings"
        };
        let style = WS_CAPTION | WS_SYSMENU;
        let dpi = GetDpiForWindow(owner).max(gui_layout::DEFAULT_DPI);
        let mut base_rect = RECT {
            left: 0,
            top: 0,
            right: CLIENT_WIDTH,
            bottom: CLIENT_HEIGHT,
        };
        AdjustWindowRectEx(&mut base_rect, style, 0, 0);
        let width = gui_layout::scale_for_dpi(base_rect.right - base_rect.left, dpi);
        let height = gui_layout::scale_for_dpi(base_rect.bottom - base_rect.top, dpi);
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wstr(title).as_ptr(),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            width,
            height,
            owner,
            0,
            instance,
            std::ptr::null(),
        );
        if hwnd == 0 {
            return;
        }
        SETTINGS_WINDOW.store(hwnd, Ordering::Release);
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, icon);
        let dark: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const i32).cast(),
            std::mem::size_of_val(&dark) as u32,
        );
        ShowWindow(hwnd, windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW);
        SetForegroundWindow(hwnd);
    }
}

unsafe extern "system" fn settings_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                create_controls(hwnd);
                populate_controls(&current());
                layout_controls(hwnd);
                0
            }
            WM_COMMAND => {
                let id = wparam & 0xffff;
                let notification = ((wparam >> 16) & 0xffff) as u32;
                if notification != BN_CLICKED {
                    return 0;
                }
                match id {
                    ID_APPLY => apply_from_controls(hwnd),
                    ID_CANCEL => {
                        DestroyWindow(hwnd);
                    }
                    ID_RESTORE => {
                        populate_controls(&AppSettings::default());
                        set_status("Defaults selected. Click Apply to save them.");
                        refresh_status_label();
                    }
                    _ => {}
                }
                0
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
                layout_controls(hwnd);
                0
            }
            WM_CTLCOLORSTATIC => {
                let hdc = wparam as isize;
                SetTextColor(hdc, gui_view::rgb(224, 224, 224));
                SetBkColor(hdc, gui_view::BACKGROUND);
                background_brush() as LRESULT
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                SETTINGS_WINDOW.store(0, Ordering::Release);
                LANGUAGE_COMBO.store(0, Ordering::Release);
                REFRESH_COMBO.store(0, Ordering::Release);
                ACCENT_COMBO.store(0, Ordering::Release);
                CHECK_UPDATES_BOX.store(0, Ordering::Release);
                STATUS_LABEL.store(0, Ordering::Release);
                if let Some(handles) = CONTROL_HANDLES.get() {
                    if let Ok(mut handles) = handles.lock() {
                        handles.clear();
                    }
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn create_controls(hwnd: HWND) {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let selected = current();
        let russian = selected.language == UiLanguage::Russian;
        let labels = if russian {
            [
                "НАСТРОЙКИ",
                "Язык интерфейса",
                "Частота обновления",
                "Акцентный цвет",
                "Температура: Цельсий",
            ]
        } else {
            [
                "SETTINGS",
                "Interface language",
                "Refresh interval",
                "Accent color",
                "Temperature: Celsius",
            ]
        };
        let mut controls = Vec::new();
        for label in labels {
            controls.push(CreateWindowExW(
                0,
                wstr("STATIC").as_ptr(),
                wstr(label).as_ptr(),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                0,
                0,
                hwnd,
                0,
                instance,
                std::ptr::null(),
            ));
        }
        let language = combo(hwnd, instance);
        let refresh = combo(hwnd, instance);
        let accent = combo(hwnd, instance);
        LANGUAGE_COMBO.store(language, Ordering::Release);
        REFRESH_COMBO.store(refresh, Ordering::Release);
        ACCENT_COMBO.store(accent, Ordering::Release);

        let status = CreateWindowExW(
            0,
            wstr("STATIC").as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            0,
            0,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        STATUS_LABEL.store(status, Ordering::Release);
        controls.extend([language, refresh, accent, status]);

        let button_labels = if russian {
            ["СБРОСИТЬ", "ОТМЕНА", "ПРИМЕНИТЬ"]
        } else {
            ["RESTORE DEFAULTS", "CANCEL", "APPLY"]
        };
        for (id, label) in [ID_RESTORE, ID_CANCEL, ID_APPLY]
            .into_iter()
            .zip(button_labels)
        {
            controls.push(CreateWindowExW(
                0,
                wstr("BUTTON").as_ptr(),
                wstr(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32),
                0,
                0,
                0,
                0,
                hwnd,
                id as isize,
                instance,
                std::ptr::null(),
            ));
        }
        let font = GetStockObject(DEFAULT_GUI_FONT);
        for &control in &controls {
            if control != 0 {
                SendMessageW(control, WM_SETFONT, font as usize, 1);
            }
        }
        let checkbox = CreateWindowExW(
            0,
            wstr("BUTTON").as_ptr(),
            wstr(if russian {
                "Проверять обновления при запуске"
            } else {
                "Check for updates at startup"
            })
            .as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_AUTOCHECKBOX as u32),
            0,
            0,
            0,
            0,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        CHECK_UPDATES_BOX.store(checkbox, Ordering::Release);
        if checkbox != 0 {
            SendMessageW(checkbox, WM_SETFONT, font as usize, 1);
        }
        let handles = CONTROL_HANDLES.get_or_init(|| Mutex::new(Vec::new()));
        if let Ok(mut handles) = handles.lock() {
            *handles = controls;
            handles.push(checkbox);
        }
        refresh_status_label();
    }
}

unsafe fn combo(parent: HWND, instance: isize) -> HWND {
    unsafe {
        CreateWindowExW(
            0,
            wstr("COMBOBOX").as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | (CBS_DROPDOWNLIST as u32),
            0,
            0,
            0,
            0,
            parent,
            0,
            instance,
            std::ptr::null(),
        )
    }
}

unsafe fn layout_controls(hwnd: HWND) {
    unsafe {
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut client);
        let sx = |value| gui_layout::scale_to_client(value, CLIENT_WIDTH, client.right);
        let sy = |value| gui_layout::scale_to_client(value, CLIENT_HEIGHT, client.bottom);
        let children = CONTROL_HANDLES
            .get()
            .and_then(|handles| handles.lock().ok())
            .map(|handles| handles.clone())
            .unwrap_or_default();
        if children.len() < 13 {
            return;
        }
        MoveWindow(children[0], sx(24), sy(18), sx(472), sy(28), 1);
        MoveWindow(children[1], sx(24), sy(72), sx(180), sy(24), 1);
        MoveWindow(children[2], sx(24), sy(122), sx(180), sy(24), 1);
        MoveWindow(children[3], sx(24), sy(172), sx(180), sy(24), 1);
        MoveWindow(children[4], sx(24), sy(224), sx(472), sy(24), 1);
        MoveWindow(children[5], sx(220), sy(64), sx(276), sy(170), 1);
        MoveWindow(children[6], sx(220), sy(114), sx(276), sy(170), 1);
        MoveWindow(children[7], sx(220), sy(164), sx(276), sy(210), 1);
        MoveWindow(children[8], sx(24), sy(292), sx(472), sy(30), 1);
        MoveWindow(children[9], sx(24), sy(326), sx(180), sy(32), 1);
        MoveWindow(children[10], sx(292), sy(326), sx(96), sy(32), 1);
        MoveWindow(children[11], sx(400), sy(326), sx(96), sy(32), 1);
        MoveWindow(children[12], sx(24), sy(254), sx(472), sy(30), 1);
    }
}

unsafe fn populate_controls(settings: &AppSettings) {
    unsafe {
        let language = LANGUAGE_COMBO.load(Ordering::Acquire);
        let refresh = REFRESH_COMBO.load(Ordering::Acquire);
        let accent = ACCENT_COMBO.load(Ordering::Acquire);
        reset_combo(language, &["Русский", "English"]);
        reset_combo(refresh, &["500 ms", "1 s", "2 s"]);
        reset_combo(accent, &["Green", "Blue", "Purple", "Orange", "Windows"]);
        SendMessageW(
            language,
            CB_SETCURSEL,
            usize::from(settings.language == UiLanguage::English),
            0,
        );
        let refresh_index = match settings.refresh_interval_ms {
            500 => 0,
            2_000 => 2,
            _ => 1,
        };
        SendMessageW(refresh, CB_SETCURSEL, refresh_index, 0);
        let accent_index = match settings.accent {
            AccentTheme::Green => 0,
            AccentTheme::Blue => 1,
            AccentTheme::Purple => 2,
            AccentTheme::Orange => 3,
            AccentTheme::Windows => 4,
        };
        SendMessageW(accent, CB_SETCURSEL, accent_index, 0);
        let checkbox = CHECK_UPDATES_BOX.load(Ordering::Acquire);
        if checkbox != 0 {
            SendMessageW(
                checkbox,
                BM_SETCHECK,
                usize::from(settings.check_updates),
                0,
            );
        }
    }
}

unsafe fn reset_combo(hwnd: HWND, values: &[&str]) {
    unsafe {
        SendMessageW(
            hwnd,
            windows_sys::Win32::UI::WindowsAndMessaging::CB_RESETCONTENT,
            0,
            0,
        );
        for value in values {
            SendMessageW(hwnd, CB_ADDSTRING, 0, wstr(value).as_ptr() as isize);
        }
    }
}

unsafe fn settings_from_controls() -> AppSettings {
    unsafe {
        let language =
            match SendMessageW(LANGUAGE_COMBO.load(Ordering::Acquire), CB_GETCURSEL, 0, 0) {
                1 => UiLanguage::English,
                _ => UiLanguage::Russian,
            };
        let refresh_interval_ms =
            match SendMessageW(REFRESH_COMBO.load(Ordering::Acquire), CB_GETCURSEL, 0, 0) {
                0 => 500,
                2 => 2_000,
                _ => 1_000,
            };
        let accent = match SendMessageW(ACCENT_COMBO.load(Ordering::Acquire), CB_GETCURSEL, 0, 0) {
            1 => AccentTheme::Blue,
            2 => AccentTheme::Purple,
            3 => AccentTheme::Orange,
            4 => AccentTheme::Windows,
            _ => AccentTheme::Green,
        };
        let check_updates =
            SendMessageW(CHECK_UPDATES_BOX.load(Ordering::Acquire), BM_GETCHECK, 0, 0) == 1;
        AppSettings {
            language,
            refresh_interval_ms,
            accent,
            check_updates,
            ..AppSettings::default()
        }
    }
}

unsafe fn apply_from_controls(hwnd: HWND) {
    unsafe {
        let selected = settings_from_controls();
        match settings::save(&selected) {
            Ok(()) => {
                if let Some(slot) = CURRENT.get() {
                    if let Ok(mut current) = slot.write() {
                        *current = selected.clone();
                    }
                }
                apply_accent(selected.accent);
                set_status("");
                let owner = GetWindow(hwnd, GW_OWNER);
                if owner != 0 {
                    windows_sys::Win32::Graphics::Gdi::InvalidateRect(owner, std::ptr::null(), 0);
                }
                DestroyWindow(hwnd);
            }
            Err(error) => {
                let prefix = if selected.language == UiLanguage::Russian {
                    "Не удалось сохранить настройки"
                } else {
                    "Could not save settings"
                };
                set_status(&format!("{prefix}: {error}"));
                refresh_status_label();
            }
        }
    }
}

fn apply_accent(theme: AccentTheme) {
    gui_view::set_accent(accent_color(theme));
}

fn accent_color(theme: AccentTheme) -> u32 {
    match theme {
        AccentTheme::Green => gui_view::rgb(87, 227, 137),
        AccentTheme::Blue => gui_view::rgb(96, 172, 255),
        AccentTheme::Purple => gui_view::rgb(199, 145, 255),
        AccentTheme::Orange => gui_view::rgb(255, 178, 94),
        AccentTheme::Windows => windows_accent(),
    }
}

fn windows_accent() -> u32 {
    unsafe {
        let mut raw = 0u32;
        let mut opaque = 0;
        if DwmGetColorizationColor(&mut raw, &mut opaque) < 0 {
            return gui_view::rgb(87, 227, 137);
        }
        let mut red = ((raw >> 16) & 0xff) as u8;
        let mut green = ((raw >> 8) & 0xff) as u8;
        let mut blue = (raw & 0xff) as u8;
        let luminance = |r: u8, g: u8, b: u8| {
            (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1_000
        };
        while luminance(red, green, blue) < 145 {
            red = ((u16::from(red) + 255) / 2) as u8;
            green = ((u16::from(green) + 255) / 2) as u8;
            blue = ((u16::from(blue) + 255) / 2) as u8;
        }
        gui_view::rgb(red, green, blue)
    }
}

fn set_status(value: &str) {
    if let Some(status) = STATUS.get() {
        if let Ok(mut status) = status.lock() {
            *status = value.to_owned();
        }
    }
}

unsafe fn refresh_status_label() {
    unsafe {
        let label = STATUS_LABEL.load(Ordering::Acquire);
        if label == 0 {
            return;
        }
        let value = STATUS
            .get()
            .and_then(|status| status.lock().ok())
            .map(|status| status.clone())
            .unwrap_or_default();
        SetWindowTextW(label, wstr(&value).as_ptr());
    }
}

fn background_brush() -> HBRUSH {
    let existing = BACKGROUND_BRUSH.load(Ordering::Acquire);
    if existing != 0 {
        return existing;
    }
    let created = unsafe { CreateSolidBrush(gui_view::BACKGROUND) };
    match BACKGROUND_BRUSH.compare_exchange(0, created, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => created,
        Err(existing) => existing,
    }
}

fn wstr(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
