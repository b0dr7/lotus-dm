use crate::core::settings::{Lang, TrafficMode};
use crate::engine::torrent::EncryptionMode;
use crate::ui::theme::ThemeMode;

/// Translatable labels. One variant per visible string so a missing
/// translation is a compile error rather than a silent English fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    SettingsTitle,
    Language,
    LanguageHint,
    Theme,
    ThemeHint,
    DefaultFolder,
    Browse,
    GlobalSpeedLimit,
    GlobalSpeedHint,
    SpeedSuffix,
    TrafficMode,
    TrafficHint,
    MaxConns,
    MaxConnsHint,
    MaxConcurrent,
    MaxConcurrentHint,
    Torrents,
    Encryption,
    EncryptionHint,
    DhtLsdHint,
    DhtCheck,
    LsdCheck,
    SeedHint,
    RatioSuffix,
    MinSuffix,
    AutoShutdown,
    AutoShutdownHint,
    AutoShutdownCheck,
    LaunchAtStartup,
    LaunchHint,
    LaunchCheck,
    Portable,
    PortableHint,
    Tray,
    TrayHint,
    TrayClose,
    TrayHidden,
    NotifyCheck,
    Clipboard,
    ClipboardHint,
    ClipboardCheck,
    AutoStart,
    AutoStartHint,
    AutoStartCheck,
    Bridge,
    BridgeHint,
    BridgeCheck,
    BridgePortHint,
    ListeningOn,
    PortBusy,
    BridgeOff,
    ExtensionSetup,
    ExtensionHint,
    NavHome,
    NavQueue,
    NavSite,
    NavSpider,
    NavSettings,
    NavHelp,
}

pub fn tr(lang: Lang, key: Key) -> &'static str {
    let (en, ru) = pair(key);
    pick(lang, en, ru)
}

fn pick(lang: Lang, en: &'static str, ru: &'static str) -> &'static str {
    match lang {
        Lang::En => en,
        Lang::Ru => ru,
    }
}

pub fn theme_label(lang: Lang, mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => pick(lang, "System", "Системная"),
        ThemeMode::Dark => pick(lang, "Dark", "Тёмная"),
        ThemeMode::Light => pick(lang, "Light", "Светлая"),
        ThemeMode::Amoled => "AMOLED",
        ThemeMode::HighContrast => pick(lang, "High contrast", "Контрастная"),
    }
}

pub fn traffic_label(lang: Lang, mode: TrafficMode) -> &'static str {
    match mode {
        TrafficMode::Custom => pick(lang, "Custom", "Свой"),
        TrafficMode::Low => pick(lang, "Low", "Низкий"),
        TrafficMode::Medium => pick(lang, "Medium", "Средний"),
        TrafficMode::High => pick(lang, "High", "Высокий"),
        TrafficMode::Unlimited => pick(lang, "Unlimited", "Без ограничений"),
    }
}

pub fn encryption_label(lang: Lang, mode: EncryptionMode) -> &'static str {
    match mode {
        EncryptionMode::Disabled => pick(lang, "Disabled", "Выключено"),
        EncryptionMode::Enabled => pick(lang, "Enabled", "Включено"),
        EncryptionMode::Forced => pick(lang, "Forced", "Принудительно"),
    }
}

fn pair(key: Key) -> (&'static str, &'static str) {
    match key {
        Key::SettingsTitle => ("Settings", "Настройки"),
        Key::Language => ("Language", "Язык"),
        Key::LanguageHint => (
            "Language of the interface.",
            "Язык интерфейса.",
        ),
        Key::Theme => ("Theme", "Тема"),
        Key::ThemeHint => (
            "System follows the Windows light and dark setting.",
            "Системная следует за светлой и тёмной темой Windows.",
        ),
        Key::DefaultFolder => ("Default download folder", "Папка загрузок по умолчанию"),
        Key::Browse => ("Browse", "Обзор"),
        Key::GlobalSpeedLimit => ("Global speed limit", "Общий лимит скорости"),
        Key::GlobalSpeedHint => (
            "Shared by every running download. 0 = unlimited. Per-download limits override it.",
            "Делится между всеми загрузками. 0 - без ограничений. Лимит отдельной загрузки важнее.",
        ),
        Key::SpeedSuffix => (" KB/s", " КБ/с"),
        Key::TrafficMode => ("Traffic mode", "Режим трафика"),
        Key::TrafficHint => (
            "Low / Medium / High set a shared speed cap. Custom keeps the value above. Unlimited removes it.",
            "Низкий / Средний / Высокий задают общий потолок. Свой оставляет значение выше. Без ограничений снимает его.",
        ),
        Key::MaxConns => ("Max connections per server", "Соединений на сервер"),
        Key::MaxConnsHint => (
            "Segments used for one file (1-32). Default 8.",
            "Сегментов на один файл (1-32). По умолчанию 8.",
        ),
        Key::MaxConcurrent => ("Max simultaneous downloads", "Одновременных загрузок"),
        Key::MaxConcurrentHint => (
            "How many run at once; the rest wait in the queue. 0 = no limit. Default 3.",
            "Сколько качается сразу; остальные ждут в очереди. 0 - без ограничения. По умолчанию 3.",
        ),
        Key::Torrents => ("Torrents", "Торренты"),
        Key::Encryption => ("Protocol encryption", "Шифрование протокола"),
        Key::EncryptionHint => (
            "MSE. Enabled prefers encrypted peers and still allows plaintext; Forced only connects to peers that complete the handshake. rqbit sets this per session, so it applies to the next torrent once no download is running.",
            "MSE. Включено предпочитает шифрованных пиров, но и открытых пускает; Принудительно пускает только тех, кто прошёл рукопожатие. rqbit задаёт это на сессию, поэтому значение подхватит следующий торрент, когда ничего не качается.",
        ),
        Key::DhtLsdHint => (
            "DHT and local-network discovery are set the same way: an idle engine is rebuilt at once, and a running download picks them up when nothing is downloading any more.",
            "DHT и поиск в локальной сети задаются так же: свободный движок пересобирается сразу, а идущая загрузка подхватит их, когда качать уже нечего.",
        ),
        Key::DhtCheck => (
            "Use the DHT (needed for magnet links without trackers)",
            "Использовать DHT (нужно для magnet-ссылок без трекеров)",
        ),
        Key::LsdCheck => (
            "Find peers on the local network",
            "Искать пиров в локальной сети",
        ),
        Key::SeedHint => (
            "Stop seeding at this share ratio and/or after this many minutes. 0 and 0 stop as soon as the download is complete.",
            "Прекращать раздачу при таком рейтинге и/или через столько минут. 0 и 0 - сразу после загрузки.",
        ),
        Key::RatioSuffix => (" ratio", " рейтинг"),
        Key::MinSuffix => (" min", " мин"),
        Key::AutoShutdown => ("Auto-shutdown", "Автовыключение"),
        Key::AutoShutdownHint => (
            "Close the app automatically once the queue finishes.",
            "Закрывать приложение, когда очередь опустеет.",
        ),
        Key::AutoShutdownCheck => (
            "Shut down when the queue is done",
            "Выключать, когда очередь закончена",
        ),
        Key::LaunchAtStartup => ("Launch at startup", "Запуск при старте"),
        Key::LaunchHint => (
            "Add Lotus to the per-user Windows Run key.",
            "Добавляет Lotus в ветку Run текущего пользователя Windows.",
        ),
        Key::LaunchCheck => ("Start with Windows", "Запускать с Windows"),
        Key::Portable => ("Portable mode", "Портативный режим"),
        Key::PortableHint => (
            "Portable mode is enabled by placing an empty file named 'portable' next to the executable. Data is then kept in a 'data' folder beside it instead of %APPDATA%.",
            "Портативный режим включается пустым файлом с именем 'portable' рядом с программой. Данные тогда лежат в папке 'data' рядом с ней, а не в %APPDATA%.",
        ),
        Key::Tray => ("Tray and notifications", "Трей и уведомления"),
        Key::TrayHint => (
            "Downloads keep running with the window hidden. The tray menu has Open, Pause all, Resume all, speed presets and Quit.",
            "Загрузки идут и со скрытым окном. В меню трея есть Открыть, Пауза всем, Продолжить всем, скорость и Выход.",
        ),
        Key::TrayClose => (
            "Close button hides to tray",
            "Кнопка закрытия прячет в трей",
        ),
        Key::TrayHidden => ("Start hidden in the tray", "Запускаться скрыто в трее"),
        Key::NotifyCheck => (
            "Notify when a download finishes or fails",
            "Уведомлять об окончании и об ошибке",
        ),
        Key::Clipboard => ("Clipboard watcher", "Наблюдение за буфером"),
        Key::ClipboardHint => (
            "While enabled and the window is focused, watch for copied URLs every 2 seconds.",
            "Пока включено и окно в фокусе, буфер проверяется на ссылки каждые 2 секунды.",
        ),
        Key::ClipboardCheck => (
            "Watch clipboard for URLs",
            "Следить за ссылками в буфере",
        ),
        Key::AutoStart => ("Auto-start queue", "Автозапуск очереди"),
        Key::AutoStartHint => (
            "When on, queued downloads start automatically at this time every day.",
            "Если включено, загрузки в очереди стартуют сами в это время каждый день.",
        ),
        Key::AutoStartCheck => ("Auto-start queue at", "Автозапуск очереди в"),
        Key::Bridge => ("Browser bridge", "Мост для браузера"),
        Key::BridgeHint => (
            "Lets the Lotus extension hand links to the queue. Listens on 127.0.0.1 only - nothing is reachable from other machines.",
            "Расширение Lotus передаёт ссылки прямо в очередь. Слушает только 127.0.0.1 - с других машин недоступен.",
        ),
        Key::BridgeCheck => (
            "Accept links from the browser extension",
            "Принимать ссылки от расширения",
        ),
        Key::BridgePortHint => ("port - takes effect after restart", "порт - после перезапуска"),
        Key::ListeningOn => ("Listening on ", "Слушает "),
        Key::PortBusy => (
            "Not listening - the port is busy. Change it and restart.",
            "Не слушает - порт занят. Измените его и перезапустите.",
        ),
        Key::BridgeOff => (
            "Off - the browser extension cannot reach this app.",
            "Выключено - расширение не может подключиться к приложению.",
        ),
        Key::ExtensionSetup => ("Extension setup", "Настройка расширения"),
        Key::ExtensionHint => (
            "Firefox: install Lotus from addons.mozilla.org. Chromium: download the .zip from the Lotus site, unzip it, open the extension page, turn on Developer mode, then click Load unpacked and pick the unzipped folder. Then pin Lotus to the toolbar. The bridge line above should read Listening.",
            "Firefox: поставьте Lotus с addons.mozilla.org. Chromium: скачайте .zip с сайта Lotus, распакуйте, откройте страницу расширений, включите режим разработчика, нажмите Загрузить распакованное и выберите папку. Затем закрепите Lotus на панели. Строка моста выше должна показывать Слушает.",
        ),
        Key::NavHome => ("Home", "Главная"),
        Key::NavQueue => ("Queue", "Очередь"),
        Key::NavSite => ("Site", "Сайт"),
        Key::NavSpider => ("Spider", "Паук"),
        Key::NavSettings => ("Settings", "Настройки"),
        Key::NavHelp => ("Help", "Справка"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_text_in_both_languages() {
        for lang in Lang::all() {
            for key in [
                Key::SettingsTitle,
                Key::Language,
                Key::Theme,
                Key::DefaultFolder,
                Key::GlobalSpeedHint,
                Key::EncryptionHint,
                Key::PortableHint,
                Key::ExtensionHint,
                Key::PortBusy,
            ] {
                assert!(!tr(lang, key).trim().is_empty());
            }
        }
    }

    #[test]
    fn lang_code_round_trips() {
        assert_eq!(Lang::from_code("ru"), Lang::Ru);
        assert_eq!(Lang::from_code(" RU "), Lang::Ru);
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("de"), Lang::En);
        assert_eq!(Lang::Ru.code(), "ru");
        assert_eq!(Lang::En.code(), "en");
    }
}
