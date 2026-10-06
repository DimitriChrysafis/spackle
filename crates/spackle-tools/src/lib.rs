#![forbid(unsafe_code)]

use std::path::{Component, Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("workspace path must be relative")]
    AbsolutePath,
    #[error("workspace path may not contain parent traversal")]
    ParentTraversal,
    #[error("path escapes the canonical workspace root")]
    EscapesWorkspace,
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct WorkspaceRoot {
    canonical: PathBuf,
}

impl WorkspaceRoot {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        Ok(Self {
            canonical: path.as_ref().canonicalize()?,
        })
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.canonical
    }

    pub fn resolve_existing(&self, relative: impl AsRef<Path>) -> Result<PathBuf, WorkspaceError> {
        let relative = relative.as_ref();
        if relative.is_absolute() {
            return Err(WorkspaceError::AbsolutePath);
        }
        if relative
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(WorkspaceError::ParentTraversal);
        }
        let resolved = self.canonical.join(relative).canonicalize()?;
        if !resolved.starts_with(&self.canonical) {
            return Err(WorkspaceError::EscapesWorkspace);
        }
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_traversal() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = WorkspaceRoot::new(temp.path()).expect("workspace root");
        assert!(matches!(
            root.resolve_existing("../outside"),
            Err(WorkspaceError::ParentTraversal)
        ));
    }
}
