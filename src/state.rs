use std::{collections::HashMap, path::PathBuf};

use crate::{args::Args, straw_config::Override};
use regex::Regex;

#[derive(Debug)]
pub struct StateInfo<'a> {
    pub home_dir: &'a str,
    pub dotfiles_path: PathBuf,
    pub args: &'a Args,
    pub env_var_regex: Regex,
    pub overrides: HashMap<String, Override>,
    pub ignores: Vec<String>,
}
