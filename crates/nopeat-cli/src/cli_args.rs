use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "nopeat",
    version,
    about = "Analyse bundler output (stats.json, *.map, dist folders) in one pass",
    after_help = "The flags of webpack-bundle-analyzer keep working: \
                  nopeat stats.json dist --mode static -r report.html is a drop-in swap."
)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    pub path: PathBuf,

    #[arg(value_name = "BUNDLE_DIR")]
    pub bundle_dir: Option<PathBuf>,

    #[arg(short, long, value_enum, default_value_t = Mode::Static)]
    pub mode: Mode,

    #[arg(long, default_value = "127.0.0.1", value_name = "HOST")]
    pub host: String,

    #[arg(short = 'p', long, default_value = "8888", value_name = "PORT")]
    pub port: String,

    #[arg(short, long, value_name = "FILE")]
    pub report: Option<PathBuf>,

    #[arg(short = 't', long, value_name = "TITLE")]
    pub title: Option<String>,

    #[arg(short, long, value_enum, default_value_t = Sizes::Parsed)]
    pub default_sizes: Sizes,

    #[arg(long, value_enum, default_value_t = Compression::Gzip, value_name = "TYPE")]
    pub compression_algorithm: Compression,

    #[arg(short = 'O', long)]
    pub no_open: bool,

    #[arg(short = 'e', long = "exclude", value_name = "REGEX")]
    pub exclude: Vec<String>,

    #[arg(short = 'i', long = "include", value_name = "REGEX")]
    pub include: Vec<String>,

    #[arg(long, value_name = "BYTES")]
    pub min_size: Option<u64>,

    #[arg(long, value_name = "FILE")]
    pub csv: Option<PathBuf>,

    #[arg(long, value_name = "FILE")]
    pub budget: Option<PathBuf>,

    #[arg(long)]
    pub json: bool,

    #[arg(long, value_enum, value_delimiter = ',')]
    pub dims: Vec<Dims>,

    #[arg(short = 'l', long, value_enum, default_value_t = LogLevel::Info)]
    pub log_level: LogLevel,

    #[arg(long, hide = true)]
    pub bench: bool,

    #[arg(long, hide = true)]
    pub bench_map: bool,
}

impl Cli {
    pub fn port_number(&self) -> Result<u16> {
        if self.port == "auto" {
            return Ok(0);
        }
        self.port.parse::<u16>().map_err(|_| {
            anyhow::anyhow!("invalid port `{}`: should be a number or `auto`", self.port)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    Static,
    Json,
    Server,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Sizes {
    Stat,
    Parsed,
    Gzip,
    Brotli,
    Zstd,
    Attributed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Compression {
    Gzip,
    Brotli,
    Zstd,
}

impl Compression {
    pub fn as_str(self) -> &'static str {
        match self {
            Compression::Gzip => "gzip",
            Compression::Brotli => "brotli",
            Compression::Zstd => "zstd",
        }
    }

    pub fn to_core(self) -> nopeat_core::sizes::Compression {
        match self {
            Compression::Gzip => nopeat_core::sizes::Compression::Gzip,
            Compression::Brotli => nopeat_core::sizes::Compression::Brotli,
            Compression::Zstd => nopeat_core::sizes::Compression::Zstd,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Silent,
}

impl LogLevel {
    pub fn shows(self, level: Level) -> bool {
        match level {
            Level::Error => self <= LogLevel::Error,
            Level::Warn => self <= LogLevel::Warn,
            Level::Info => self <= LogLevel::Info,
            Level::Debug => self <= LogLevel::Debug,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Dims {
    Package,
    Source,
    Chunk,
    Ext,
}

pub fn validate(cli: &Cli) -> Result<()> {
    cli.port_number()?;
    if cli.bundle_dir.is_some() && cli.path.is_dir() {
        bail!(
            "a bundle directory is already given as the input `{}`; the second \
             argument is only used when the input is a stats/metafile JSON file",
            cli.path.display()
        );
    }
    Ok(())
}
