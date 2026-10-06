//! Sandboxed workspace tools for the agent loop.
//!
//! Every tool resolves paths through [`WorkspaceRoot`], which rejects
//! absolute paths, `..` traversal, and symlinks that escape the canonical
//! root. Read tools are side-effect free and may run concurrently; write
//! and command tools serialize through the loop and consult the approval
//! gate first (the `describe` text carries a `kind:` prefix the gate maps
//! to the configured confirmation policy).

#![forbid(unsafe_code)]

mod classify;
mod command;
mod edit;
mod grep;
mod list;
mod read;
mod write;

use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use spackle_core::agent::{ToolDefinition, ToolRegistry};
use thiserror::Error;

pub use classify::{CommandClass, classify_command, looks_secret};
pub use command::RunCommand;
pub use edit::EditFile;
pub use grep::Grep;
pub use list::ListFiles;
pub use read::ReadFile;
pub use write::WriteFile;

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

/// Canonical sandbox root. All tool paths resolve inside it.
#[derive(Debug, Clone)]
pub struct WorkspaceRoot {
    canonical: PathBuf,
}

impl WorkspaceRoot {
    /// Canonicalize `path`; fails when it does not exist.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        Ok(Self {
            canonical: path.as_ref().canonicalize()?,
        })
    }

    /// The canonical root path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.canonical
    }

    /// Resolve a workspace-relative path that must already exist. Rejects
    /// absolute paths, `..`, and symlinks escaping the root.
    pub fn resolve_existing(&self, relative: impl AsRef<Path>) -> Result<PathBuf, WorkspaceError> {
        let relative = checked_relative(relative.as_ref())?;
        let resolved = self.canonical.join(relative).canonicalize()?;
        if !resolved.starts_with(&self.canonical) {
            return Err(WorkspaceError::EscapesWorkspace);
        }
        Ok(resolved)
    }

    /// Resolve a workspace-relative path that may not exist yet (writes).
    /// The deepest existing ancestor is canonicalized, so a symlinked
    /// directory still cannot smuggle the target outside the root.
    pub fn resolve_new(&self, relative: impl AsRef<Path>) -> Result<PathBuf, WorkspaceError> {
        let relative = checked_relative(relative.as_ref())?;
        let candidate = self.canonical.join(relative);
        if candidate.exists() {
            return self.resolve_existing(relative);
        }
        let mut existing = candidate.clone();
        let mut tail: Vec<std::ffi::OsString> = Vec::new();
        while !existing.exists() {
            match existing.file_name() {
                Some(name) => {
                    tail.push(name.to_owned());
                    existing = existing
                        .parent()
                        .map(Path::to_path_buf)
                        .ok_or(WorkspaceError::EscapesWorkspace)?;
                }
                None => return Err(WorkspaceError::EscapesWorkspace),
            }
        }
        let anchor = existing.canonicalize()?;
        if !anchor.starts_with(&self.canonical) {
            return Err(WorkspaceError::EscapesWorkspace);
        }
        let mut resolved = anchor;
        for part in tail.iter().rev() {
            resolved.push(part);
        }
        Ok(resolved)
    }

    /// Path relative to the root for display in output and prompts.
    #[must_use]
    pub fn display(&self, path: &Path) -> String {
        path.strip_prefix(&self.canonical)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    }
}

fn checked_relative(relative: &Path) -> Result<&Path, WorkspaceError> {
    if relative.is_absolute() {
        return Err(WorkspaceError::AbsolutePath);
    }
    if relative
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(WorkspaceError::ParentTraversal);
    }
    Ok(relative)
}

/// Canonical paths read through `read_file` this session. `edit_file` and
/// `write_file` consult it (when enabled) to refuse blind edits: modifying
/// a file the model has never read is a reliable source of broken patches.
pub type ReadTracker = std::sync::Arc<std::sync::Mutex<std::collections::HashSet<PathBuf>>>;

/// Options for [`standard_registry_with`].
#[derive(Debug, Clone, Copy)]
pub struct RegistryOptions {
    /// Refuse edits/writes to files that were not read this turn.
    pub read_guard: bool,
}

impl Default for RegistryOptions {
    fn default() -> Self {
        Self { read_guard: true }
    }
}

/// Register every standard tool for `root` with the read guard on.
pub fn standard_registry(root: WorkspaceRoot) -> ToolRegistry {
    standard_registry_with(root, RegistryOptions::default())
}

/// Register every standard tool for `root` with explicit options.
pub fn standard_registry_with(root: WorkspaceRoot, options: RegistryOptions) -> ToolRegistry {
    let reads: ReadTracker = Default::default();
    let guard = options.read_guard.then(|| reads.clone());
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ReadFile::new(root.clone(), reads)));
    registry.register(Box::new(ListFiles::new(root.clone())));
    registry.register(Box::new(Grep::new(root.clone())));
    registry.register(Box::new(WriteFile::new(root.clone(), guard.clone())));
    registry.register(Box::new(EditFile::new(root.clone(), guard)));
    registry.register(Box::new(RunCommand::new(root)));
    registry
}

/// Schemas for every standard tool, sorted by name (stable ordering).
#[must_use]
pub fn standard_schemas() -> Vec<ToolDefinition> {
    let mut out = vec![
        ReadFile::schema(),
        ListFiles::schema(),
        Grep::schema(),
        WriteFile::schema(),
        EditFile::schema(),
        RunCommand::schema(),
    ];
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// ---------------------------------------------------------------------------
// Shared argument helpers
// ---------------------------------------------------------------------------

fn arg_str<'a>(arguments: &'a Value, key: &str) -> Option<&'a str> {
    arguments.get(key).and_then(Value::as_str)
}

fn arg_u64(arguments: &Value, key: &str) -> Option<u64> {
    arguments.get(key).and_then(Value::as_u64)
}

fn arg_bool(arguments: &Value, key: &str) -> Option<bool> {
    arguments.get(key).and_then(Value::as_bool)
}

fn required_str_opt<'a>(arguments: &'a Value, key: &str) -> Option<&'a str> {
    arg_str(arguments, key).filter(|value| !value.is_empty())
}

fn required_str<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, ToolArgError> {
    arg_str(arguments, key)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ToolArgError::Missing(key.to_owned()))
}

#[derive(Debug)]
pub enum ToolArgError {
    Missing(String),
    Invalid(String),
}

impl ToolArgError {
    fn into_failed(self) -> spackle_core::agent::ToolError {
        spackle_core::agent::ToolError::Failed(self.to_string())
    }
}

impl std::fmt::Display for ToolArgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(key) => write!(f, "missing required argument `{key}`"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

/// Bound a byte string, labelling the cut so the model sees the truncation.
fn truncate(text: &mut String, max_bytes: usize) {
    if text.len() <= max_bytes {
        return;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str("\n…[truncated by spackle]");
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

    #[test]
    fn rejects_absolute_paths() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = WorkspaceRoot::new(temp.path()).expect("workspace root");
        assert!(matches!(
            root.resolve_existing("/etc/hosts"),
            Err(WorkspaceError::AbsolutePath)
        ));
        assert!(matches!(
            root.resolve_new("/etc/hosts"),
            Err(WorkspaceError::AbsolutePath)
        ));
    }

    #[test]
    fn symlink_escape_is_rejected() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let outside = tempfile::tempdir().expect("outside directory");
        std::fs::write(outside.path().join("secret.txt"), "hidden").unwrap();
        let link = temp.path().join("link");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        let root = WorkspaceRoot::new(temp.path()).expect("workspace root");
        assert!(matches!(
            root.resolve_existing("link/secret.txt"),
            Err(WorkspaceError::EscapesWorkspace)
        ));
        // Writing through the link must fail too.
        assert!(matches!(
            root.resolve_new("link/new.txt"),
            Err(WorkspaceError::EscapesWorkspace)
        ));
    }

    #[test]
    fn new_file_inside_root_resolves() {
        let temp = tempfile::tempdir().expect("temporary directory");
        std::fs::create_dir(temp.path().join("sub")).unwrap();
        let root = WorkspaceRoot::new(temp.path()).expect("workspace root");
        let resolved = root.resolve_new("sub/deep/file.rs").expect("resolves");
        assert!(resolved.starts_with(root.as_path()));
        assert!(resolved.ends_with("sub/deep/file.rs"));
    }

    #[test]
    fn schemas_are_objects_with_names() {
        let schemas = standard_schemas();
        assert_eq!(schemas.len(), 6);
        for schema in &schemas {
            assert!(!schema.name.is_empty());
            assert_eq!(schema.parameters["type"], "object", "{}", schema.name);
            assert!(!schema.description.is_empty());
        }
        let names: Vec<&str> = schemas.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"run_command"));
    }
}
