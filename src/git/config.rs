use serde::{Deserialize, Serialize};

// TODO: move remote and branch (and flavor?) to `GitDefaults`
// TODO: add optional overrides as a toml-based `.git-overrides`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepositoryConfig {
    pub remote: String,
    pub branch: String,
    pub merge_flavor: Option<MergeFlavor>,
    pub commit_message: Option<String>,
}

impl Default for RepositoryConfig {
    fn default() -> Self {
        Self {
            remote: "origin".into(),
            branch: "main".into(),
            merge_flavor: Some(Default::default()), // we want to default to Rebase
            commit_message: Default::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub enum MergeFlavor {
    #[default]
    Rebase,
    MergeCommit,
}
