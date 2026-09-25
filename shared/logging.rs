use log::kv;
use std::fmt::Write as _;
use std::io::Write as _;
use std::{process, sync};

type SyslogLogger = syslog::Logger<syslog::LoggerBackend, syslog::Formatter3164>;

/// A syslog logger which preserves the key-values supplied to `log!` macros.
///
/// `syslog::BasicLogger` currently formats only `Record::args()`, silently
/// dropping `Record::key_values()`.  Keep the traditional RFC 3164 transport
/// and put the fields in the message body so existing syslog deployments keep
/// working.
pub struct KvLogger {
    logger: sync::Arc<sync::Mutex<SyslogLogger>>,
}

impl KvLogger {
    fn new(logger: SyslogLogger) -> Self {
        Self {
            logger: sync::Arc::new(sync::Mutex::new(logger)),
        }
    }
}

impl log::Log for KvLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::max_level() && metadata.level() <= log::STATIC_MAX_LEVEL
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let message = format_message(record);
        let Ok(mut logger) = self.logger.lock() else {
            return;
        };

        let result = match record.level() {
            log::Level::Error => logger.err(message),
            log::Level::Warn => logger.warning(message),
            log::Level::Info => logger.info(message),
            log::Level::Debug | log::Level::Trace => logger.debug(message),
        };
        let _ = result;
    }

    fn flush(&self) {
        if let Ok(mut logger) = self.logger.lock() {
            let _ = logger.backend.flush();
        }
    }
}

struct FieldVisitor {
    fields: String,
}

impl<'kvs> kv::VisitSource<'kvs> for FieldVisitor {
    fn visit_pair(&mut self, key: kv::Key<'kvs>, value: kv::Value<'kvs>) -> Result<(), kv::Error> {
        if !self.fields.is_empty() {
            self.fields.push(' ');
        }
        let _ = write!(self.fields, "{}={}", key.as_str(), value);
        Ok(())
    }
}

fn format_message(record: &log::Record<'_>) -> String {
    let mut message = record.args().to_string();
    let mut visitor = FieldVisitor {
        fields: String::new(),
    };

    if let Err(error) = record.key_values().visit(&mut visitor) {
        if !visitor.fields.is_empty() {
            message.push(' ');
        }
        let _ = write!(message, "kv_error={error}");
    }

    if !visitor.fields.is_empty() {
        message.push(' ');
        message.push_str(&visitor.fields);
    }
    message
}

pub fn init_syslog(
    facility: syslog::Facility,
    log_level: log::LevelFilter,
    process_name: &str,
) -> syslog::Result<()> {
    let formatter = syslog::Formatter3164 {
        facility,
        hostname: None,
        process: process_name.to_owned(),
        pid: process::id(),
    };

    let logger = if let Ok(logger) = syslog::unix(formatter.clone()) {
        logger
    } else if let Ok(logger) = syslog::tcp(formatter.clone(), ("127.0.0.1", 601)) {
        logger
    } else {
        syslog::udp(formatter, ("0.0.0.0", 0), ("127.0.0.1", 514))?
    };

    log::set_boxed_logger(Box::new(KvLogger::new(logger)))
        .map_err(|error| syslog::Error::Initialization(Box::new(error)))?;
    log::set_max_level(log_level);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::format_message;

    #[test]
    fn includes_key_values_after_message() {
        let record = log::Record::builder()
            .args(format_args!("session added"))
            .key_values(&[("session_id", 42)])
            .build();

        assert_eq!(format_message(&record), "session added session_id=42");
    }

    #[test]
    fn supports_values_with_spaces() {
        let record = log::Record::builder()
            .args(format_args!("assigned"))
            .key_values(&[("seat", "seat 1")])
            .build();

        assert_eq!(format_message(&record), "assigned seat=seat 1");
    }

    #[test]
    fn preserves_messages_without_key_values() {
        let record = log::Record::builder()
            .args(format_args!("application failed: connection refused"))
            .build();

        assert_eq!(
            format_message(&record),
            "application failed: connection refused"
        );
    }

    #[test]
    fn includes_all_error_fields() {
        let record = log::Record::builder()
            .args(format_args!("failed to open device"))
            .key_values(&[
                ("path", "/dev/input/event0"),
                ("error", "Permission denied"),
            ])
            .build();

        assert_eq!(
            format_message(&record),
            "failed to open device path=/dev/input/event0 error=Permission denied"
        );
    }
}
