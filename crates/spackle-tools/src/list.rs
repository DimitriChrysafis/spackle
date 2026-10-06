//! `list_files`: bounded directory listing inside the workspace, honoring
//! `.gitignore` via the `ignore` walker.

use async_trait::async_trait;
use serde_json::json;
use spackle_core::agent::{ToolDefinition, ToolError, ToolExecutor, ToolOutput};
use spackle_core::cancel::CancellationToken;
use spackle_core::message::ToolCall;

use crate::{WorkspaceRoot, arg_str, arg_u64, required_str_opt};

const DEFAULT_MAX_ENTRIES: usize = 400;
const HARD_MAX_ENTRIES: usize = 5000;

pub struct ListFiles {
    root: WorkspaceRoot,
}

impl ListFiles {
    #[must_use]
    pub fn new(root: WorkspaceRoot) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn schema() -> ToolDefinition {
        ToolDefinition {
            name: "list_files".to_owned(),
            description: "List workspace files (respects .gitignore). One relative \
                          path per line, directories end in `/`. Filter with a \
                          glob pattern; bounded output."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string",
                             "description": "Workspace-relative directory to list (default: root)"},
                    "pattern": {"type": "string",
                                "description": "Glob filter applied to relative paths, e.g. `**/*.rs`"},
                    "max_entries": {"type": "integer", "minimum": 1, "maximum": 5000,
                                    "description": "Cap on returned paths"},
                },
                "required": [],
                "additionalProperties": false
            }),
        }
    }
}

#[async_trait]
impl ToolExecutor for ListFiles {
    fn name(&self) -> &str {
        "list_files"
    }

    fn read_only(&self) -> bool {
        true
    }

    fn describe(&self, call: &ToolCall) -> String {
        format!(
            "list:{}",
            crate::arg_str(&call.arguments, "path").unwrap_or(".")
        )
    }

    async fn execute(
        &self,
        call: &ToolCall,
        cancel: CancellationToken,
    ) -> Result<ToolOutput, ToolError> {
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        let base = required_str_opt(&call.arguments, "path").unwrap_or(".");
        let dir = match self.root.resolve_existing(base) {
            Ok(path) => path,
            Err(error) => return Err(ToolError::Failed(error.to_string())),
        };
        if !dir.is_dir() {
            return Err(ToolError::Failed(format!(
                "{base} is not a directory; use read_file instead"
            )));
        }
        let matcher = match arg_str(&call.arguments, "pattern") {
            Some(pattern) => Some(
                globset::GlobBuilder::new(pattern)
                    .literal_separator(false)
                    .build()
                    .map_err(|err| ToolError::Failed(format!("invalid glob `{pattern}`: {err}")))?
                    .compile_matcher(),
            ),
            None => None,
        };
        let max = arg_u64(&call.arguments, "max_entries")
            .map(|v| (v as usize).min(HARD_MAX_ENTRIES))
            .unwrap_or(DEFAULT_MAX_ENTRIES);

        let root = self.root.clone();
        let dir_clone = dir.clone();
        let mut entries = tokio::task::spawn_blocking(move || {
            collect_entries(&root, &dir_clone, matcher.as_ref(), max)
        })
        .await
        .map_err(|err| ToolError::Failed(format!("walk failed: {err}")))?;
        entries.sort();

        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        let mut out = String::new();
        let truncated = entries.len() >= max;
        for entry in &entries {
            out.push_str(entry);
            out.push('\n');
        }
        if out.is_empty() {
            out.push_str("[no matching entries]\n");
        }
        if truncated {
            out.push_str(&format!(
                "[truncated at {max} entries; narrow with path or pattern]"
            ));
        }
        Ok(ToolOutput {
            text: out,
            is_error: false,
        })
    }
}

fn collect_entries(
    root: &WorkspaceRoot,
    dir: &std::path::Path,
    matcher: Option<&globset::GlobMatcher>,
    max: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    let walker = ignore::WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .require_git(false)
        .follow_links(false)
        .build();
    for result in walker {
        if out.len() >= max {
            break;
        }
        let Ok(entry) = result else { continue };
        let path = entry.path();
        if path == dir {
            continue;
        }
        let display = root.display(path);
        let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
        if let Some(matcher) = matcher
            && !matcher.is_match(display.as_str())
            && !is_dir
        {
            continue;
        }
        let mut line = display;
        if is_dir {
            line.push('/');
        }
        out.push(line);
    }
    out
}
