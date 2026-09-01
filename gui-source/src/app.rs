use crate::feedback;
use crate::gui_i18n::{Key, Language};
use crate::gui_state::{SensorHistory, SensorStats};
use crate::sensor_model::{SensorKind, metadata, sensor_id};
use crate::settings::{self, AccentTheme, AppSettings, UiLanguage, UiTheme};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, TextStyle, Vec2,
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

const ROW_HEIGHT: f32 = 26.0;
const SPARK_WIDTH: f32 = 64.0;
const TABLE_MARGIN: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Sensors,
    Settings,
    Feedback,
    About,
}

#[derive(Clone, Copy)]
struct Palette {
    chrome: Color32,
    background: Color32,
    hover: Color32,
    divider: Color32,
    text: Color32,
    muted: Color32,
    value: Color32,
    graph: Color32,
    selection: Color32,
    danger: Color32,
    warning: Color32,
    edit_bg: Color32,
    plot_bg: Color32,
}

fn palette(theme: UiTheme, accent: Color32) -> Palette {
    match theme {
        UiTheme::Light => Palette {
            chrome: Color32::from_rgb(240, 240, 240),
            background: Color32::from_rgb(255, 255, 255),
            hover: Color32::from_rgb(238, 244, 250),
            divider: Color32::from_rgb(219, 219, 219),
            text: Color32::from_rgb(26, 26, 26),
            muted: Color32::from_rgb(128, 128, 128),
            value: Color32::from_rgb(0, 0, 0),
            graph: Color32::from_rgb(224, 0, 0),
            selection: Color32::from_rgb(204, 232, 255),
            danger: Color32::from_rgb(192, 28, 28),
            warning: Color32::from_rgb(176, 128, 0),
            edit_bg: Color32::from_rgb(255, 255, 255),
            plot_bg: Color32::from_rgb(250, 250, 250),
        },
        UiTheme::Dark => Palette {
            chrome: Color32::from_rgb(25, 27, 30),
            background: Color32::from_rgb(25, 27, 30),
            hover: Color32::from_rgb(45, 49, 54),
            divider: Color32::from_rgb(66, 71, 78),
            text: Color32::from_rgb(194, 198, 204),
            muted: Color32::from_rgb(139, 145, 154),
            value: Color32::from_rgb(246, 248, 250),
            graph: accent,
            selection: accent.gamma_multiply(0.20),
            danger: Color32::from_rgb(240, 82, 91),
            warning: Color32::from_rgb(246, 197, 68),
            edit_bg: Color32::from_rgb(27, 30, 33),
            plot_bg: Color32::from_rgb(22, 24, 27),
        },
    }
}

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
    tab: Tab,
    settings: AppSettings,
    settings_draft: AppSettings,
    settings_notice: Option<String>,
    snapshot: Option<Snapshot>,
    telemetry_rx: mpsc::Receiver<Snapshot>,
    refresh_interval: Arc<AtomicU64>,
    _worker: TelemetryWorker,
    history: SensorHistory,
    feedback_note: String,
    feedback_contact: String,
    feedback_consent: bool,
    feedback_sending: bool,
    feedback_status: FeedbackStatus,
    feedback_rx: Option<mpsc::Receiver<FeedbackStatus>>,
    about_icon: Option<egui::TextureHandle>,
}

impl GpuSharkApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        let outcome = settings::load();
        let mut settings = outcome.settings;
        settings.autostart = crate::autostart::is_enabled();
        configure_style(&cc.egui_ctx, settings.theme, accent_color(settings.accent));
        let refresh_interval = Arc::new(AtomicU64::new(settings.refresh_interval_ms));
        let (telemetry_rx, worker) =
            spawn_worker(cc.egui_ctx.clone(), Arc::clone(&refresh_interval));
        Self {
            tab: Tab::Sensors,
            settings_draft: settings.clone(),
            settings,
            settings_notice: outcome.warning,
            snapshot: None,
            telemetry_rx,
            refresh_interval,
            _worker: worker,
            history: SensorHistory::default(),
            feedback_note: String::new(),
            feedback_contact: String::new(),
            feedback_consent: false,
            feedback_sending: false,
            feedback_status: FeedbackStatus::Empty,
            feedback_rx: None,
            about_icon: None,
        }
    }

    fn language(&self) -> Language {
        Language::from_russian(self.settings.language == UiLanguage::Russian)
    }

    fn accent(&self) -> Color32 {
        accent_color(self.settings.accent)
    }

    fn palette(&self) -> Palette {
        palette(self.settings.theme, self.accent())
    }

    fn drain_updates(&mut self) {
        while let Ok(snapshot) = self.telemetry_rx.try_recv() {
            if let Snapshot::Data(info) = &snapshot {
                let sensors = ordered_sensors(info);
                if self.settings.track_all_maxima {
                    self.history.track_all(&sensors);
                }
                self.history.record(&sensors);
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

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        ui.set_min_height(34.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.label(RichText::new("GPU SHARK").size(12.0).strong().color(p.text));
            ui.add_space(14.0);
            for (tab, key) in [
                (Tab::Sensors, Key::Sensors),
                (Tab::Settings, Key::Settings),
                (Tab::Feedback, Key::Feedback),
                (Tab::About, Key::About),
            ] {
                let active = self.tab == tab;
                let text = RichText::new(self.language().text(key))
                    .size(12.5)
                    .strong()
                    .color(if active { p.text } else { p.muted });
                let button = egui::Button::new(text)
                    .min_size(Vec2::new(0.0, 26.0))
                    .fill(if active {
                        p.background
                    } else {
                        Color32::TRANSPARENT
                    })
                    .stroke(if active {
                        Stroke::new(1.0, p.divider)
                    } else {
                        Stroke::NONE
                    })
                    .corner_radius(0.0);
                if ui.add(button).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.add_space(2.0);
    }

    fn sensors_tab(&mut self, ui: &mut egui::Ui) {
        let Some(Snapshot::Data(info)) = self.snapshot.clone() else {
            let message = match &self.snapshot {
                Some(Snapshot::LoadFailed(error)) => {
                    format!("Could not load {}: {error}", dll_library_path())
                }
                Some(Snapshot::FetchError(error)) => error.clone(),
                _ => connect_message(self.settings.language),
            };
            self.unavailable(ui, &message);
            return;
        };
        let sensors = ordered_sensors(&info);
        let detail_open = self
            .history
            .selected_id()
            .is_some_and(|selected| sensors.iter().any(|sensor| sensor_id(sensor) == *selected));
        let detail_height = if detail_open { 198.0 } else { 0.0 };
        let width = ui.available_width();
        let table_height = (ui.available_height() - 48.0 - detail_height).max(160.0);
        ui.allocate_ui(Vec2::new(width, table_height), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("sensor-table")
                .auto_shrink([false, false])
                .show(ui, |ui| self.sensor_table(ui, &sensors));
        });
        if detail_open {
            self.detail_panel(ui, &sensors);
        }
        ui.add_space(6.0);
        ui.separator();
        self.sensors_bottom_bar(ui, &info);
    }

    fn sensor_table(&mut self, ui: &mut egui::Ui, sensors: &[SensorReading]) {
        let p = self.palette();
        let width = ui.available_width();
        for sensor in sensors {
            let item = metadata(sensor);
            let id = item.id.clone();
            let selected = self.history.selected_id() == Some(&id);
            let tracked = self.history.is_tracked(&id);
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::click());
            let fill = if selected {
                p.selection
            } else if response.hovered() {
                p.hover
            } else {
                Color32::TRANSPARENT
            };
            let font = TextStyle::Body.resolve(ui.style());
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, fill);
            painter.line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                Stroke::new(1.0, p.divider.gamma_multiply(0.7)),
            );
            painter.text(
                egui::pos2(rect.left() + TABLE_MARGIN, rect.center().y),
                Align2::LEFT_CENTER,
                &sensor.name,
                font.clone(),
                p.text,
            );
            let value_x = rect.left() + ((width - 200.0).max(240.0) * 0.55) + TABLE_MARGIN;
            painter.text(
                egui::pos2(value_x, rect.center().y),
                Align2::LEFT_CENTER,
                sensor_value(sensor),
                font,
                sensor_color(sensor, p),
            );
            let spark = Rect::from_min_size(
                egui::pos2(rect.right() - SPARK_WIDTH - 8.0, rect.center().y - 9.0),
                Vec2::new(SPARK_WIDTH, 18.0),
            );
            let samples = self.history.row_samples(&id);
            let unit = sensor.unit.trim();
            if tracked {
                if let Some(stats) = self.history.row_stats(&id) {
                    painter.text(
                        egui::pos2(spark.left() - 14.0, rect.center().y),
                        Align2::RIGHT_CENTER,
                        format!("{:.1}", stats.max),
                        FontId::monospace(11.0),
                        p.graph,
                    );
                }
            }
            if item.graphable {
                sparkline(painter, spark, &samples, p.graph);
            }
            if response.double_clicked() {
                self.history.select_maximum(sensor);
            } else if response.clicked() {
                if selected {
                    self.history.clear_selection();
                } else {
                    self.history.select(sensor);
                }
            }
            let mut tooltip = String::from(sensor_tooltip(item.kind, self.language()));
            if let Some(stats) = self.history.row_stats(&id) {
                let language = self.language();
                tooltip.push_str(&format!(
                    "\n{:.1} {unit} · {} {:.1} · {} {:.1} · {} {:.1}",
                    stats.current,
                    language.text(Key::Min),
                    stats.min,
                    language.text(Key::Avg),
                    stats.avg,
                    language.text(Key::Max),
                    stats.max,
                ));
            }
            if let Some(pointer) = response.hover_pos() {
                if spark.contains(pointer) && samples.len() >= 2 {
                    let fraction = ((pointer.x - spark.left()) / spark.width()).clamp(0.0, 1.0);
                    let index = (fraction * (samples.len() - 1) as f32).round() as usize;
                    if let Some(value) = samples.get(index) {
                        tooltip = format!("{:.1} {unit}\n{tooltip}", value);
                    }
                }
            }
            let _ = response.on_hover_text(tooltip);
        }
        if sensors.is_empty() {
            ui.label(
                RichText::new(self.language().text(Key::Unavailable)).color(self.palette().muted),
            );
        }
    }

    fn detail_panel(&mut self, ui: &mut egui::Ui, sensors: &[SensorReading]) {
        let p = self.palette();
        let language = self.language();
        let Some(selected_id) = self.history.selected_id().cloned() else {
            return;
        };
        let Some(sensor) = sensors
            .iter()
            .find(|sensor| sensor_id(sensor) == selected_id)
        else {
            return;
        };
        ui.add_space(8.0);
        let tracked = self.history.is_tracked(&selected_id);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(&sensor.name)
                        .size(15.0)
                        .strong()
                        .color(p.text),
                );
                let shown = if tracked {
                    self.history
                        .row_stats(&selected_id)
                        .map(|stats| stats.max)
                        .unwrap_or(sensor.value)
                } else {
                    sensor.value
                };
                ui.label(
                    RichText::new(format!("{:.1} {}", shown, sensor.unit.trim()))
                        .size(22.0)
                        .strong()
                        .color(sensor_color(sensor, p)),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if tracked {
                    ui.label(
                        RichText::new(language.text(Key::ShowingMaximum))
                            .size(10.5)
                            .strong()
                            .color(p.graph),
                    );
                }
            });
        });
        if metadata(sensor).kind == SensorKind::PerfCap {
            ui.label(RichText::new(language.text(Key::PerfCapDetail)).color(p.text));
            ui.label(RichText::new(language.text(Key::PerfCapNoGraph)).color(p.muted));
            return;
        }
        let stats = self
            .history
            .row_stats(&selected_id)
            .unwrap_or(SensorStats::initial(sensor.value));
        ui.horizontal(|ui| {
            metric_chip(
                ui,
                language.text(Key::Current),
                stats.current,
                &sensor.unit,
                p.text,
            );
            metric_chip(
                ui,
                language.text(Key::Min),
                stats.min,
                &sensor.unit,
                p.muted,
            );
            metric_chip(
                ui,
                language.text(Key::Avg),
                stats.avg,
                &sensor.unit,
                p.muted,
            );
            metric_chip(
                ui,
                language.text(Key::Max),
                stats.max,
                &sensor.unit,
                p.graph,
            );
        });
        graph(
            ui,
            &self.history.row_samples(&selected_id),
            stats,
            &sensor.unit,
            p,
        );
    }

    fn sensors_bottom_bar(&mut self, ui: &mut egui::Ui, info: &SysInfo) {
        let p = self.palette();
        let language = self.language();
        ui.horizontal(|ui| {
            let name = info.gpu_name.as_deref().unwrap_or("Unknown GPU");
            let response = ui.label(RichText::new(name).size(12.5).color(
                if info.gpu_name.is_none() {
                    p.warning
                } else {
                    p.text
                },
            ));
            if info.gpu_name.is_none() {
                response.on_hover_text(language.text(Key::UnknownGpu));
            }
            ui.label(
                RichText::new(language.refresh_hint(self.settings.refresh_interval_ms))
                    .size(11.0)
                    .color(p.muted),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(language.text(Key::Reset)).clicked() {
                    self.history.reset();
                }
            });
        });
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        let language = self.language();
        let p = self.palette();
        let ctx = ui.ctx().clone();
        let mut apply = false;
        let mut restore = false;
        let mut cancel = false;
        ui.add_space(14.0);
        ui.allocate_ui(Vec2::new(470.0, ui.available_height()), |ui| {
            if let Some(notice) = self.settings_notice.clone() {
                ui.label(RichText::new(notice).color(p.warning));
                ui.add_space(8.0);
            }
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
                    ui.label(language.text(Key::Theme));
                    egui::ComboBox::from_id_salt("theme")
                        .selected_text(match self.settings_draft.theme {
                            UiTheme::Light => language.text(Key::Light),
                            UiTheme::Dark => language.text(Key::Dark),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.settings_draft.theme,
                                UiTheme::Light,
                                language.text(Key::Light),
                            );
                            ui.selectable_value(
                                &mut self.settings_draft.theme,
                                UiTheme::Dark,
                                language.text(Key::Dark),
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
                    ui.label(language.text(Key::TrackAllMaxima));
                    ui.checkbox(&mut self.settings_draft.track_all_maxima, "");
                    ui.end_row();
                    ui.label(language.text(Key::Autostart));
                    ui.checkbox(&mut self.settings_draft.autostart, "");
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
                        cancel = true;
                    }
                });
            });
        });
        if restore {
            self.settings_draft = AppSettings::default();
        }
        if cancel {
            self.settings_draft = self.settings.clone();
        }
        if apply {
            if let Err(error) = crate::autostart::set(self.settings_draft.autostart) {
                self.settings_notice = Some(error);
            }
            match settings::save(&self.settings_draft) {
                Ok(()) => {
                    self.settings = self.settings_draft.clone();
                    self.refresh_interval
                        .store(self.settings.refresh_interval_ms, Ordering::Release);
                    configure_style(&ctx, self.settings.theme, self.accent());
                    self.settings_notice = None;
                }
                Err(error) => self.settings_notice = Some(error),
            }
        }
    }

    fn feedback_tab(&mut self, ui: &mut egui::Ui) {
        let language = self.language();
        let p = self.palette();
        let mut submit = false;
        ui.add_space(14.0);
        ui.allocate_ui(Vec2::new(660.0, ui.available_height()), |ui| {
            ui.label(RichText::new(language.text(Key::FeedbackPrivacy)).color(p.text));
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
                            self.accent()
                        } else {
                            p.hover
                        }),
                    )
                    .clicked()
                {
                    submit = true;
                }
                let status = self.feedback_status.localized(language);
                if !status.is_empty() {
                    let color = if matches!(self.feedback_status, FeedbackStatus::Accepted(_)) {
                        self.accent()
                    } else {
                        p.text
                    };
                    ui.label(RichText::new(status).color(color));
                }
            });
        });
        if submit {
            let ctx = ui.ctx().clone();
            self.start_feedback(&ctx);
        }
    }

    fn unavailable(&self, ui: &mut egui::Ui, detail: &str) {
        let p = self.palette();
        ui.add_space(48.0);
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.language().text(Key::Unavailable))
                        .size(17.0)
                        .strong()
                        .color(p.danger),
                );
                ui.add_space(10.0);
                ui.label(RichText::new(detail).color(p.text));
            });
        });
    }

    fn about_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let language = self.language();
        if self.about_icon.is_none() {
            self.about_icon = load_icon_texture(ui.ctx());
        }
        ui.add_space(28.0);
        ui.allocate_ui(Vec2::new(480.0, ui.available_height()), |ui| {
            ui.horizontal(|ui| {
                if let Some(icon) = &self.about_icon {
                    ui.add(egui::Image::new((icon.id(), Vec2::splat(72.0))));
                    ui.add_space(12.0);
                }
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("GPU SHARK")
                            .size(24.0)
                            .strong()
                            .color(p.graph),
                    );
                    ui.label(
                        RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                            .size(12.0)
                            .color(p.muted),
                    );
                });
            });
            ui.add_space(14.0);
            ui.label(RichText::new(language.text(Key::AboutTagline)).color(p.text));
            ui.add_space(8.0);
            ui.label(RichText::new(language.text(Key::AboutReadOnly)).color(p.text));
            ui.label(RichText::new(language.text(Key::AboutLicense)).color(p.text));
            ui.add_space(14.0);
            ui.separator();
            ui.label(
                RichText::new(if matches!(language, Language::Russian) {
                    "Шрифт: Ubuntu — лицензия Ubuntu Font License (см. assets/fonts)."
                } else {
                    "Font: Ubuntu — Ubuntu Font License (see assets/fonts)."
                })
                .size(10.5)
                .color(p.muted),
            );
        });
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

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        "ubuntu_light".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/Ubuntu-Light.ttf"
        ))),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("ubuntu_light".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("ubuntu_light".to_owned());
    ctx.set_fonts(fonts);
}

fn load_icon_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let image = decode_bmp(include_bytes!("../assets/icon128.bmp"))?;
    Some(ctx.load_texture("app-icon", image, egui::TextureOptions::LINEAR))
}

fn decode_bmp(data: &[u8]) -> Option<egui::ColorImage> {
    if data.len() < 54 || data[0] != b'B' || data[1] != b'M' {
        return None;
    }
    let offset = u32::from_le_bytes(data[10..14].try_into().ok()?) as usize;
    let width = i32::from_le_bytes(data[18..22].try_into().ok()?);
    let height = i32::from_le_bytes(data[22..26].try_into().ok()?);
    let bpp = u16::from_le_bytes(data[28..30].try_into().ok()?);
    let compression = u32::from_le_bytes(data[30..34].try_into().ok()?);
    if bpp != 32 || compression != 0 || width <= 0 {
        return None;
    }
    let width = width as usize;
    let top_down = height < 0;
    let rows = height.unsigned_abs() as usize;
    let has_alpha = data[offset..]
        .chunks_exact(4)
        .take(width * rows)
        .any(|pixel| pixel[3] != 0);
    let mut pixels = Vec::with_capacity(width * rows);
    for row in 0..rows {
        let source = if top_down { row } else { rows - 1 - row };
        let base = offset + source * width * 4;
        for column in 0..width {
            let i = base + column * 4;
            let blue = *data.get(i)?;
            let green = *data.get(i + 1)?;
            let red = *data.get(i + 2)?;
            let mut alpha = *data.get(i + 3)?;
            if !has_alpha {
                alpha = 255;
            }
            pixels.push(Color32::from_rgba_unmultiplied(red, green, blue, alpha));
        }
    }
    Some(egui::ColorImage::new([width, rows], pixels))
}

fn connect_message(language: UiLanguage) -> String {
    if language == UiLanguage::Russian {
        "Подключение к локальному источнику телеметрии…".into()
    } else {
        "Connecting to the local telemetry provider…".into()
    }
}

impl eframe::App for GpuSharkApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_updates();
        let p = self.palette();
        egui::Panel::top("tab-bar")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(p.chrome)
                    .inner_margin(egui::Margin::symmetric(10, 4)),
            )
            .show(ui, |ui| self.tab_bar(ui));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.background)
                    .inner_margin(egui::Margin::symmetric(12, 6)),
            )
            .show(ui, |ui| match self.tab {
                Tab::Sensors => self.sensors_tab(ui),
                Tab::Settings => self.settings_tab(ui),
                Tab::Feedback => self.feedback_tab(ui),
                Tab::About => self.about_tab(ui),
            });
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

fn configure_style(ctx: &egui::Context, theme: UiTheme, accent: Color32) {
    let egui_theme = match theme {
        UiTheme::Light => egui::Theme::Light,
        UiTheme::Dark => egui::Theme::Dark,
    };
    ctx.set_theme(egui_theme);
    let p = palette(theme, accent);
    let mut style = (*ctx.style_of(egui_theme)).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.visuals = match theme {
        UiTheme::Light => egui::Visuals::light(),
        UiTheme::Dark => egui::Visuals::dark(),
    };
    style.visuals.panel_fill = p.background;
    style.visuals.extreme_bg_color = p.plot_bg;
    style.visuals.faint_bg_color = p.hover;
    style.visuals.text_edit_bg_color = Some(p.edit_bg);
    style.visuals.selection.bg_fill = p.selection;
    style.visuals.selection.stroke = Stroke::new(1.0, p.graph);
    style.visuals.widgets.inactive.bg_fill = p.hover;
    style.visuals.widgets.inactive.fg_stroke.color = p.text;
    style.visuals.widgets.hovered.bg_fill = p.hover;
    style.visuals.widgets.active.bg_fill = p.selection;
    ctx.set_style_of(egui_theme, style);
}

fn sparkline(painter: &egui::Painter, rect: Rect, samples: &[f32], color: Color32) {
    if samples.len() < 2 {
        return;
    }
    let min = samples.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = samples.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let padding = ((max - min).abs() * 0.2).max(0.3);
    let lower = min - padding;
    let upper = max + padding;
    let span = (upper - lower).max(0.01);
    let points = samples
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = egui::lerp(
                rect.left()..=rect.right(),
                index as f32 / (samples.len() - 1) as f32,
            );
            let y = egui::lerp(
                rect.bottom()..=rect.top(),
                ((*value - lower) / span).clamp(0.0, 1.0),
            );
            egui::pos2(x, y)
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(1.5, color)));
}

fn graph(ui: &mut egui::Ui, samples: &[f32], stats: SensorStats, unit: &str, p: Palette) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 128.0), Sense::hover());
    ui.painter().rect_filled(rect, 6.0, p.plot_bg);
    let plot = rect.shrink2(Vec2::new(12.0, 18.0));
    for step in 0..=4 {
        let y = egui::lerp(plot.top()..=plot.bottom(), step as f32 / 4.0);
        ui.painter().line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            Stroke::new(1.0, p.divider.gamma_multiply(0.55)),
        );
    }
    let padding = ((stats.max - stats.min).abs() * 0.20)
        .max(stats.max.abs() * 0.03)
        .max(0.5);
    let lower = stats.min - padding;
    let upper = stats.max + padding;
    let span = (upper - lower).max(0.01);
    ui.painter().text(
        plot.left_top(),
        Align2::LEFT_TOP,
        format!("{upper:.1}"),
        FontId::monospace(10.0),
        p.muted,
    );
    ui.painter().text(
        plot.left_bottom(),
        Align2::LEFT_BOTTOM,
        format!("{lower:.1}"),
        FontId::monospace(10.0),
        p.muted,
    );
    if samples.len() < 2 {
        return;
    }
    let points = samples
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = egui::lerp(
                plot.left()..=plot.right(),
                index as f32 / (samples.len() - 1) as f32,
            );
            let y = egui::lerp(
                plot.bottom()..=plot.top(),
                ((*value - lower) / span).clamp(0.0, 1.0),
            );
            egui::pos2(x, y)
        })
        .collect();
    ui.painter()
        .add(egui::Shape::line(points, Stroke::new(2.2, p.graph)));
    let mut hovered = None;
    if let Some(pointer) = response.hover_pos() {
        if plot.contains(pointer) {
            let fraction = ((pointer.x - plot.left()) / plot.width()).clamp(0.0, 1.0);
            let index = (fraction * (samples.len() - 1) as f32).round() as usize;
            if let Some(value) = samples.get(index) {
                let x = egui::lerp(
                    plot.left()..=plot.right(),
                    index as f32 / (samples.len() - 1) as f32,
                );
                let y = egui::lerp(
                    plot.bottom()..=plot.top(),
                    ((*value - lower) / span).clamp(0.0, 1.0),
                );
                ui.painter().line_segment(
                    [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
                    Stroke::new(1.0, p.divider),
                );
                ui.painter().circle_filled(egui::pos2(x, y), 3.5, p.graph);
                hovered = Some(*value);
            }
        }
    }
    let _ = match hovered {
        Some(value) => response.on_hover_text(format!("{value:.1} {unit}")),
        None => response,
    };
}

fn metric_chip(ui: &mut egui::Ui, label: &str, value: f32, unit: &str, color: Color32) {
    egui::Frame::new()
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.5)))
        .corner_radius(4.0)
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

fn sensor_color(sensor: &SensorReading, p: Palette) -> Color32 {
    match metadata(sensor).kind {
        SensorKind::HotspotTemperature if sensor.value >= 90.0 => p.danger,
        SensorKind::GpuCoreTemperature if sensor.value >= 83.0 => p.danger,
        SensorKind::MemoryTemperature if sensor.value >= 80.0 => p.warning,
        _ => p.value,
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
        (_, Language::Russian) => "Клик — график детали; двойной клик включает максимум сессии.",
        (_, Language::English) => {
            "Click for the detail graph; double-click toggles the session maximum."
        }
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
        let sensors = sensors
            .iter()
            .map(|(name, value, unit)| {
                serde_json::json!({"name": name, "value": value, "unit": unit})
            })
            .collect::<Vec<_>>();
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
