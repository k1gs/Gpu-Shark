/// UI strings are kept out of rendering code so additional translations don't
/// require touching layout or telemetry semantics.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum Language {
    English,
    Russian,
}

impl Language {
    pub const fn from_russian(russian: bool) -> Self {
        if russian {
            Self::Russian
        } else {
            Self::English
        }
    }
    pub const fn text(self, key: Key) -> &'static str {
        match (self, key) {
            (Self::Russian, Key::Sensors) => "ДАТЧИКИ",
            (Self::Russian, Key::Selected) => "ВЫБРАННЫЙ ДАТЧИК",
            (Self::Russian, Key::GpuActivity) => "АКТИВНОСТЬ GPU",
            (Self::Russian, Key::System) => "СИСТЕМА",
            (Self::Russian, Key::ChooseSensor) => "Выберите датчик",
            (Self::Russian, Key::ChooseSensorHint) => {
                "Нажмите на значение, чтобы начать запись истории."
            }
            (Self::Russian, Key::History) => "ИСТОРИЯ",
            (Self::Russian, Key::Reset) => "СБРОС",
            (Self::Russian, Key::Max) => "МАКС",
            (Self::Russian, Key::Min) => "МИН",
            (Self::Russian, Key::Feedback) => "ОБРАТНАЯ СВЯЗЬ",
            (Self::Russian, Key::FeedbackPrivacy) => {
                "В отчёт попадут только название GPU, публичные датчики и введённый вами текст."
            }
            (Self::Russian, Key::FeedbackContact) => "Контакт для ответа (необязательно):",
            (Self::Russian, Key::FeedbackDescription) => "Описание проблемы:",
            (Self::Russian, Key::FeedbackConsent) => {
                "Я согласен отправить указанные данные на сервер обратной связи."
            }
            (Self::Russian, Key::FeedbackSending) => "ОТПРАВКА…",
            (Self::Russian, Key::FeedbackSubmit) => "ОТПРАВИТЬ ОТЧЁТ",
            (Self::Russian, Key::About) => "О ПРОГРАММЕ",
            (Self::Russian, Key::AboutTagline) => "Компактный монитор телеметрии GPU.",
            (Self::Russian, Key::AboutReadOnly) => {
                "Только мониторинг. Управление оборудованием отсутствует."
            }
            (Self::Russian, Key::AboutLicense) => {
                "Публичный GUI распространяется по лицензии Beerware."
            }
            (Self::Russian, Key::PerfCapDetail) => {
                "Текущая причина ограничения производительности NVIDIA."
            }
            (Self::Russian, Key::PerfCapNoGraph) => {
                "Категориальное состояние без числового графика."
            }
            (Self::Russian, Key::Back) => "НАЗАД",
            (Self::Russian, Key::Settings) => "НАСТРОЙКИ",
            (Self::Russian, Key::Beta) => "БЕТА",
            (Self::Russian, Key::UnknownGpu) => {
                "Неизвестная видеокарта — отправьте отчёт через обратную связь."
            }
            (Self::Russian, Key::Unavailable) => "ТЕЛЕМЕТРИЯ НЕДОСТУПНА",
            (Self::Russian, Key::UpdateCheck) => "Проверить обновления",
            (Self::Russian, Key::UpdateChecking) => "Проверка обновлений…",
            (Self::Russian, Key::UpdateAvailable) => "Доступна новая версия:",
            (Self::Russian, Key::UpdateOpen) => "Открыть страницу загрузки",
            (Self::Russian, Key::UpdateUpToDate) => "У вас последняя версия.",
            (Self::Russian, Key::UpdateFailed) => "Не удалось проверить обновления.",
            (Self::Russian, Key::UpdateInstall) => "Скачать и установить",
            (Self::Russian, Key::UpdateRestart) => "Перезапустить и обновить",
            (Self::Russian, Key::UpdateDownloading) => "Загрузка обновления…",
            (Self::Russian, Key::UpdateReady) => "Обновление загружено и проверено.",
            (Self::Russian, Key::UpdateInstallFailed) => "Не удалось установить обновление.",
            (Self::Russian, Key::UpdateDisabled) => {
                "Автопроверка обновлений отключена в настройках."
            }
            (Self::English, Key::Sensors) => "SENSORS",
            (Self::English, Key::Selected) => "SELECTED SENSOR",
            (Self::English, Key::GpuActivity) => "GPU ACTIVITY",
            (Self::English, Key::System) => "SYSTEM",
            (Self::English, Key::ChooseSensor) => "Choose a sensor",
            (Self::English, Key::ChooseSensorHint) => "Click a value to start its local history.",
            (Self::English, Key::History) => "HISTORY",
            (Self::English, Key::Reset) => "RESET",
            (Self::English, Key::Max) => "MAX",
            (Self::English, Key::Min) => "MIN",
            (Self::English, Key::Feedback) => "FEEDBACK",
            (Self::English, Key::FeedbackPrivacy) => {
                "The report includes only the GPU name, public sensors, and the text you enter."
            }
            (Self::English, Key::FeedbackContact) => "Contact for a reply (optional):",
            (Self::English, Key::FeedbackDescription) => "Problem description:",
            (Self::English, Key::FeedbackConsent) => {
                "I agree to send the listed data to the feedback server."
            }
            (Self::English, Key::FeedbackSending) => "SENDING…",
            (Self::English, Key::FeedbackSubmit) => "SEND REPORT",
            (Self::English, Key::About) => "ABOUT",
            (Self::English, Key::AboutTagline) => "A compact GPU telemetry monitor.",
            (Self::English, Key::AboutReadOnly) => {
                "Monitoring only. No hardware controls are available."
            }
            (Self::English, Key::AboutLicense) => {
                "The public GUI is distributed under the Beerware license."
            }
            (Self::English, Key::PerfCapDetail) => "Current NVIDIA performance-limit reason.",
            (Self::English, Key::PerfCapNoGraph) => "Categorical state without a numeric graph.",
            (Self::English, Key::Back) => "BACK",
            (Self::English, Key::Settings) => "SETTINGS",
            (Self::English, Key::Beta) => "BETA",
            (Self::English, Key::UnknownGpu) => "Unknown GPU — send a report through feedback.",
            (Self::English, Key::Unavailable) => "TELEMETRY UNAVAILABLE",
            (Self::English, Key::UpdateCheck) => "Check for updates",
            (Self::English, Key::UpdateChecking) => "Checking for updates…",
            (Self::English, Key::UpdateAvailable) => "New version available:",
            (Self::English, Key::UpdateOpen) => "Open the download page",
            (Self::English, Key::UpdateUpToDate) => "You have the latest version.",
            (Self::English, Key::UpdateFailed) => "Could not check for updates.",
            (Self::English, Key::UpdateInstall) => "Download and install",
            (Self::English, Key::UpdateRestart) => "Restart to update",
            (Self::English, Key::UpdateDownloading) => "Downloading update…",
            (Self::English, Key::UpdateReady) => "The update is downloaded and verified.",
            (Self::English, Key::UpdateInstallFailed) => "Could not install the update.",
            (Self::English, Key::UpdateDisabled) => {
                "Automatic update checks are disabled in Settings."
            }
        }
    }

    pub fn refresh_hint(self, interval_ms: u64) -> String {
        match (self, interval_ms) {
            (Self::Russian, 500) => {
                "Обновление каждые 0,5 с · двойной клик показывает максимум".into()
            }
            (Self::Russian, 2_000) => {
                "Обновление каждые 2 с · двойной клик показывает максимум".into()
            }
            (Self::Russian, _) => {
                "Обновление раз в секунду · двойной клик показывает максимум".into()
            }
            (Self::English, 500) => {
                "Refreshes every 500 ms · double-click shows the maximum".into()
            }
            (Self::English, 2_000) => {
                "Refreshes every 2 seconds · double-click shows the maximum".into()
            }
            (Self::English, _) => "Refreshes every second · double-click shows the maximum".into(),
        }
    }

    pub const fn feedback_required(self) -> &'static str {
        match self {
            Self::Russian => "Введите сообщение и подтвердите согласие на отправку.",
            Self::English => "Enter a message and confirm consent to send it.",
        }
    }

    pub const fn feedback_sending_status(self) -> &'static str {
        match self {
            Self::Russian => "Отправка…",
            Self::English => "Sending…",
        }
    }

    pub fn feedback_accepted(self, report_id: &str) -> String {
        match self {
            Self::Russian => format!("Отчёт принят. Номер: {report_id}"),
            Self::English => format!("Report accepted. ID: {report_id}"),
        }
    }

    pub fn feedback_rejected(self, detail: &str) -> String {
        match self {
            Self::Russian => format!("Отчёт отклонён: {detail}"),
            Self::English => format!("Report rejected: {detail}"),
        }
    }

    pub const fn feedback_payload_too_large(self) -> &'static str {
        match self {
            Self::Russian => "Отчёт превышает 256 КБ (413).",
            Self::English => "The report exceeds 256 KB (413).",
        }
    }

    pub const fn feedback_rate_limited(self) -> &'static str {
        match self {
            Self::Russian => "Лимит исчерпан. Попробуйте позднее (429).",
            Self::English => "Rate limit reached. Try again later (429).",
        }
    }

    pub const fn feedback_server_error(self) -> &'static str {
        match self {
            Self::Russian => "Временная ошибка сервера. Попробуйте позднее.",
            Self::English => "Temporary server error. Try again later.",
        }
    }

    pub fn feedback_network_error(self, error: &str) -> String {
        match self {
            Self::Russian => format!("Ошибка сети: {error}"),
            Self::English => format!("Network error: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn refresh_hint_matches_supported_intervals() {
        assert_eq!(
            Language::English.refresh_hint(500),
            "Refreshes every 500 ms · double-click shows the maximum"
        );
        assert_eq!(
            Language::Russian.refresh_hint(2_000),
            "Обновление каждые 2 с · двойной клик показывает максимум"
        );
    }

    #[test]
    fn feedback_copy_is_fully_localized() {
        assert_eq!(
            Language::English.text(super::Key::FeedbackConsent),
            "I agree to send the listed data to the feedback server."
        );
        assert_eq!(
            Language::English.feedback_required(),
            "Enter a message and confirm consent to send it."
        );
        assert_eq!(
            Language::English.feedback_accepted("R-42"),
            "Report accepted. ID: R-42"
        );
        assert_eq!(
            Language::Russian.feedback_server_error(),
            "Временная ошибка сервера. Попробуйте позднее."
        );
        assert_eq!(Language::Russian.text(super::Key::About), "О ПРОГРАММЕ");
    }
}

#[derive(Clone, Copy)]
pub enum Key {
    Sensors,
    Selected,
    GpuActivity,
    System,
    ChooseSensor,
    ChooseSensorHint,
    History,
    Reset,
    Max,
    Min,
    Feedback,
    FeedbackPrivacy,
    FeedbackContact,
    FeedbackDescription,
    FeedbackConsent,
    FeedbackSending,
    FeedbackSubmit,
    About,
    AboutTagline,
    AboutReadOnly,
    AboutLicense,
    PerfCapDetail,
    PerfCapNoGraph,
    Back,
    Settings,
    Beta,
    UnknownGpu,
    Unavailable,
    UpdateCheck,
    UpdateChecking,
    UpdateAvailable,
    UpdateOpen,
    UpdateUpToDate,
    UpdateFailed,
    UpdateDisabled,
    UpdateInstall,
    UpdateRestart,
    UpdateDownloading,
    UpdateReady,
    UpdateInstallFailed,
}
