use std::sync::OnceLock;

pub(crate) static LOG_FILE: OnceLock<std::sync::Mutex<std::fs::File>> = OnceLock::new();

pub(crate) fn init_logging() {
    if let Ok(path) = std::env::var("EMUX_LOG")
        && let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
    {
        let _ = LOG_FILE.set(std::sync::Mutex::new(file));
    }
}

macro_rules! emux_log {
    ($($arg:tt)*) => {
        if let Some(file) = $crate::logging::LOG_FILE.get() {
            if let Ok(mut f) = file.lock() {
                use std::io::Write as _;
                let _ = writeln!(f, "[{}] {}", $crate::logging::chrono_now(), format_args!($($arg)*));
            }
        }
    };
}

pub(crate) fn chrono_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let ms = dur.subsec_millis();
    format!("{secs}.{ms:03}")
}
