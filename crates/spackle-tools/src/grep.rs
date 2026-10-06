//! `grep`: bounded regex search over workspace text files. Secret-looking
//! files are skipped so reads cannot leak credentials into the transcript.

use std::io::BufRead;

use async_trait::async_trait;
use serde_json::json;
use spackle_core::agent::{ToolDefinition, ToolError, ToolExecutor, ToolOutput};
use spackle_core::cancel::CancellationToken;
use spackle_core::message::ToolCall;

use crate::classify::looks_secret;
use crate::{ToolArgError, WorkspaceRoot, arg_bool, arg_str, arg_u64, required_str, truncate};

const DEFAULT_MAX_MATCHES: usize = 200;
const HARD_MAX_MATCHES: usize = 2000;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

pub struct Grep {
    root: WorkspaceRoot,
}

impl Grep {
    #[must_use]
    pub fn new(root: WorkspaceRoot) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn schema() -> ToolDefinition {
        ToolDefinition {
            name: "grep".to_owned(),
            description: "Search workspace files for a regex pattern. Output is \
                          `path:line:match`, bounded. Skips gitignored, binary, \
                          and secret-looking files. Use glob to narrow scope."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Regex pattern (Rust syntax)"},
                    "path": {"type": "string",
                             "description": "Workspace-relative file or directory to search (default: root)"},
                    "glob": {"type": "string",
                             "description": "Restrict matches to paths matching this glob, e.g. `*.rs`"},
                    "ignore_case": {"type": "boolean", "description": "Case-insensitive matching"},
                    "max_matches": {"type": "integer", "minimum": 1, "maximum": 2000,
                                    "description": "Cap on reported matches"},
                },
                "required": ["pattern"],
                "additionalProperties": false
            }),
        }
    }
}

#[async_trait]
impl ToolExecutor for Grep {
    fn name(&self) -> &str {
        "grep"
    }

    fn read_only(&self) -> bool {
        true
    }

    fn describe(&self, call: &ToolCall) -> String {
        let pattern = arg_str(&call.arguments, "pattern").unwrap_or("?");
        format!("grep:{pattern}")
    }

    async fn execute(
        &self,
        call: &ToolCall,
        cancel: CancellationToken,
    ) -> Result<ToolOutput, ToolError> {
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        let pattern =
            required_str(&call.arguments, "pattern").map_err(ToolArgError::into_failed)?;
        let ignore_case = arg_bool(&call.arguments, "ignore_case").unwrap_or(false);
        let regex = regex::RegexBuilder::new(pattern)
            .case_insensitive(ignore_case)
            .build()
            .map_err(|err| ToolError::Failed(format!("invalid regex `{pattern}`: {err}")))?;

        let base = arg_str(&call.arguments, "path").unwrap_or(".");
        let start = match self.root.resolve_existing(base) {
            Ok(path) => path,
            Err(error) => return Err(ToolError::Failed(error.to_string())),
        };
        let matcher = match arg_str(&call.arguments, "glob") {
            Some(pattern) => Some(
                globset::GlobBuilder::new(pattern)
                    .literal_separator(false)
                    .build()
                    .map_err(|err| ToolError::Failed(format!("invalid glob `{pattern}`: {err}")))?
                    .compile_matcher(),
            ),
            None => None,
        };
        let max = arg_u64(&call.arguments, "max_matches")
            .map(|v| (v as usize).min(HARD_MAX_MATCHES))
            .unwrap_or(DEFAULT_MAX_MATCHES);

        let root = self.root.clone();
        let hits = tokio::task::spawn_blocking(move || {
            search(&root, &start, &regex, matcher.as_ref(), max)
        })
        .await
        .map_err(|err| ToolError::Failed(format!("search failed: {err}")))?;

        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        let mut out = String::new();
        for line in &hits.lines {
            out.push_str(line);
            out.push('\n');
        }
        if hits.skipped_secrets > 0 {
            out.push_str(&format!(
                "[skipped {} secret-looking file{}]\n",
                hits.skipped_secrets,
                if hits.skipped_secrets == 1 { "" } else { "s" }
            ));
        }
        if hits.truncated {
            out.push_str(&format!(
                "[stopped after {max} matches; narrow with glob or path]\n"
            ));
        }
        if out.is_empty() {
            out.push_str("[no matches]\n");
        }
        truncate(&mut out, MAX_OUTPUT_BYTES);
        Ok(ToolOutput {
            text: out,
            is_error: false,
        })
    }
}

struct SearchHits {
    lines: Vec<String>,
    skipped_secrets: usize,
    truncated: bool,
}

fn search(
    root: &WorkspaceRoot,
    start: &std::path::Path,
    regex: &regex::Regex,
    matcher: Option<&globset::GlobMatcher>,
    max: usize,
) -> SearchHits {
    let mut hits = SearchHits {
        lines: Vec::new(),
        skipped_secrets: 0,
        truncated: false,
    };
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    if start.is_file() {
        files.push(start.to_path_buf());
    } else {
        for result in ignore::WalkBuilder::new(start)
            .hidden(false)
            .git_ignore(true)
            .git_exclude(true)
            .require_git(false)
            .follow_links(false)
            .build()
        {
            if let Ok(entry) = result
                && entry.file_type().is_some_and(|t| t.is_file())
            {
                files.push(entry.path().to_path_buf());
            }
        }
        files.sort();
    }
    for path in files {
        if hits.lines.len() >= max {
            hits.truncated = true;
            break;
        }
        let display = root.display(&path);
        if let Some(matcher) = matcher
            && !matcher.is_match(display.as_str())
        {
            continue;
        }
        let relative = std::path::Path::new(&display);
        if looks_secret(relative) {
            hits.skipped_secrets += 1;
            continue;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let reader = std::io::BufReader::new(file);
        for (index, line) in reader.lines().enumerate() {
            if hits.lines.len() >= max {
                hits.truncated = true;
                break;
            }
            let Ok(line) = line else { break }; // decode error/binary-ish
            if regex.is_match(&line) {
                hits.lines.push(format!("{display}:{}:{line}", index + 1));
            }
        }
    }
    hits
}
