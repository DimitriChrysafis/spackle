//! `write_file`: create or overwrite a workspace file. Writes go to a
//! temporary sibling then rename into place, so a crash mid-write never
//! leaves a half-written file.

use async_trait::async_trait;
use serde_json::json;
use spackle_core::agent::{ToolDefinition, ToolError, ToolExecutor, ToolOutput};
use spackle_core::cancel::CancellationToken;
use spackle_core::message::ToolCall;

use crate::classify::looks_secret;
use crate::{ToolArgError, WorkspaceRoot, required_str};

/// Refuse to write files larger than this; a coding agent should never
/// legitimately emit megabytes in one call.
const MAX_WRITE_BYTES: usize = 1024 * 1024;

pub struct WriteFile {
    root: WorkspaceRoot,
    /// When set, overwriting an existing file first requires a `read_file`
    /// call on it this session.
    reads: Option<crate::ReadTracker>,
}

impl WriteFile {
    #[must_use]
    pub fn new(root: WorkspaceRoot, reads: Option<crate::ReadTracker>) -> Self {
        Self { root, reads }
    }

    #[must_use]
    pub fn schema() -> ToolDefinition {
        ToolDefinition {
            name: "write_file".to_owned(),
            description: "Create or overwrite a workspace file with the given \
                          content. Prefer edit_file for changes to existing \
                          files. Creates missing parent directories inside the \
                          workspace."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Workspace-relative path"},
                    "content": {"type": "string", "description": "Full file content"},
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
        }
    }

    /// Atomic write shared with `edit_file`.
    pub(crate) fn write_atomic(target: &std::path::Path, content: &[u8]) -> std::io::Result<()> {
        let parent = target.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing parent directory")
        })?;
        std::fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        use std::io::Write;
        temp.write_all(content)?;
        temp.as_file().sync_all()?;
        temp.persist(target).map_err(|err| err.error)?;
        Ok(())
    }
}

#[async_trait]
impl ToolExecutor for WriteFile {
    fn name(&self) -> &str {
        "write_file"
    }

    fn read_only(&self) -> bool {
        false
    }

    fn requires_approval(&self, _call: &ToolCall) -> bool {
        true
    }

    fn describe(&self, call: &ToolCall) -> String {
        let path = crate::arg_str(&call.arguments, "path").unwrap_or("?");
        let bytes = crate::arg_str(&call.arguments, "content")
            .map(str::len)
            .unwrap_or(0);
        if looks_secret(std::path::Path::new(path)) {
            format!("secret:write {path} ({bytes} bytes)")
        } else {
            format!("write:{path} ({bytes} bytes)")
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
        let content = crate::arg_str(&call.arguments, "content")
            .ok_or(ToolArgError::Missing("content".to_owned()))
            .map_err(ToolArgError::into_failed)?;
        if content.len() > MAX_WRITE_BYTES {
            return Err(ToolError::Failed(format!(
                "content is {} bytes; write_file is capped at {MAX_WRITE_BYTES}",
                content.len()
            )));
        }
        let resolved = match self.root.resolve_new(path) {
            Ok(path) => path,
            Err(error) => return Err(ToolError::Failed(error.to_string())),
        };
        if resolved.is_dir() {
            return Err(ToolError::Failed(format!("{path} is a directory")));
        }
        let existed = resolved.exists();
        if existed
            && let Some(reads) = &self.reads
            && !reads
                .lock()
                .map(|set| set.contains(&resolved))
                .unwrap_or(true)
        {
            return Err(ToolError::Failed(format!(
                "refusing to overwrite {path} before reading it; call read_file \
                 on it first, then write the full new content"
            )));
        }
        Self::write_atomic(&resolved, content.as_bytes())
            .map_err(|err| ToolError::Failed(format!("cannot write {path}: {err}")))?;
        Ok(ToolOutput {
            text: format!(
                "{} {path} ({} bytes)\n",
                if existed { "updated" } else { "created" },
                content.len()
            ),
            is_error: false,
        })
    }
}
