use crate::feedback;
use crate::gui_i18n::{Key, Language};
use crate::gui_state::{SensorHistory, SensorStats};
use crate::sensor_model::{SensorGroup, SensorKind, metadata, sensor_id};
use crate::settings::{self, AccentTheme, AppSettings, UiLanguage};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, RichText, Sense, Stroke, TextStyle, Vec2,
};
use gpu_shark::{
    SensorReading, SysInfo, dll_library_path, fetch_data_from_dll, load_driver_library,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::{thread, time::Duration};
use windows_sys::Win32::Graphics::Dwm::DwmGetColorizationColor;

const BACKGROUND: Color32 = Color32::from_rgb(25, 27, 30);
const SURFACE: Color32 = Color32::from_rgb(34, 37, 41);
const RAISED: Color32 = Color32::from_rgb(45, 49, 54);
const DIVIDER: Color32 = Color32::from_rgb(66, 71, 78);
const LABEL: Color32 = Color32::from_rgb(194, 198, 204);
const MUTED: Color32 = Color32::from_rgb(139, 145, 154);
const VALUE: Color32 = Color32::from_rgb(246, 248, 250);
const AMBER: Color32 = Color32::from_rgb(246, 197, 68);
const RED: Color32 = Color32::from_rgb(240, 82, 91);

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
    fn localized(&self, language: Language) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Required => language.feedback_required().into(),
            Self::Sending => language.feedback_sending_status().into(),
            Self::Accepted(id) => language.feedback_accepted(id),
            Self::Rejected(detail) => language.feedback_rejected(detail),
            Self::PayloadTooLarge => language.feedback_payload_too_large().into(),
            Self::RateLimited => language.feedback_rate_limited().into(),
            Self::Server => language.feedback_server_error().into(),
            Self::Network(error) => language.feedback_network_error(error),
        }
    }
}

struct TelemetryWorker {
    stop: mpsc::Sender<()>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for TelemetryWorker {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub struct GpuSharkApp {
    settings: AppSettings,
    settings_draft: AppSettings,
    settings_open: bool,
    settings_notice: Option<String>,
    snapshot: Option<Snapshot>,
    telemetry_rx: mpsc::Receiver<Snapshot>,
    refresh_interval: Arc<AtomicU64>,
    _worker: TelemetryWorker,
    history: SensorHistory,
    feedback_open: bool,
    feedback_note: String,
    feedback_contact: String,
    feedback_consent: bool,
    feedback_sending: bool,
    feedback_status: FeedbackStatus,
    feedback_rx: Option<mpsc::Receiver<FeedbackStatus>>,
}

impl GpuSharkApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let outcome = settings::load();
        let settings = outcome.settings;
        configure_style(&cc.egui_ctx, accent_color(settings.accent));
        let refresh_interval = Arc::new(AtomicU64::new(settings.refresh_interval_ms));
        let (telemetry_rx, worker) =
            spawn_worker(cc.egui_ctx.clone(), Arc::clone(&refresh_interval));
        Self {
            settings_draft: settings.clone(),
            settings,
            settings_open: false,
            settings_notice: outcome.warning,
            snapshot: None,
            telemetry_rx,
            refresh_interval,
            _worker: worker,
            history: SensorHistory::default(),
            feedback_open: false,
            feedback_note: String::new(),
            feedback_contact: String::new(),
            feedback_consent: false,
            feedback_sending: false,
            feedback_status: FeedbackStatus::Empty,
            feedback_rx: None,
        }
    }

    fn language(&self) -> Language {
        Language::from_russian(self.settings.language == UiLanguage::Russian)
    }

    fn accent(&self) -> Color32 {
        accent_color(self.settings.accent)
    }

    fn drain_updates(&mut self) {
        while let Ok(snapshot) = self.telemetry_rx.try_recv() {
            if let Snapshot::Data(info) = &snapshot {
                self.history.record(&ordered_sensors(info));
            }
            self.snapshot = Some(snapshot);
        }
        let feedback = self.feedback_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(status) = feedback {
            self.feedback_status = status;
            self.feedback_sending = false;
            self.feedback_rx = None;
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        let language = self.language();
        let accent = self.accent();
        let name = match &self.snapshot {
            Some(Snapshot::Data(info)) => info.gpu_name.as_deref().unwrap_or("Unknown GPU"),
            _ => "GPU Shark",
        };
        let available = ui.available_width();
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new((available - 270.0).max(300.0), 76.0),
                Layout::top_down(Align::LEFT),
                |ui| {
                    ui.label(RichText::new("GPU SHARK").size(18.0).strong().color(accent));
                    ui.add(
                        egui::Label::new(RichText::new(name).size(17.0).strong().color(VALUE))
                            .truncate(),
                    )
                    .on_hover_text(name);
                    ui.label(
                        RichText::new(language.refresh_hint(self.settings.refresh_interval_ms))
                            .size(12.5)
                            .color(MUTED),
                    );
                },
            );
            ui.allocate_ui_with_layout(
                Vec2::new(260.0, 76.0),
                Layout::right_to_left(Align::TOP),
                |ui| {
                    if ui
                        .add(header_button(language.text(Key::Feedback), accent))
                        .clicked()
                    {
                        self.feedback_open = true;
                    }
                    if ui
                        .add(header_button(language.text(Key::Settings), accent))
                        .clicked()
                    {
                        self.settings_draft = self.settings.clone();
                        self.settings_open = true;
                    }
                },
            );
        });
        if matches!(&self.snapshot, Some(Snapshot::Data(info)) if info.gpu_name.is_none()) {
            ui.label(RichText::new(language.text(Key::UnknownGpu)).color(AMBER));
        }
        if let Some(notice) = self.settings_notice.clone() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(notice).color(AMBER));
                if ui.small_button("?").clicked() {
                    self.settings_notice = None;
                }
            });
        }
    }

    fn dashboard(&mut self, ui: &mut egui::Ui, info: &SysInfo) {
        let sensors = ordered_sensors(info);
        let height = ui.available_height().max(420.0);
        ui.columns(2, |columns| {
            card(&mut columns[0], height, |ui| {
                heading(ui, self.language().text(Key::Sensors), self.accent());
                ui.separator();
                self.sensor_list(ui, &sensors);
            });
            card(&mut columns[1], height, |ui| {
                heading(ui, self.language().text(Key::Selected), self.accent());
                ui.separator();
                self.selected_panel(ui, &sensors);
            });
        });
    }

    fn sensor_list(&mut self, ui: &mut egui::Ui, sensors: &[SensorReading]) {
        let language = self.language();
        let accent = self.accent();
        egui::ScrollArea::vertical()
            .id_salt("sensor-list")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut previous_group = None;
                for sensor in sensors {
                    let item = metadata(sensor);
                    if previous_group != Some(item.group) {
                        if previous_group.is_some() {
                            ui.add_space(8.0);
                        }
                        ui.label(
                            RichText::new(group_title(item.group, language))
                                .size(11.5)
                                .strong()
                                .color(MUTED),
                        );
                        previous_group = Some(item.group);
                    }
                    let selected = self.history.selected_id().is_some_and(|id| *id == item.id);
                    let response = sensor_row(ui, sensor, selected, accent)
                        .on_hover_text(sensor_tooltip(item.kind, language));
                    if response.double_clicked() {
                        self.history.select_maximum(sensor);
                    } else if response.clicked() {
                        self.history.select(sensor);
                    }
                }
                if sensors.is_empty() {
                    ui.label(RichText::new(language.text(Key::Unavailable)).color(MUTED));
                }
            });
    }

    fn selected_panel(&mut self, ui: &mut egui::Ui, sensors: &[SensorReading]) {
        let language = self.language();
        let accent = self.accent();
        let selected = self
            .history
            .selected_id()
            .and_then(|id| sensors.iter().find(|sensor| sensor_id(sensor) == *id));
        let Some(sensor) = selected else {
            ui.add_space(38.0);
            ui.heading(language.text(Key::ChooseSensor));
            ui.label(RichText::new(language.text(Key::ChooseSensorHint)).color(MUTED));
            return;
        };
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(&sensor.name).size(18.0).strong().color(VALUE));
                ui.label(
                    RichText::new(sensor_value(sensor))
                        .size(28.0)
                        .strong()
                        .color(sensor_color(sensor)),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if metadata(sensor).graphable && ui.button(language.text(Key::Reset)).clicked() {
                    self.history.reset();
                }
            });
        });
        ui.add_space(8.0);
        if metadata(sensor).kind == SensorKind::PerfCap {
            egui::Frame::new()
                .fill(RAISED)
                .corner_radius(8.0)
                .inner_margin(14.0)
                .show(ui, |ui| {
                    ui.label(RichText::new(language.text(Key::PerfCapDetail)).color(LABEL));
                    ui.label(RichText::new(language.text(Key::PerfCapNoGraph)).color(MUTED));
                });
            return;
        }
        let stats = self.history.stats().unwrap_or(SensorStats {
            current: sensor.value,
            min: sensor.value,
            max: sensor.value,
        });
        ui.horizontal(|ui| {
            metric_chip(ui, language.text(Key::Min), stats.min, &sensor.unit, MUTED);
            metric_chip(ui, language.text(Key::Max), stats.max, &sensor.unit, accent);
            if self.history.shows_maximum() {
                let text = if matches!(language, Language::Russian) {
                    "ПОКАЗАН МАКСИМУМ"
                } else {
                    "SHOWING MAXIMUM"
                };
                ui.label(RichText::new(text).size(10.5).strong().color(accent));
            }
        });
        ui.label(
            RichText::new(language.text(Key::History))
                .size(11.5)
                .strong()
                .color(MUTED),
        );
        graph(ui, &self.history.samples(), stats, accent);
    }

    fn unavailable(&self, ui: &mut egui::Ui, detail: &str) {
        card(ui, ui.available_height().max(320.0), |ui| {
            ui.add_space(32.0);
            ui.label(
                RichText::new(self.language().text(Key::Unavailable))
                    .size(18.0)
                    .strong()
                    .color(RED),
            );
            ui.add_space(12.0);
            ui.label(RichText::new(detail).color(LABEL));
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let language = self.language();
        let mut open = self.settings_open;
        let mut apply = false;
        let mut restore = false;
        let title = if matches!(language, Language::Russian) {
            "Настройки GPU Shark"
        } else {
            "GPU Shark Settings"
        };
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                egui::Grid::new("settings-grid")
                    .num_columns(2)
                    .spacing([24.0, 14.0])
                    .show(ui, |ui| {
                        ui.label(if matches!(language, Language::Russian) {
                            "Язык интерфейса"
                        } else {
                            "Interface language"
                        });
                        egui::ComboBox::from_id_salt("language")
                            .selected_text(match self.settings_draft.language {
                                UiLanguage::English => "English",
                                UiLanguage::Russian => "Русский",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.settings_draft.language,
                                    UiLanguage::English,
                                    "English",
                                );
                                ui.selectable_value(
                                    &mut self.settings_draft.language,
                                    UiLanguage::Russian,
                                    "Русский",
                                );
                            });
                        ui.end_row();
                        ui.label(if matches!(language, Language::Russian) {
                            "Частота обновления"
                        } else {
                            "Refresh interval"
                        });
                        egui::ComboBox::from_id_salt("refresh")
                            .selected_text(refresh_label(self.settings_draft.refresh_interval_ms))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.settings_draft.refresh_interval_ms,
                                    500,
                                    "500 ms",
                                );
                                ui.selectable_value(
                                    &mut self.settings_draft.refresh_interval_ms,
                                    1_000,
                                    "1 s",
                                );
                                ui.selectable_value(
                                    &mut self.settings_draft.refresh_interval_ms,
                                    2_000,
                                    "2 s",
                                );
                            });
                        ui.end_row();
                        ui.label(if matches!(language, Language::Russian) {
                            "Акцентный цвет"
                        } else {
                            "Accent color"
                        });
                        egui::ComboBox::from_id_salt("accent")
                            .selected_text(accent_label(self.settings_draft.accent))
                            .show_ui(ui, |ui| {
                                for theme in [
                                    AccentTheme::Green,
                                    AccentTheme::Blue,
                                    AccentTheme::Purple,
                                    AccentTheme::Orange,
                                    AccentTheme::Windows,
                                ] {
                                    ui.selectable_value(
                                        &mut self.settings_draft.accent,
                                        theme,
                                        accent_label(theme),
                                    );
                                }
                            });
                        ui.end_row();
                        ui.label(if matches!(language, Language::Russian) {
                            "Температура"
                        } else {
                            "Temperature"
                        });
                        ui.add_enabled(
                            false,
                            egui::Button::new(if matches!(language, Language::Russian) {
                                "Цельсий"
                            } else {
                                "Celsius"
                            }),
                        );
                        ui.end_row();
                    });
                ui.add_space(16.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .button(if matches!(language, Language::Russian) {
                            "Восстановить значения"
                        } else {
                            "Restore defaults"
                        })
                        .clicked()
                    {
                        restore = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(if matches!(language, Language::Russian) {
                                    "Применить"
                                } else {
                                    "Apply"
                                })
                                .fill(accent_color(self.settings_draft.accent)),
                            )
                            .clicked()
                        {
                            apply = true;
                        }
                        if ui
                            .button(if matches!(language, Language::Russian) {
                                "Отмена"
                            } else {
                                "Cancel"
                            })
                            .clicked()
                        {
                            self.settings_draft = self.settings.clone();
                            open = false;
                        }
                    });
                });
            });
        if restore {
            self.settings_draft = AppSettings::default();
        }
        if apply {
            match settings::save(&self.settings_draft) {
                Ok(()) => {
                    self.settings = self.settings_draft.clone();
                    self.refresh_interval
                        .store(self.settings.refresh_interval_ms, Ordering::Release);
                    configure_style(ctx, self.accent());
                    self.settings_notice = None;
                    open = false;
                }
                Err(error) => self.settings_notice = Some(error),
            }
        }
        self.settings_open = open;
    }

    fn feedback_window(&mut self, ctx: &egui::Context) {
        if !self.feedback_open {
            return;
        }
        let language = self.language();
        let accent = self.accent();
        let mut open = self.feedback_open;
        let mut submit = false;
        egui::Window::new(language.text(Key::Feedback))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(620.0)
            .min_width(480.0)
            .show(ctx, |ui| {
                ui.label(RichText::new(language.text(Key::FeedbackPrivacy)).color(LABEL));
                ui.add_space(10.0);
                ui.label(language.text(Key::FeedbackContact));
                ui.add(
                    egui::TextEdit::singleline(&mut self.feedback_contact)
                        .hint_text(if matches!(language, Language::Russian) {
                            "Email или другой контакт"
                        } else {
                            "Email or another contact"
                        })
                        .char_limit(500),
                );
                ui.label(language.text(Key::FeedbackDescription));
                ui.add(
                    egui::TextEdit::multiline(&mut self.feedback_note)
                        .desired_rows(7)
                        .lock_focus(true)
                        .char_limit(8_000),
                );
                ui.checkbox(
                    &mut self.feedback_consent,
                    language.text(Key::FeedbackConsent),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let caption = if self.feedback_sending {
                        language.text(Key::FeedbackSending)
                    } else {
                        language.text(Key::FeedbackSubmit)
                    };
                    if ui
                        .add_enabled(
                            !self.feedback_sending,
                            egui::Button::new(caption).fill(if self.feedback_consent {
                                accent
                            } else {
                                RAISED
                            }),
                        )
                        .clicked()
                    {
                        submit = true;
                    }
                    let status = self.feedback_status.localized(language);
                    if !status.is_empty() {
                        let color = if matches!(self.feedback_status, FeedbackStatus::Accepted(_)) {
                            accent
                        } else {
                            LABEL
                        };
                        ui.label(RichText::new(status).color(color));
                    }
                });
            });
        self.feedback_open = open;
        if submit {
            self.start_feedback(ctx);
        }
    }

    fn start_feedback(&mut self, ctx: &egui::Context) {
        if self.feedback_sending {
            return;
        }
        if self.feedback_note.trim().is_empty() || !self.feedback_consent {
            self.feedback_status = FeedbackStatus::Required;
            return;
        }
        let note = self.feedback_note.clone();
        let contact = self.feedback_contact.clone();
        let consent = self.feedback_consent;
        let locale = if self.settings.language == UiLanguage::Russian {
            "ru"
        } else {
            "en"
        };
        let info = match &self.snapshot {
            Some(Snapshot::Data(info)) => Some(info.clone()),
            _ => None,
        };
        let (tx, rx) = mpsc::channel();
        let repaint = ctx.clone();
        self.feedback_sending = true;
        self.feedback_status = FeedbackStatus::Sending;
        self.feedback_rx = Some(rx);
        thread::spawn(move || {
            let status =
                match feedback::submit(info.as_ref(), &note, Some(&contact), locale, consent) {
                    Ok(id) => FeedbackStatus::Accepted(id),
                    Err(feedback::SubmitError::InvalidPayload(detail)) => {
                        FeedbackStatus::Rejected(detail)
                    }
                    Err(feedback::SubmitError::PayloadTooLarge) => FeedbackStatus::PayloadTooLarge,
                    Err(feedback::SubmitError::RateLimited) => FeedbackStatus::RateLimited,
                    Err(feedback::SubmitError::Server) => FeedbackStatus::Server,
                    Err(feedback::SubmitError::Network(error)) => FeedbackStatus::Network(error),
                };
            let _ = tx.send(status);
            repaint.request_repaint();
        });
    }
}

impl eframe::App for GpuSharkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_updates();
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| self.header(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(16.0))
            .show(ctx, |ui| match self.snapshot.clone() {
                Some(Snapshot::Data(info)) => self.dashboard(ui, &info),
                Some(Snapshot::LoadFailed(error)) => self.unavailable(
                    ui,
                    &format!("Could not load {}: {error}", dll_library_path()),
                ),
                Some(Snapshot::FetchError(error)) => self.unavailable(ui, &error),
                None => self.unavailable(
                    ui,
                    if self.settings.language == UiLanguage::Russian {
                        "Подключение к локальному источнику телеметрии:"
                    } else {
                        "Connecting to the local telemetry provider:"
                    },
                ),
            });
        self.settings_window(ctx);
        self.feedback_window(ctx);
    }
}

fn spawn_worker(
    repaint: egui::Context,
    interval: Arc<AtomicU64>,
) -> (mpsc::Receiver<Snapshot>, TelemetryWorker) {
    let (snapshot_tx, snapshot_rx) = mpsc::channel();
    let (stop_tx, stop_rx) = mpsc::channel();
    let handle = thread::Builder::new()
        .name("gpu-shark-telemetry".into())
        .spawn(move || {
            let library = match load_driver_library() {
                Ok(library) => library,
                Err(error) => {
                    let _ = snapshot_tx.send(Snapshot::LoadFailed(error));
                    repaint.request_repaint();
                    return;
                }
            };
            loop {
                let snapshot = match fetch_data_from_dll(&library) {
                    Ok(info) => Snapshot::Data(info),
                    Err(error) => Snapshot::FetchError(error.to_string()),
                };
                if snapshot_tx.send(snapshot).is_err() {
                    break;
                }
                repaint.request_repaint();
                if stop_rx
                    .recv_timeout(Duration::from_millis(interval.load(Ordering::Acquire)))
                    .is_ok()
                {
                    break;
                }
            }
        })
        .expect("telemetry worker");
    (
        snapshot_rx,
        TelemetryWorker {
            stop: stop_tx,
            handle: Some(handle),
        },
    )
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
            name: "PerfCap Reason".into(),
            value: 0.0,
            unit: reason.into(),
        });
    }
    sensors.retain(|sensor| metadata(sensor).visible);
    sensors.sort_by_key(|sensor| {
        let item = metadata(sensor);
        (item.group, item.priority, sensor.name.clone())
    });
    sensors
}

fn configure_style(ctx: &egui::Context, accent: Color32) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BACKGROUND;
    style.visuals.extreme_bg_color = Color32::from_rgb(20, 22, 24);
    style.visuals.faint_bg_color = SURFACE;
    style.visuals.text_edit_bg_color = Some(Color32::from_rgb(27, 30, 33));
    style.visuals.selection.bg_fill = accent.gamma_multiply(0.45);
    style.visuals.selection.stroke = Stroke::new(1.0, accent);
    style.visuals.widgets.inactive.bg_fill = RAISED;
    style.visuals.widgets.inactive.fg_stroke.color = LABEL;
    style.visuals.widgets.hovered.bg_fill = accent.gamma_multiply(0.22);
    style.visuals.widgets.active.bg_fill = accent.gamma_multiply(0.38);
    ctx.set_style(style);
}

fn card<R>(ui: &mut egui::Ui, height: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, DIVIDER))
        .corner_radius(10.0)
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.set_min_height((height - 34.0).max(360.0));
            add(ui)
        })
        .inner
}

fn heading(ui: &mut egui::Ui, text: &str, accent: Color32) {
    ui.label(RichText::new(text).size(12.0).strong().color(accent));
}

fn header_button<'a>(text: &'a str, accent: Color32) -> egui::Button<'a> {
    egui::Button::new(RichText::new(text).strong().color(accent))
        .frame(true)
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, DIVIDER))
        .corner_radius(7.0)
}

fn sensor_row(
    ui: &mut egui::Ui,
    sensor: &SensorReading,
    selected: bool,
    accent: Color32,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 31.0), Sense::click());
    let fill = if selected {
        accent.gamma_multiply(0.16)
    } else if response.hovered() {
        RAISED
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 5.0, fill);
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 3.0, rect.bottom())),
            2.0,
            accent,
        );
    }
    let font = TextStyle::Body.resolve(ui.style());
    ui.painter().text(
        rect.left_center() + egui::vec2(10.0, 0.0),
        Align2::LEFT_CENTER,
        &sensor.name,
        font.clone(),
        LABEL,
    );
    ui.painter().text(
        rect.right_center() - egui::vec2(10.0, 0.0),
        Align2::RIGHT_CENTER,
        sensor_value(sensor),
        font,
        sensor_color(sensor),
    );
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, DIVIDER.gamma_multiply(0.65)),
    );
    response
}

fn graph(ui: &mut egui::Ui, samples: &[f32], stats: SensorStats, accent: Color32) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ui.available_height().max(230.0)),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, 8.0, Color32::from_rgb(22, 24, 27));
    let graph = rect.shrink2(Vec2::new(12.0, 20.0));
    for step in 0..=4 {
        let y = egui::lerp(graph.top()..=graph.bottom(), step as f32 / 4.0);
        ui.painter().line_segment(
            [egui::pos2(graph.left(), y), egui::pos2(graph.right(), y)],
            Stroke::new(1.0, DIVIDER.gamma_multiply(0.55)),
        );
    }
    let padding = ((stats.max - stats.min).abs() * 0.20)
        .max(stats.max.abs() * 0.03)
        .max(0.5);
    let lower = stats.min - padding;
    let upper = stats.max + padding;
    let span = (upper - lower).max(0.01);
    ui.painter().text(
        graph.left_top(),
        Align2::LEFT_TOP,
        format!("{upper:.1}"),
        FontId::monospace(10.0),
        MUTED,
    );
    ui.painter().text(
        graph.left_bottom(),
        Align2::LEFT_BOTTOM,
        format!("{lower:.1}"),
        FontId::monospace(10.0),
        MUTED,
    );
    if samples.len() < 2 {
        return;
    }
    let points = samples
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = egui::lerp(
                graph.left()..=graph.right(),
                index as f32 / (samples.len() - 1) as f32,
            );
            let y = egui::lerp(
                graph.bottom()..=graph.top(),
                ((*value - lower) / span).clamp(0.0, 1.0),
            );
            egui::pos2(x, y)
        })
        .collect();
    ui.painter()
        .add(egui::Shape::line(points, Stroke::new(2.2, accent)));
}

fn metric_chip(ui: &mut egui::Ui, label: &str, value: f32, unit: &str, color: Color32) {
    egui::Frame::new()
        .fill(RAISED)
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("{label}  {value:.1} {unit}"))
                    .size(11.5)
                    .strong()
                    .color(color),
            );
        });
}

fn sensor_value(sensor: &SensorReading) -> String {
    if metadata(sensor).kind == SensorKind::PerfCap
        && sensor.name.trim().eq_ignore_ascii_case("PerfCap Reason")
    {
        return sensor.unit.clone();
    }
    match sensor.unit.trim().to_ascii_lowercase().as_str() {
        "rpm" | "mhz" | "mb" => format!("{:.0} {}", sensor.value, sensor.unit),
        "v" => format!("{:.3} {}", sensor.value, sensor.unit),
        _ => format!("{:.1} {}", sensor.value, sensor.unit),
    }
}

fn sensor_color(sensor: &SensorReading) -> Color32 {
    match metadata(sensor).kind {
        SensorKind::HotspotTemperature if sensor.value >= 90.0 => RED,
        SensorKind::GpuCoreTemperature if sensor.value >= 83.0 => RED,
        SensorKind::MemoryTemperature if sensor.value >= 80.0 => AMBER,
        _ => VALUE,
    }
}

fn sensor_tooltip(kind: SensorKind, language: Language) -> &'static str {
    match (kind, language) {
        (SensorKind::PerfCap, Language::Russian) => {
            "Причина текущего ограничения производительности NVIDIA. Это категориальное состояние."
        }
        (SensorKind::PerfCap, Language::English) => {
            "The current NVIDIA performance-limit reason. This is a categorical state."
        }
        (SensorKind::HotspotTemperature, Language::Russian) => {
            "HotSpot отображается только когда источник действительно предоставляет подтверждённое значение."
        }
        (SensorKind::HotspotTemperature, Language::English) => {
            "HotSpot is shown only when the provider exposes a validated value."
        }
        (_, Language::Russian) => "Нажмите для графика; двойной клик показывает максимум сессии.",
        (_, Language::English) => "Click for a graph; double-click shows the session maximum.",
    }
}

fn group_title(group: SensorGroup, language: Language) -> &'static str {
    match group {
        SensorGroup::Gpu => "GPU",
        SensorGroup::Activity => language.text(Key::GpuActivity),
        SensorGroup::System => language.text(Key::System),
    }
}

fn refresh_label(value: u64) -> &'static str {
    match value {
        500 => "500 ms",
        2_000 => "2 s",
        _ => "1 s",
    }
}

fn accent_label(theme: AccentTheme) -> &'static str {
    match theme {
        AccentTheme::Green => "Green",
        AccentTheme::Blue => "Blue",
        AccentTheme::Purple => "Purple",
        AccentTheme::Orange => "Orange",
        AccentTheme::Windows => "Windows",
    }
}

fn accent_color(theme: AccentTheme) -> Color32 {
    match theme {
        AccentTheme::Green => Color32::from_rgb(87, 227, 137),
        AccentTheme::Blue => Color32::from_rgb(96, 172, 255),
        AccentTheme::Purple => Color32::from_rgb(199, 145, 255),
        AccentTheme::Orange => Color32::from_rgb(255, 178, 94),
        AccentTheme::Windows => windows_accent(),
    }
}

fn windows_accent() -> Color32 {
    unsafe {
        let mut raw = 0u32;
        let mut opaque = 0;
        if DwmGetColorizationColor(&mut raw, &mut opaque) < 0 {
            return Color32::from_rgb(87, 227, 137);
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
        Color32::from_rgb(red, green, blue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(sensors: &[(&str, f32, &str)]) -> SysInfo {
        let sensors = sensors.iter().map(|(name, value, unit)| serde_json::json!({"name": name, "value": value, "unit": unit})).collect::<Vec<_>>();
        serde_json::from_value(serde_json::json!({"gpu_name": "egui fixture", "sensors": sensors}))
            .expect("safe fixture")
    }

    #[test]
    fn ordered_rows_preserve_visibility_and_perfcap_contract() {
        let mut snapshot = info(&[
            ("Memory Clock", 10_501.0, "MHz"),
            ("GPU Core", 60.0, "°C"),
            ("D3D 3D", 90.0, "%"),
        ]);
        snapshot.perfcap_reason = Some("Pwr, VRel".into());
        let rows = ordered_sensors(&snapshot);
        assert_eq!(
            rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
            ["GPU Core", "PerfCap Reason", "D3D 3D"]
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.name == "PerfCap Reason")
                .unwrap()
                .unit,
            "Pwr, VRel"
        );
    }

    #[test]
    fn value_formatting_is_unit_aware() {
        assert_eq!(
            sensor_value(&SensorReading {
                name: "GPU Fan 1".into(),
                value: 1380.0,
                unit: "RPM".into()
            }),
            "1380 RPM"
        );
        assert_eq!(
            sensor_value(&SensorReading {
                name: "GPU Voltage".into(),
                value: 1.05,
                unit: "V".into()
            }),
            "1.050 V"
        );
    }
}
