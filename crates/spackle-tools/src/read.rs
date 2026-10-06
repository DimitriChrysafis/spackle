//! `read_file`: bounded, line-numbered file reads inside the workspace.

use async_trait::async_trait;
use serde_json::json;
use spackle_core::agent::{ToolDefinition, ToolError, ToolExecutor, ToolOutput};
use spackle_core::cancel::CancellationToken;
use spackle_core::message::ToolCall;

use crate::classify::looks_secret;
use crate::{ToolArgError, WorkspaceRoot, arg_u64, required_str, truncate};

/// Bytes returned to the model before any slicing; larger files are still
/// opened so the model gets a labelled truncation rather than an error.
const MAX_READ_BYTES: usize = 64 * 1024;
/// Hard cap on bytes read from disk.
const MAX_DISK_BYTES: u64 = 4 * 1024 * 1024;

pub struct ReadFile {
    root: WorkspaceRoot,
    /// Records every successfully read path for the edit/write guard.
    reads: crate::ReadTracker,
}

impl ReadFile {
    #[must_use]
    pub fn new(root: WorkspaceRoot, reads: crate::ReadTracker) -> Self {
        Self { root, reads }
    }

    #[must_use]
    pub fn schema() -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_owned(),
            description: "Read a file from the workspace. Output is line-numbered \
                          (`line|text`) and bounded; use offset/limit to page \
                          through large files. Paths are workspace-relative."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Workspace-relative path"},
                    "offset": {"type": "integer", "minimum": 1,
                               "description": "1-based line to start at (default 1)"},
                    "limit": {"type": "integer", "minimum": 1,
                              "description": "Maximum number of lines to return"},
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        }
    }
}

#[async_trait]
impl ToolExecutor for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn read_only(&self) -> bool {
        true
    }

    fn requires_approval(&self, call: &ToolCall) -> bool {
        let path = crate::arg_str(&call.arguments, "path").unwrap_or("");
        looks_secret(std::path::Path::new(path))
    }

    fn describe(&self, call: &ToolCall) -> String {
        let path = crate::arg_str(&call.arguments, "path").unwrap_or("?");
        if self.requires_approval(call) {
            format!("secret:read file {path}")
        } else {
            format!("read:{path}")
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
        let resolved = match self.root.resolve_existing(path) {
            Ok(path) => path,
            Err(error) => return Err(ToolError::Failed(error.to_string())),
        };
        if resolved.is_dir() {
            return Err(ToolError::Failed(format!(
                "{path} is a directory; use list_files instead"
            )));
        }
        use tokio::io::AsyncReadExt;
        let file_size = std::fs::metadata(&resolved).map(|meta| meta.len()).ok();
        let file = tokio::fs::File::open(&resolved)
            .await
            .map_err(|err| ToolError::Failed(format!("cannot read {path}: {err}")))?;
        let mut bytes = Vec::new();
        file.take(MAX_DISK_BYTES)
            .read_to_end(&mut bytes)
            .await
            .map_err(|err| ToolError::Failed(format!("cannot read {path}: {err}")))?;
        let file_truncated = file_size.is_some_and(|size| size > bytes.len() as u64);
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        if bytes.contains(&0) {
            return Err(ToolError::Failed(format!(
                "{path} looks binary (contains NUL); use run_command with a \
                 binary-aware tool if you really need it"
            )));
        }
        let text = String::from_utf8_lossy(&bytes);
        let mut lines: Vec<&str> = text.lines().collect();
        let total_lines = lines.len();
        let offset = arg_u64(&call.arguments, "offset").unwrap_or(1).max(1) as usize;
        let limit = arg_u64(&call.arguments, "limit")
            .map(|v| v as usize)
            .unwrap_or(usize::MAX);
        if offset > total_lines && total_lines > 0 {
            return Err(ToolError::Failed(format!(
                "offset {offset} is past the end of {path} ({total_lines} lines)"
            )));
        }
        if offset > 1 {
            lines = lines.split_off(offset - 1);
        }
        lines.truncate(limit);

        let mut out = String::new();
        for (index, line) in lines.iter().enumerate() {
            out.push_str(&format!("{}|{line}\n", offset + index));
        }
        if out.len() > MAX_READ_BYTES {
            truncate(&mut out, MAX_READ_BYTES);
        }
        if file_truncated {
            out.push_str(&format!(
                "\n[file is {} bytes; showing the start only — page with offset/limit or grep]",
                file_size.unwrap_or_default()
            ));
        } else if out.len() >= MAX_READ_BYTES {
            out.push_str("\n[output truncated; page with offset/limit]");
        }
        if out.is_empty() {
            out = format!("[{path} is empty]\n");
        }
        if let Ok(mut reads) = self.reads.lock() {
            reads.insert(resolved.clone());
        }
        Ok(ToolOutput {
            text: out,
            is_error: false,
        })
    }
}
