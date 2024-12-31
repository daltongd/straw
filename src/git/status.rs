use crate::result::Result;
use std::fmt::Display;
use std::path::PathBuf;

pub struct GitStatus {
    file: String,
    status: git2::Status,
}

impl GitStatus {
    pub fn unchanged(&self) -> bool {
        self.status == git2::Status::CURRENT
    }

    pub fn ignored(&self) -> bool {
        self.status == git2::Status::IGNORED
    }
}

impl Display for GitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let file_status = match self.status {
            git2::Status::INDEX_NEW => "new",
            git2::Status::INDEX_MODIFIED => "modified",
            git2::Status::INDEX_DELETED => "deleted",
            git2::Status::INDEX_RENAMED => "renamed",
            git2::Status::INDEX_TYPECHANGE => "type change",
            git2::Status::WT_NEW => "new",
            git2::Status::WT_MODIFIED => "modified",
            git2::Status::WT_DELETED => "deleted",
            git2::Status::WT_RENAMED => "renamed",
            git2::Status::WT_TYPECHANGE => "type change",
            git2::Status::CONFLICTED => "conflict",
            git2::Status::IGNORED => "ignored",
            git2::Status::CURRENT => "unchanged",
            _ => "unknown",
        };
        write!(f, "{} ({})", self.file, file_status)
    }
}

pub fn get_status(path: &PathBuf) -> Result<Vec<GitStatus>> {
    let repository = git2::Repository::open(path)?;
    let mut status_options = git2::StatusOptions::default();
    status_options
        .include_untracked(true)
        .include_unmodified(false);

    let mut out_status = Vec::default();
    for statuses in repository.statuses(Some(&mut status_options)).iter() {
        for status in statuses.iter() {
            out_status.push(GitStatus {
                file: status.path().unwrap_or_default().into(),
                status: status.status(),
            });
        }
    }

    Ok(out_status)
}
