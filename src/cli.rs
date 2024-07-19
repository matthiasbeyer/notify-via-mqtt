use camino::Utf8PathBuf;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(short, long)]
    pub verbose: bool,

    #[clap(short, long)]
    pub debug: bool,

    #[clap(short, long)]
    pub trace: bool,

    #[clap(short, long)]
    pub config: Utf8PathBuf,
}
