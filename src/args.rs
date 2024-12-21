use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    /// Replace the existing symlinks with symlinks to .dotfiles
    #[clap(long, short, action)]
    pub preview: bool,

    /// Synchronize the .dotfiles with a git repository.This performs Commit -> Pull -> Rebase -> Push
    #[clap(long, short, action)] // default_value = "true"
    pub sync: bool,

    /// Apply the packages onto the system
    #[clap(long, short, action)] // default_value = "true"
    pub apply: bool,

    /// Override git remote setting for this operation
    #[clap(long)]
    pub remote: Option<String>,

    /// Override git branch setting for this operation
    #[clap(long)]
    pub branch: Option<String>,

    /// Set the merge flavor to a merge commit. This also selectively enables the Pull action. E.g. when
    /// the Sync flag is not set
    #[clap(long, action)]
    pub merge_commit: bool,

    /// Set the merge flavor to rebase
    #[clap(long, action)]
    pub rebase: bool,

    /// Set the commit message
    #[clap(long, short)]
    pub message: Option<String>,

    /// Pull .dotfiles from the repository. This selectively enables the Pull action. E.g. when
    /// the Sync flag is not set
    #[clap(long, action)]
    pub pull: bool,

    /// Push .dotfiles to the repository. This selectively enables the Push action. E.g. when
    /// the Sync flag is not set
    #[clap(long, action)]
    pub push: bool,

    /// Show git status
    #[clap(long, action)]
    pub status: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

// TODO: add optional path override to Clone command
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Clone a git repository into ~/.dotfiles
    Clone { url: String },

    /// Init a git repository in ~/.dotfiles, you can also provide the origin url
    Init { url: Option<String> },
}
