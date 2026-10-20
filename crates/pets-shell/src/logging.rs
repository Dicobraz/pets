//! Журнал процесса: tracing в stderr, `log` через tracing-log, ERROR и паники — ещё и в журнал Windows.

use std::fmt::Write as _;
use std::io::IsTerminal as _;

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt as _};
use tracing_subscriber::util::SubscriberInitExt as _;

use crate::sys;

/// Фильтр, если переменная не задана.
const DEFAULT_FILTER: &str = "info";

/// Журнал: tracing в stderr (уровень из переменной `env_var`, напр. POSTMAN_LOG, по умолчанию info;
/// формат filter::Targets — "info,gpui=warn"), записи `log` (gpui) тоже идут сюда (tracing-log);
/// события ERROR и паники дублируются в журнал Windows «Приложение» с источником `app`.
/// Хук паники: сообщение+место → tracing::error!(target: "panic") и журнал Windows, затем прежний хук.
///
/// Повторный вызов — без эффекта (подписчик уже стоит; пишется предупреждение).
pub fn init_logging(app: &str, env_var: &str) {
    let raw = std::env::var(env_var).ok();
    let (targets, bad) = match raw.as_deref().map(str::parse::<Targets>) {
        Some(Ok(t)) => (t, None),
        Some(Err(e)) => (default_targets(), Some(e)),
        None => (default_targets(), None),
    };
    sys::event_source(app);
    let stderr = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal());
    let installed = tracing_subscriber::registry()
        .with(targets)
        .with(stderr)
        .with(EventLogLayer)
        .try_init();
    if let Err(e) = installed {
        tracing::warn!(target: "pets_shell", "журнал уже настроен: {e}");
        return;
    }
    if let Some(e) = bad {
        tracing::warn!(target: "pets_shell", "{env_var}: фильтр не разобран ({e}), взят «{DEFAULT_FILTER}»");
    }
    install_panic_hook();
}

/// Запуск невозможен: tracing::error! (→ журнал Windows), eprintln, exit(1).
pub fn fatal(what: &str, e: impl std::fmt::Display) -> ! {
    tracing::error!(target: "fatal", "{what}: {e}");
    eprintln!("{what}: {e}");
    std::process::exit(1)
}

fn default_targets() -> Targets {
    Targets::new().with_default(Level::INFO)
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let msg = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("(не строка)");
        match info.location() {
            Some(l) => tracing::error!(target: "panic", "{msg} ({}:{}:{})", l.file(), l.line(), l.column()),
            None => tracing::error!(target: "panic", "{msg}"),
        }
        previous(info);
    }));
}

/// Слой tracing: события ERROR → журнал Windows (сообщение и поля одной строкой).
struct EventLogLayer;

impl<S: Subscriber> Layer<S> for EventLogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if *event.metadata().level() != Level::ERROR {
            return;
        }
        sys::report_error(&event_line(event));
    }
}

/// Строка события: `цель: сообщение имя=значение …`.
fn event_line(event: &Event<'_>) -> String {
    let mut f = Fields::default();
    event.record(&mut f);
    format!("{}: {}{}", event.metadata().target(), f.message, f.rest)
}

/// Сборщик полей: `message` отдельно, прочие — ` имя=значение`.
#[derive(Default)]
struct Fields {
    message: String,
    rest: String,
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            let _ = write!(self.rest, " {}={value}", field.name());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.rest, " {}={value:?}", field.name());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Слой-перехватчик: строка, которую получил бы журнал Windows.
    struct Capture(Arc<Mutex<Vec<String>>>);

    impl<S: Subscriber> Layer<S> for Capture {
        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            self.0.lock().unwrap().push(event_line(event));
        }
    }

    #[test]
    fn fields_are_formatted_in_one_line() {
        let out = Arc::new(Mutex::new(Vec::new()));
        let sub = tracing_subscriber::registry().with(Capture(out.clone()));
        tracing::subscriber::with_default(sub, || {
            tracing::error!(target: "imap", account = "work", code = 7, "нет связи: {}", "таймаут");
        });
        assert_eq!(*out.lock().unwrap(), ["imap: нет связи: таймаут account=work code=7"]);
    }
}
