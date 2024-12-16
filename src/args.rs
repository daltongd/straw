use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    /// Replace the existing symlinks with symlinks to .dotfiles
    #[clap(long, short, action)]
    pub preview: bool,

    #[clap(long, short, action)] // default_value = "true"
    pub sync: bool,

    #[clap(long, short, action)] // default_value = "true"
    pub apply: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Clone { url: String },
}
