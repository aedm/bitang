mod control;
mod file;
mod loader;
mod render;
mod tool;

use crate::tool::run_app;
use anyhow::Result;
use build_time::build_time_local;
use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    set_up_tracing()?;
    if VERSION == "0.0.0" {
        info!("Bitang dev version, build time {}", build_time_local!());
    } else {
        info!("Bitang {VERSION}");
    }

    run_app()?;
    Ok(())
}

fn set_up_tracing() -> Result<()> {
    #[cfg(windows)]
    let with_color = nu_ansi_term::enable_ansi_support().is_ok();
    #[cfg(not(windows))]
    let with_color = true;

    let crate_filter = tracing_subscriber::filter::filter_fn(|metadata| {
        metadata.target().starts_with("bitang")
    });
    let fmt_layer = fmt::layer().with_ansi(with_color).with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(if cfg!(debug_assertions) { "debug" } else { "info" }))?;
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(crate_filter)
        .init();

    Ok(())
}
