//! `edit_file`: exact-match text replacement. `old_text` must occur exactly
//! once so edits are unambiguous; the result includes a unified diff so the
//! model sees what actually changed.

use async_trait::async_trait;
use serde_json::json;
use spackle_core::agent::{ToolDefinition, ToolError, ToolExecutor, ToolOutput};
use spackle_core::cancel::CancellationToken;
use spackle_core::message::ToolCall;

use crate::classify::looks_secret;
use crate::write::WriteFile;
use crate::{ToolArgError, WorkspaceRoot, required_str, truncate};

/// Cap on the returned diff text.
const MAX_DIFF_BYTES: usize = 16 * 1024;
/// Largest file an exact-match edit will process (files are loaded fully).
const MAX_EDIT_FILE_BYTES: u64 = 4 * 1024 * 1024;

pub struct EditFile {
    root: WorkspaceRoot,
    /// When set, editing a file first requires a `read_file` call on it
    /// this session.
    reads: Option<crate::ReadTracker>,
}

impl EditFile {
    #[must_use]
    pub fn new(root: WorkspaceRoot, reads: Option<crate::ReadTracker>) -> Self {
        Self { root, reads }
    }

    #[must_use]
    pub fn schema() -> ToolDefinition {
        ToolDefinition {
            name: "edit_file".to_owned(),
            description: "Replace `old_text` with `new_text` in a workspace file. \
                          `old_text` must match the file content exactly once \
                          (include enough context lines to be unique). Returns a \
                          unified diff of the change."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Workspace-relative path"},
                    "old_text": {"type": "string",
                                 "description": "Exact text to replace; must occur exactly once"},
                    "new_text": {"type": "string", "description": "Replacement text"},
                    "replace_all": {"type": "boolean",
                                    "description": "Replace every occurrence instead of requiring uniqueness"},
                },
                "required": ["path", "old_text", "new_text"],
                "additionalProperties": false
            }),
        }
    }
}

#[async_trait]
impl ToolExecutor for EditFile {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn read_only(&self) -> bool {
        false
    }

    fn requires_approval(&self, _call: &ToolCall) -> bool {
        true
    }

    fn describe(&self, call: &ToolCall) -> String {
        let path = crate::arg_str(&call.arguments, "path").unwrap_or("?");
        if looks_secret(std::path::Path::new(path)) {
            format!("secret:edit {path}")
        } else {
            format!("write:edit {path}")
        }
    }

    async fn execute(
        &self,
        call: &ToolCall,
        cancel: CancellationToken,
    ) -> Result<ToolOutput, ToolError> {
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        let path = required_str(&call.arguments, "path").map_err(ToolArgError::into_failed)?;
        let old_text = crate::arg_str(&call.arguments, "old_text")
            .ok_or(ToolArgError::Missing("old_text".to_owned()))
            .map_err(ToolArgError::into_failed)?;
        let new_text = crate::arg_str(&call.arguments, "new_text")
            .ok_or(ToolArgError::Missing("new_text".to_owned()))
            .map_err(ToolArgError::into_failed)?;
        let replace_all = crate::arg_bool(&call.arguments, "replace_all").unwrap_or(false);
        if old_text.is_empty() {
            return Err(ToolError::Failed(
                "old_text must not be empty; to create a file use write_file".to_owned(),
            ));
        }
        if old_text == new_text {
            return Err(ToolError::Failed(
                "old_text and new_text are identical; nothing to change".to_owned(),
            ));
        }
        let resolved = match self.root.resolve_existing(path) {
            Ok(path) => path,
            Err(error) => return Err(ToolError::Failed(error.to_string())),
        };
        if resolved.is_dir() {
            return Err(ToolError::Failed(format!("{path} is a directory")));
        }
        if let Some(reads) = &self.reads
            && !reads
                .lock()
                .map(|set| set.contains(&resolved))
                .unwrap_or(true)
        {
            return Err(ToolError::Failed(format!(
                "refusing to edit {path} before reading it; call read_file on it \
                 first so old_text matches the real content"
            )));
        }
        let size = std::fs::metadata(&resolved)
            .map(|meta| meta.len())
            .unwrap_or(0);
        if size > MAX_EDIT_FILE_BYTES {
            return Err(ToolError::Failed(format!(
                "{path} is {size} bytes; edit_file is capped at {MAX_EDIT_FILE_BYTES}"
            )));
        }
        let before = std::fs::read(&resolved)
            .map_err(|err| ToolError::Failed(format!("cannot read {path}: {err}")))?;
        if before.contains(&0) {
            return Err(ToolError::Failed(format!(
                "{path} looks binary (contains NUL); not editing"
            )));
        }
        let before = String::from_utf8_lossy(&before).into_owned();
        let occurrences = before.matches(old_text).count();
        if occurrences == 0 {
            return Err(ToolError::Failed(format!(
                "old_text not found in {path}; re-read the file and match its \
                 exact content (including indentation and whitespace)"
            )));
        }
        if occurrences > 1 && !replace_all {
            return Err(ToolError::Failed(format!(
                "old_text matches {occurrences} places in {path}; add more \
                 context to make it unique, or set replace_all"
            )));
        }
        let after = before.replace(old_text, new_text);
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        WriteFile::write_atomic(&resolved, after.as_bytes())
            .map_err(|err| ToolError::Failed(format!("cannot write {path}: {err}")))?;

        let patch = diffy::create_patch(&before, &after).to_string();
        let mut out = format!(
            "edited {path} ({occurrences} replacement{})\n",
            if occurrences == 1 { "" } else { "s" }
        );
        let mut patch = patch;
        truncate(&mut patch, MAX_DIFF_BYTES);
        out.push_str(&patch);
        Ok(ToolOutput {
            text: out,
            is_error: false,
        })
    }
}
