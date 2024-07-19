use camino::Utf8PathBuf;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Cli {
    pub verbose: bool,
    pub debug: bool,
    pub trace: bool,
    pub config: Utf8PathBuf,
}
