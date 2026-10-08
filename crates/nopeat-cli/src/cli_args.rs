use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "nopeat",
    version,
    about = "Analyse bundler output (stats.json, *.map, dist folders) in one pass"
)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    pub path: PathBuf,

    #[arg(short, long, value_enum, default_value_t = Mode::Static)]
    pub mode: Mode,

    #[arg(short, long, default_value = "nopeat-report.html")]
    pub report: PathBuf,

    #[arg(short, long, value_enum, default_value_t = Sizes::Parsed)]
    pub default_sizes: Sizes,

    #[arg(short = 'e', long = "exclude", value_name = "REGEX")]
    pub exclude: Vec<String>,

    #[arg(short = 'i', long = "include", value_name = "REGEX")]
    pub include: Vec<String>,

    #[arg(long, value_name = "REGEX")]
    pub min_size: Option<u64>,

    #[arg(long, value_name = "FILE")]
    pub csv: Option<PathBuf>,

    #[arg(long, value_name = "FILE")]
    pub budget: Option<PathBuf>,

    #[arg(long, default_value_t = 8888, value_name = "PORT")]
    pub port: u16,

    #[arg(long)]
    pub json: bool,

    #[arg(long, value_enum, value_delimiter = ',')]
    pub dims: Vec<Dims>,

    #[arg(long)]
    pub include_sources: bool,

    #[arg(long)]
    pub bench: bool,

    #[arg(long)]
    pub bench_map: bool,
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

    Attributed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Dims {
    Package,

    Source,

    Chunk,

    Ext,
}
