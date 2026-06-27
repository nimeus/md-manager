//! Tracing/logging initialisation.

use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::LogFormat;

/// Initialise the global tracing subscriber. Honors `RUST_LOG`; otherwise a sane default.
pub fn init(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,mdm_api=debug,mdm_mcp=debug,mdm_db=debug,sqlx=warn")
    });

    let registry = tracing_subscriber::registry().with(filter);
    match format {
        LogFormat::Json => registry.with(fmt::layer().json()).init(),
        LogFormat::Pretty => registry.with(fmt::layer().pretty()).init(),
    }
}
