use crate::MAIN_CONFIG;
use chrono::Local;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;

struct LocalTimer;
impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%Y-%m-%d %H:%M:%S"))
    }
}
pub fn init_tracing() {
    if MAIN_CONFIG.log_level == "debug".to_string() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_target(false)
            .with_timer(LocalTimer)
            .init();
    } else if MAIN_CONFIG.log_level == "info".to_string() || true {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .with_timer(LocalTimer)
            .init();
    }
}
