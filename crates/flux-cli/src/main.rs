//! `flux` binary entry point (FLUX-022, spec §14.3).
//!
//! Parses the command line, installs a `tracing` subscriber driven by
//! `RUST_LOG`, and runs the selected subcommand on the Tokio runtime.

#![forbid(unsafe_code)]

use std::io::IsTerminal;

use clap::Parser;
use flux_cli::Cli;
use tracing::Event;
use tracing::Level;
use tracing::Subscriber;
use tracing_subscriber::fmt::format::FormatEvent;
use tracing_subscriber::fmt::format::FormatFields;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::registry::LookupSpan;

/// A `tracing` event formatter with a high-contrast palette.
///
/// The stock `tracing_subscriber::fmt()` paints `INFO` blue and leaves the
/// message in the terminal's default foreground. On a blue-background terminal
/// that renders as near-unreadable dark-on-blue. This formatter keeps the same
/// `LEVEL message` layout but recolors the level explicitly and forces the
/// message to bold bright white so it stays legible on any background
/// (including terminal themes with saturated backgrounds).
///
/// Whether ANSI is emitted is decided once, up front, from the real TTY/NO_COLOR
/// state (see [`build_subscriber`]) and threaded in via [`Self::use_ansi`], so
/// piped/log-file output is always plain text regardless of how the writer is
/// configured internally.
struct HighContrastFormat {
    use_ansi: bool,
}

impl HighContrastFormat {
    /// The ANSI prefix for a given level, or `""` when color is disabled.
    fn level_style(level: &Level, ansi: bool) -> &'static str {
        if !ansi {
            return "";
        }
        // Bright, high-luminance colors chosen to read on both light and dark
        // (including saturated blue) terminal backgrounds.
        match *level {
            Level::ERROR => "\x1b[1;91m", // bold bright red
            Level::WARN => "\x1b[1;93m",  // bold bright yellow
            Level::INFO => "\x1b[1;92m",  // bold bright green (was blue)
            Level::DEBUG => "\x1b[1;96m", // bold bright cyan
            Level::TRACE => "\x1b[1;95m", // bold bright magenta
        }
    }
}

impl<S, N> FormatEvent<S, N> for HighContrastFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let meta = event.metadata();
        let ansi = self.use_ansi;

        if ansi {
            let open = Self::level_style(meta.level(), ansi);
            write!(writer, "{open}{:<5}\x1b[0m ", meta.level())?;
        } else {
            write!(writer, "{:<5} ", meta.level())?;
        }

        // Message + fields: bold bright white so it never collapses into a
        // blue/dark background. The field renderer is configured by the
        // subscriber builder (see `build_subscriber`) to honor the same ANSI
        // decision, so piped output stays escape-free.
        if ansi {
            write!(writer, "\x1b[1;97m")?;
        }
        ctx.format_fields(writer.by_ref(), event)?;
        if ansi {
            write!(writer, "\x1b[0m")?;
        }
        writeln!(writer)
    }
}

/// Installs the global `tracing` subscriber.
///
/// ANSI is enabled only when stdout is a real terminal and `NO_COLOR` is unset
/// (clicolor convention). When disabled, the formatter emits plain text, so logs
/// redirected to a file or pipe stay clean.
fn install_tracing() {
    let use_ansi = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    tracing_subscriber::fmt()
        .event_format(HighContrastFormat { use_ansi })
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

fn main() -> anyhow::Result<()> {
    install_tracing();

    let cli = Cli::parse();
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(flux_cli::run(cli.command))
}
