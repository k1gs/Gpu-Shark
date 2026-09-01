use crate::gui_i18n::{Key, Language};
use crate::gui_state::SensorHistory;
use crate::sensor_model::{SensorGroup, SensorKind, metadata, sensor_id};
use gpu_shark::{SensorReading, SysInfo};
use std::sync::atomic::{AtomicU32, Ordering};
use windows_sys::Win32::Foundation::{RECT, SIZE};
use windows_sys::Win32::Graphics::Gdi::{
    CreatePen, CreateSolidBrush, DeleteObject, FillRect, GetTextExtentPoint32W, HDC, LineTo,
    MoveToEx, PS_SOLID, SelectObject, SetTextColor, TextOutW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{DI_NORMAL, DrawIconEx, HICON};

pub const BACKGROUND: u32 = rgb(30, 30, 30);
static ACCENT_COLOR: AtomicU32 = AtomicU32::new(rgb(87, 227, 137));
const SURFACE: u32 = rgb(36, 36, 36);
const SURFACE_RAISED: u32 = rgb(48, 48, 48);
const DIVIDER: u32 = rgb(72, 72, 72);
const LABEL: u32 = rgb(190, 190, 190);
const VALUE: u32 = rgb(255, 255, 255);
const MUTED: u32 = rgb(145, 145, 145);
const AMBER: u32 = rgb(246, 211, 45);
const RED: u32 = rgb(237, 51, 59);

const LIST_LEFT: i32 = 16;
const LIST_TOP: i32 = 124;
const LIST_WIDTH: i32 = 548;
const ROW_HEIGHT: i32 = 23;
const MAX_ROWS: usize = 14;

pub const FEEDBACK_CONTACT_EDIT: RECT = RECT {
    left: 36,
    top: 244,
    right: 936,
    bottom: 272,
};
pub const FEEDBACK_MESSAGE_EDIT: RECT = RECT {
    left: 36,
    top: 310,
    right: 936,
    bottom: 420,
};
pub const FEEDBACK_CONSENT_HIT: RECT = RECT {
    left: 30,
    top: 430,
    right: 720,
    bottom: 472,
};
pub const FEEDBACK_SUBMIT_HIT: RECT = RECT {
    left: 30,
    top: 476,
    right: 480,
    bottom: 518,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderNavLayout {
    pub about: (i32, i32),
    pub settings: (i32, i32),
    pub feedback: (i32, i32),
}

pub const HEADER_NAV_GAP: i32 = 24;
const HEADER_NAV_RIGHT_MARGIN: i32 = 16;

pub fn header_nav_layout(
    client_right: i32,
    about_width: i32,
    settings_width: i32,
    feedback_width: i32,
) -> HeaderNavLayout {
    let feedback_right = client_right - HEADER_NAV_RIGHT_MARGIN;
    let feedback_left = feedback_right - feedback_width;
    let settings_right = feedback_left - HEADER_NAV_GAP;
    let settings_left = settings_right - settings_width;
    let about_right = settings_left - HEADER_NAV_GAP;
    let about_left = about_right - about_width;
    HeaderNavLayout {
        about: (about_left, about_right),
        settings: (settings_left, settings_right),
        feedback: (feedback_left, feedback_right),
    }
}

pub const fn header_link_contains(link: (i32, i32), x: i32) -> bool {
    const HIT_PADDING: i32 = 8;
    x >= link.0 - HIT_PADDING && x <= link.1 + HIT_PADDING
}

pub const fn point_in_rect(rect: &RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

pub fn accent() -> u32 {
    ACCENT_COLOR.load(Ordering::Acquire)
}

pub fn set_accent(color: u32) {
    ACCENT_COLOR.store(color, Ordering::Release);
}

pub fn ordered_sensors(info: &SysInfo) -> Vec<SensorReading> {
    let mut sensors = info.sensors.clone();
    if let Some(reason) = info
        .perfcap_reason
        .as_deref()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
    {
        sensors.retain(|sensor| metadata(sensor).kind != SensorKind::PerfCap);
        sensors.push(SensorReading {
            name: "PerfCap Reason".to_owned(),
            value: 0.0,
            unit: reason.to_owned(),
        });
    }
    sensors.retain(|sensor| metadata(sensor).visible);
    sensors.sort_by_key(|sensor| {
        let item = metadata(sensor);
        (item.group, item.priority, sensor.name.clone())
    });
    sensors
}

fn is_geforce_rtx_50_series(gpu_name: Option<&str>) -> bool {
    let Some(name) = gpu_name.map(str::to_ascii_lowercase) else {
        return false;
    };
    if !name.contains("geforce rtx") {
        return false;
    }
    let Some(model) = name.split("rtx ").nth(1) else {
        return false;
    };
    let digits = model
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    digits.len() >= 4 && digits.starts_with("50")
}

fn sensor_label(sensor: &SensorReading, info: &SysInfo, language: Language) -> String {
    if metadata(sensor).kind == SensorKind::HotspotTemperature
        && is_geforce_rtx_50_series(info.gpu_name.as_deref())
    {
        format!("{} ({})", sensor.name, language.text(Key::Beta))
    } else {
        sensor.name.clone()
    }
}

fn group_title(group: SensorGroup, language: Language) -> &'static str {
    match group {
        SensorGroup::Gpu => "GPU",
        SensorGroup::Activity => language.text(Key::GpuActivity),
        SensorGroup::System => language.text(Key::System),
    }
}

fn wstr(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn text(hdc: HDC, x: i32, y: i32, value: &str, color: u32) {
    let wide = wstr(value);
    unsafe {
        SetTextColor(hdc, color);
        TextOutW(hdc, x, y, wide.as_ptr(), (wide.len() - 1) as i32);
    }
}

pub fn text_width(hdc: HDC, value: &str) -> i32 {
    let wide = wstr(value);
    let mut size = SIZE { cx: 0, cy: 0 };
    unsafe {
        GetTextExtentPoint32W(hdc, wide.as_ptr(), (wide.len() - 1) as i32, &mut size);
    }
    size.cx
}

pub fn clipped(hdc: HDC, x: i32, y: i32, value: &str, color: u32, width: i32) {
    if text_width(hdc, value) <= width {
        text(hdc, x, y, value, color);
        return;
    }
    let mut end = value.len();
    while end > 0 && text_width(hdc, &format!("{}…", &value[..end])) > width {
        end = value[..end]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
    }
    if end > 0 {
        text(hdc, x, y, &format!("{}…", &value[..end]), color);
    }
}

fn fill(hdc: HDC, rect: &RECT, color: u32) {
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(hdc, rect, brush);
        DeleteObject(brush);
    }
}

fn divider(hdc: HDC, left: i32, top: i32, right: i32) {
    fill(
        hdc,
        &RECT {
            left,
            top,
            right,
            bottom: top + 1,
        },
        DIVIDER,
    );
}

fn sensor_color(sensor: &SensorReading) -> u32 {
    match metadata(sensor).kind {
        SensorKind::HotspotTemperature if sensor.value >= 90.0 => RED,
        SensorKind::GpuCoreTemperature if sensor.value >= 83.0 => RED,
        SensorKind::MemoryTemperature if sensor.value >= 80.0 => AMBER,
        _ => VALUE,
    }
}

fn sensor_value(sensor: &SensorReading) -> String {
    if metadata(sensor).kind == SensorKind::PerfCap
        && sensor.name.trim().eq_ignore_ascii_case("PerfCap Reason")
    {
        sensor.unit.clone()
    } else {
        format!("{:.1} {}", sensor.value, sensor.unit)
    }
}

pub fn sensor_at(info: &SysInfo, y: i32) -> Option<SensorReading> {
    let mut current_y = LIST_TOP + 35;
    let mut previous_group = None;
    for sensor in ordered_sensors(info).into_iter().take(MAX_ROWS) {
        let group = metadata(&sensor).group;
        if previous_group != Some(group) {
            current_y += 22;
            previous_group = Some(group);
        }
        if (current_y..current_y + ROW_HEIGHT).contains(&y) {
            return Some(sensor);
        }
        current_y += ROW_HEIGHT;
    }
    None
}

pub fn sensor_at_point(info: &SysInfo, x: i32, y: i32) -> Option<SensorReading> {
    if !(LIST_LEFT..LIST_LEFT + LIST_WIDTH).contains(&x) {
        return None;
    }
    sensor_at(info, y)
}

pub fn draw_dashboard(
    hdc: HDC,
    client: &RECT,
    info: &SysInfo,
    history: &SensorHistory,
    bold_font: isize,
    language: Language,
) {
    let list_right = LIST_LEFT + LIST_WIDTH;
    let panel_right = client.right - 16;
    let sensors = ordered_sensors(info);
    fill(
        hdc,
        &RECT {
            left: LIST_LEFT,
            top: LIST_TOP,
            right: list_right,
            bottom: client.bottom - 16,
        },
        SURFACE,
    );
    fill(
        hdc,
        &RECT {
            left: list_right + 12,
            top: LIST_TOP,
            right: panel_right,
            bottom: client.bottom - 16,
        },
        SURFACE,
    );
    text(
        hdc,
        LIST_LEFT + 16,
        LIST_TOP + 14,
        language.text(Key::Sensors),
        accent(),
    );
    text(
        hdc,
        list_right + 28,
        LIST_TOP + 14,
        language.text(Key::Selected),
        accent(),
    );
    divider(hdc, LIST_LEFT + 16, LIST_TOP + 34, list_right - 16);
    divider(hdc, list_right + 28, LIST_TOP + 34, panel_right - 16);

    let selected = history.selected_id();
    let mut y = LIST_TOP + 42;
    let mut previous_group = None;
    for sensor in sensors.iter().take(MAX_ROWS) {
        let group = metadata(sensor).group;
        if previous_group != Some(group) {
            if previous_group.is_some() {
                y += 3;
            }
            text(hdc, LIST_LEFT + 16, y, group_title(group, language), MUTED);
            y += 20;
            previous_group = Some(group);
        }
        if selected.is_some_and(|id| *id == sensor_id(sensor)) {
            fill(
                hdc,
                &RECT {
                    left: LIST_LEFT + 8,
                    top: y - 2,
                    right: list_right - 8,
                    bottom: y + 20,
                },
                SURFACE_RAISED,
            );
            fill(
                hdc,
                &RECT {
                    left: LIST_LEFT + 8,
                    top: y - 2,
                    right: LIST_LEFT + 11,
                    bottom: y + 20,
                },
                accent(),
            );
        }
        let label = sensor_label(sensor, info, language);
        clipped(hdc, LIST_LEFT + 20, y + 2, &label, LABEL, 310);
        let previous = unsafe { SelectObject(hdc, bold_font) };
        let value = sensor_value(sensor);
        let value_x = list_right - 20 - text_width(hdc, &value);
        text(hdc, value_x, y + 2, &value, sensor_color(sensor));
        unsafe {
            SelectObject(hdc, previous);
        }
        divider(hdc, LIST_LEFT + 20, y + 22, list_right - 20);
        y += ROW_HEIGHT;
    }
    if sensors.len() > MAX_ROWS {
        text(
            hdc,
            LIST_LEFT + 20,
            client.bottom - 39,
            "More available sensors will appear here as the list is expanded.",
            MUTED,
        );
    }
    draw_selected_panel(
        hdc,
        list_right + 12,
        LIST_TOP,
        panel_right,
        client.bottom - 16,
        info,
        history,
        bold_font,
        language,
    );
}

fn draw_selected_panel(
    hdc: HDC,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    info: &SysInfo,
    history: &SensorHistory,
    bold_font: isize,
    language: Language,
) {
    let sensors = ordered_sensors(info);
    let selected = history
        .selected_id()
        .and_then(|id| sensors.iter().find(|sensor| sensor_id(sensor) == *id));
    let Some(sensor) = selected else {
        text(
            hdc,
            left + 16,
            top + 64,
            language.text(Key::ChooseSensor),
            VALUE,
        );
        text(
            hdc,
            left + 16,
            top + 88,
            language.text(Key::ChooseSensorHint),
            MUTED,
        );
        return;
    };
    clipped(
        hdc,
        left + 16,
        top + 54,
        &sensor_label(sensor, info, language),
        VALUE,
        right - left - 32,
    );
    if metadata(sensor).kind == SensorKind::PerfCap {
        let value = sensor_value(sensor);
        let previous = unsafe { SelectObject(hdc, bold_font) };
        clipped(
            hdc,
            left + 16,
            top + 82,
            &value,
            accent(),
            right - left - 32,
        );
        unsafe {
            SelectObject(hdc, previous);
        }
        text(
            hdc,
            left + 16,
            top + 118,
            language.text(Key::PerfCapDetail),
            LABEL,
        );
        text(
            hdc,
            left + 16,
            top + 142,
            language.text(Key::PerfCapNoGraph),
            MUTED,
        );
        return;
    }
    let stats = history.stats().unwrap_or(crate::gui_state::SensorStats {
        current: sensor.value,
        min: sensor.value,
        max: sensor.value,
        avg: sensor.value,
    });
    let previous = unsafe { SelectObject(hdc, bold_font) };
    text(
        hdc,
        left + 16,
        top + 82,
        &format!(
            "{:.1} {}",
            if history.shows_maximum() {
                stats.max
            } else {
                stats.current
            },
            sensor.unit
        ),
        sensor_color(sensor),
    );
    unsafe {
        SelectObject(hdc, previous);
    }
    text(
        hdc,
        left + 16,
        top + 110,
        &format!("{}  {:.1}", language.text(Key::Min), stats.min),
        LABEL,
    );
    text(
        hdc,
        left + 16,
        top + 132,
        &format!("{}  {:.1}", language.text(Key::Max), stats.max),
        LABEL,
    );
    text(
        hdc,
        right - 63,
        top + 120,
        language.text(Key::Reset),
        accent(),
    );
    let graph = RECT {
        left: left + 16,
        top: top + 170,
        right: right - 16,
        bottom: bottom - 24,
    };
    fill(hdc, &graph, BACKGROUND);
    draw_graph(hdc, &graph, &history.samples(), stats.min, stats.max);
    text(
        hdc,
        graph.left + 8,
        graph.top + 8,
        language.text(Key::History),
        MUTED,
    );
}

fn draw_graph(hdc: HDC, rect: &RECT, samples: &[f32], min: f32, max: f32) {
    if samples.len() < 2 {
        return;
    }
    // Add visual headroom.  A 20% padded range prevents normal tiny changes
    // from looking like dramatic full-height spikes.
    let padding = ((max - min).abs() * 0.20).max(max.abs() * 0.03).max(0.5);
    let lower = min - padding;
    let span = (max + padding - lower).max(0.01);
    unsafe {
        let pen = CreatePen(PS_SOLID, 2, accent());
        let old = SelectObject(hdc, pen);
        for (index, value) in samples.iter().enumerate() {
            let x = rect.left
                + 8
                + ((rect.right - rect.left - 16) as usize * index / (samples.len() - 1)) as i32;
            let y = rect.bottom
                - 10
                - (((value - lower) / span) * (rect.bottom - rect.top - 28) as f32) as i32;
            if index == 0 {
                MoveToEx(hdc, x, y, std::ptr::null_mut());
            } else {
                LineTo(hdc, x, y);
            }
        }
        SelectObject(hdc, old);
        DeleteObject(pen);
    }
}

pub fn draw_unavailable(hdc: HDC, client: &RECT, detail: &str, language: Language) {
    fill(
        hdc,
        &RECT {
            left: 16,
            top: LIST_TOP,
            right: client.right - 16,
            bottom: client.bottom - 16,
        },
        SURFACE,
    );
    text(hdc, 36, LIST_TOP + 32, language.text(Key::Unavailable), RED);
    clipped(hdc, 36, LIST_TOP + 62, detail, LABEL, client.right - 72);
}

pub fn draw_feedback_form(
    hdc: HDC,
    client: &RECT,
    status: &str,
    language: Language,
    consent: bool,
    sending: bool,
) {
    fill(
        hdc,
        &RECT {
            left: 16,
            top: LIST_TOP,
            right: client.right - 16,
            bottom: client.bottom - 16,
        },
        SURFACE,
    );
    text(
        hdc,
        36,
        LIST_TOP + 28,
        language.text(Key::Feedback),
        accent(),
    );
    text(
        hdc,
        36,
        LIST_TOP + 62,
        language.text(Key::FeedbackPrivacy),
        LABEL,
    );
    text(
        hdc,
        36,
        LIST_TOP + 96,
        language.text(Key::FeedbackContact),
        LABEL,
    );
    text(
        hdc,
        36,
        LIST_TOP + 160,
        language.text(Key::FeedbackDescription),
        MUTED,
    );
    let consent_rect = RECT {
        left: 36,
        top: LIST_TOP + 314,
        right: 54,
        bottom: LIST_TOP + 332,
    };
    fill(hdc, &consent_rect, if consent { accent() } else { DIVIDER });
    if consent {
        text(hdc, 39, LIST_TOP + 313, "✓", BACKGROUND);
    }
    text(
        hdc,
        64,
        LIST_TOP + 314,
        language.text(Key::FeedbackConsent),
        LABEL,
    );
    text(
        hdc,
        36,
        LIST_TOP + 358,
        if sending {
            language.text(Key::FeedbackSending)
        } else {
            language.text(Key::FeedbackSubmit)
        },
        if consent && !sending { accent() } else { MUTED },
    );
    clipped(hdc, 36, LIST_TOP + 396, status, LABEL, client.right - 72);
    text(
        hdc,
        client.right - 95,
        LIST_TOP + 28,
        language.text(Key::Back),
        accent(),
    );
}

pub fn draw_about(hdc: HDC, client: &RECT, icon: HICON, language: Language, version: &str) {
    fill(
        hdc,
        &RECT {
            left: 16,
            top: LIST_TOP,
            right: client.right - 16,
            bottom: client.bottom - 16,
        },
        SURFACE,
    );
    text(hdc, 36, LIST_TOP + 28, language.text(Key::About), accent());
    text(
        hdc,
        client.right - 95,
        LIST_TOP + 28,
        language.text(Key::Back),
        accent(),
    );
    if icon != 0 {
        unsafe {
            DrawIconEx(hdc, 52, LIST_TOP + 82, icon, 128, 128, 0, 0, DI_NORMAL);
        }
    }
    text(hdc, 220, LIST_TOP + 88, "GPU Shark", VALUE);
    text(hdc, 220, LIST_TOP + 120, version, accent());
    text(
        hdc,
        220,
        LIST_TOP + 166,
        language.text(Key::AboutTagline),
        LABEL,
    );
    text(
        hdc,
        220,
        LIST_TOP + 198,
        language.text(Key::AboutReadOnly),
        LABEL,
    );
    text(
        hdc,
        220,
        LIST_TOP + 230,
        language.text(Key::AboutLicense),
        MUTED,
    );
    text(hdc, 220, LIST_TOP + 262, "© 2026 k1gs", MUTED);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(sensors: &[(&str, f32, &str)]) -> SysInfo {
        let sensors = sensors
            .iter()
            .map(|(name, value, unit)| {
                serde_json::json!({"name": name, "value": value, "unit": unit})
            })
            .collect::<Vec<_>>();
        serde_json::from_value(serde_json::json!({
            "gpu_name": "Visual fixture",
            "sensors": sensors
        }))
        .expect("safe visual fixture")
    }

    #[test]
    fn ordered_rows_hide_memory_clock_and_keep_expected_groups() {
        let mut snapshot = info(&[
            ("CPU Package", 50.0, "°C"),
            ("Memory Clock", 10501.0, "MHz"),
            ("D3D 3D", 90.0, "%"),
            ("GPU Core", 60.0, "°C"),
            ("PerfCap", 1.0, "%"),
        ]);
        snapshot.perfcap_reason = Some("Pwr, VRel".to_owned());

        let rows = ordered_sensors(&snapshot);
        let names = rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>();
        assert_eq!(
            names,
            ["GPU Core", "PerfCap Reason", "D3D 3D", "CPU Package"]
        );
        let perfcap = rows
            .iter()
            .find(|sensor| metadata(sensor).kind == SensorKind::PerfCap)
            .expect("categorical PerfCap row");
        assert_eq!(perfcap.unit, "Pwr, VRel");
        assert_eq!(
            rows.iter()
                .filter(|sensor| metadata(sensor).kind == SensorKind::PerfCap)
                .count(),
            1
        );
    }

    #[test]
    fn rtx_50_hotspot_uses_a_localized_beta_label_without_changing_sensor_id() {
        let mut snapshot = info(&[("GPU Hot Spot", 72.0, "°C")]);
        snapshot.gpu_name = Some("NVIDIA GeForce RTX 5050".to_owned());
        let hotspot = ordered_sensors(&snapshot)
            .into_iter()
            .next()
            .expect("visible HotSpot");

        assert_eq!(
            sensor_label(&hotspot, &snapshot, Language::English),
            "GPU Hot Spot (BETA)"
        );
        assert_eq!(
            sensor_label(&hotspot, &snapshot, Language::Russian),
            "GPU Hot Spot (БЕТА)"
        );
        assert_eq!(
            sensor_id(&hotspot),
            sensor_id(&SensorReading {
                name: "GPU Hot Spot".to_owned(),
                value: hotspot.value,
                unit: hotspot.unit.clone(),
            })
        );
    }

    #[test]
    fn beta_label_is_not_applied_to_non_rtx_50_hotspots() {
        let snapshot = info(&[("GPU Hot Spot", 72.0, "°C")]);
        let hotspot = ordered_sensors(&snapshot)
            .into_iter()
            .next()
            .expect("visible HotSpot");

        assert_eq!(
            sensor_label(&hotspot, &snapshot, Language::English),
            "GPU Hot Spot"
        );
    }

    #[test]
    fn list_hit_testing_cannot_select_the_graph_panel() {
        let snapshot = info(&[("GPU Core", 60.0, "°C"), ("Hot Spot", 74.0, "°C")]);

        assert_eq!(
            sensor_at_point(&snapshot, 100, 190)
                .expect("first visible sensor")
                .name,
            "GPU Core"
        );
        assert!(sensor_at_point(&snapshot, 700, 190).is_none());
    }

    #[test]
    fn feedback_controls_leave_labels_and_each_other_visible() {
        assert!(FEEDBACK_CONTACT_EDIT.top >= LIST_TOP + 116);
        assert!(FEEDBACK_MESSAGE_EDIT.top >= LIST_TOP + 180);
        assert!(FEEDBACK_MESSAGE_EDIT.top > FEEDBACK_CONTACT_EDIT.bottom);
        assert!(FEEDBACK_CONSENT_HIT.top > FEEDBACK_MESSAGE_EDIT.bottom);
        assert!(FEEDBACK_SUBMIT_HIT.top > FEEDBACK_CONSENT_HIT.top);
    }

    #[test]
    fn localized_header_navigation_keeps_fixed_gaps() {
        let layout = header_nav_layout(964, 119, 91, 139);
        assert_eq!(layout.settings.0 - layout.about.1, HEADER_NAV_GAP);
        assert_eq!(layout.feedback.0 - layout.settings.1, HEADER_NAV_GAP);
        assert!(layout.about.0 > 120);
        assert!(header_link_contains(layout.about, layout.about.0));
        assert!(!header_link_contains(layout.about, layout.settings.0));
    }
}
