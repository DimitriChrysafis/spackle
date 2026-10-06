//! `run_command`: run a shell command with the workspace as the working
//! directory. Output is capped, the process is bounded by a timeout, and
//! cancellation kills the child.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;
use spackle_core::agent::{ToolDefinition, ToolError, ToolExecutor, ToolOutput};
use spackle_core::cancel::CancellationToken;
use spackle_core::message::ToolCall;

use crate::classify::{CommandClass, classify_command};
use crate::{ToolArgError, WorkspaceRoot, arg_u64, required_str, truncate};

const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_TIMEOUT_SECS: u64 = 900;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

pub struct RunCommand {
    root: WorkspaceRoot,
}

impl RunCommand {
    #[must_use]
    pub fn new(root: WorkspaceRoot) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn schema() -> ToolDefinition {
        ToolDefinition {
            name: "run_command".to_owned(),
            description: "Run a shell command with the workspace as the working \
                          directory (via `sh -c`). Returns exit code, stdout, and \
                          stderr; output is bounded. Read-only commands run \
                          freely; mutating, destructive, or networked commands \
                          need approval."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Shell command line"},
                    "timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 900,
                                        "description": "Kill the command after this long (default 120)"},
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        }
    }
}

fn class_tag(class: CommandClass) -> &'static str {
    match class {
        CommandClass::SafeRead => "read",
        CommandClass::Write => "command",
        CommandClass::Destructive => "command-destructive",
        CommandClass::Network => "command-network",
    }
}

#[async_trait]
impl ToolExecutor for RunCommand {
    fn name(&self) -> &str {
        "run_command"
    }

    fn read_only(&self) -> bool {
        false
    }

    fn requires_approval(&self, call: &ToolCall) -> bool {
        let command = crate::arg_str(&call.arguments, "command").unwrap_or("");
        classify_command(command) != CommandClass::SafeRead
    }

    fn describe(&self, call: &ToolCall) -> String {
        let command = crate::arg_str(&call.arguments, "command").unwrap_or("?");
        let class = classify_command(command);
        format!("{}:{command}", class_tag(class))
    }

    async fn execute(
        &self,
        call: &ToolCall,
        cancel: CancellationToken,
    ) -> Result<ToolOutput, ToolError> {
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        let command =
            required_str(&call.arguments, "command").map_err(ToolArgError::into_failed)?;
        let timeout = Duration::from_secs(
            arg_u64(&call.arguments, "timeout_seconds")
                .unwrap_or(DEFAULT_TIMEOUT_SECS)
                .clamp(1, MAX_TIMEOUT_SECS),
        );

        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(self.root.as_path())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| ToolError::Failed(format!("cannot spawn `sh -c`: {err}")))?;
        let mut stdout = child.stdout.take().expect("stdout piped");
        let mut stderr = child.stderr.take().expect("stderr piped");
        let stdout_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let _ = tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut buf).await;
            buf
        });
        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut buf).await;
            buf
        });

        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
        let finished = Arc::new(AtomicBool::new(false));
        {
            let token = cancel.clone();
            let finished = finished.clone();
            tokio::task::spawn_blocking(move || {
                while !finished.load(Ordering::Acquire) {
                    if token.wait_timeout(Duration::from_millis(25)) {
                        let _ = cancel_tx.send(());
                        return;
                    }
                }
            });
        }
        struct MarkFinished(Arc<AtomicBool>);
        impl Drop for MarkFinished {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let _finished_guard = MarkFinished(finished);

        let status = tokio::select! {
            result = child.wait() => {
                result.map_err(|err| ToolError::Failed(format!("wait failed: {err}")))?
            }
            () = tokio::time::sleep(timeout) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Ok(ToolOutput {
                    text: format!(
                        "[killed: exceeded {timeout:?} timeout]\ncommand: {command}"
                    ),
                    is_error: true,
                });
            }
            _ = &mut cancel_rx => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(ToolError::Cancelled);
            }
        };
        let stdout_bytes = stdout_task.await.unwrap_or_default();
        let stderr_bytes = stderr_task.await.unwrap_or_default();

        let mut text = String::new();
        let code = status.code().unwrap_or(-1);
        text.push_str(&format!("exit={code}\n"));
        if !stdout_bytes.is_empty() {
            text.push_str("--- stdout ---\n");
            text.push_str(&String::from_utf8_lossy(&stdout_bytes));
        }
        if !stderr_bytes.is_empty() {
            text.push_str("--- stderr ---\n");
            text.push_str(&String::from_utf8_lossy(&stderr_bytes));
        }
        truncate(&mut text, MAX_OUTPUT_BYTES);
        Ok(ToolOutput {
            text,
            is_error: !status.success(),
        })
    }
}
