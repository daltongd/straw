use std::path::PathBuf;

use crate::{args::Args, config::Config};
use regex::Regex;

#[derive(Debug)]
pub struct StateInfo<'a> {
    pub target_dir: &'a str,
    pub dotfiles_path: PathBuf,
    pub args: &'a Args,
    pub env_var_regex: Regex,
    pub config: Config,
}
