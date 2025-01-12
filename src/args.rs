use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug, Clone, Serialize, Deserialize, mlua::FromLua)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    /// Replace the existing symlinks with symlinks to .dotfiles
    #[clap(long, short, action)]
    pub preview: bool,

    /// Synchronize the .dotfiles with a git repository. This performs Commit -> Pull -> Rebase/Merge -> Push
    #[clap(long, short, action)] // default_value = "true"
    pub sync_repo: bool,

    /// Apply the packages onto the system
    #[clap(long, short, action)] // default_value = "true"
    pub apply: bool,

    /// Clear all deployed symlinks
    #[clap(long, short, action)]
    pub clear: bool,

    /// Specify packages to apply. Implies `--apply`, overrides ignores from the config
    #[clap(long, num_args(1..))]
    pub packages: Vec<String>,

    /// Specify packages to ignore when applying the config
    #[clap(long, num_args(1..))]
    pub ignore: Vec<String>,

    /// Override git remote setting for this operation
    #[clap(long)]
    pub remote: Option<String>,

    /// Override git branch setting for this operation
    #[clap(long)]
    pub branch: Option<String>,

    /// Set the merge flavor to a merge commit
    #[clap(long, action)]
    pub merge_commit: bool,

    /// Set the merge flavor to rebase
    #[clap(long, action)]
    pub rebase: bool,

    /// Set the commit message. Implies `--commit`
    #[clap(long, short)]
    pub message: Option<String>,

    /// Pull .dotfiles from the repository. This selectively enables the `pull` action, even if
    /// the `--sync-repo` flag is not set
    #[clap(long, action)]
    pub pull: bool,

    /// Push .dotfiles to the repository. This selectively enables the `push` action, even if
    /// the `--sync-repo` flag is not set
    #[clap(long, action)]
    pub push: bool,

    /// Commit .dotfiles to the repository. This selectively enables the `commit` action, even if
    /// the `--sync-repo` flag is not set
    #[clap(long, action)]
    pub commit: bool,

    /// Adopt the package by moving the contents of a directory to the `.dotfiles` path and creating
    /// a symlink.
    /// NOTE: This functionality is limited to the paths under your target directory
    #[clap(long)]
    pub adopt: Option<String>,

    /// Override the default `~/.dotfiles` path
    #[clap(long)]
    pub dotfiles: Option<String>,

    /// Override the target path (default: `~`)
    #[clap(long)]
    pub target: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

// TODO: add optional path override to Clone command
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize, mlua::FromLua)]
pub enum Commands {
    /// Clone a git repository into ~/.dotfiles
    Clone {
        /// Repository url
        url: String,
    },

    /// Init a git repository in ~/.dotfiles, you can also provide the origin url
    Init {
        /// Remote url (optional)
        url: Option<String>,
    },

    /// Get git status
    Status,

    /// Equivalend of `--sync-repo --apply` or `-sa`
    Sync,

    /// Adopt the package by moving the contents of a directory to the `.dotfiles` path and creating
    /// a symlink.
    /// NOTE: This functionality is limited to the paths under your target directory
    Adopt {
        /// Path of the item to adopt
        path: String,
    },

    /// Run lua script
    Run {
        /// lua module, e.g. `test`
        module: String,
    },

    /// Execute lua script
    Execute {
        /// lua script
        script: String,
    },
}
