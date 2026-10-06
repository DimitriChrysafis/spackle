//! Append-only JSONL session journals, keyed by canonical workspace path.
//!
//! Layout (under the state directory):
//!   sessions/<ws-XXXXXXXXXXXXXXXX>.jsonl
//!   checkpoints/<ws-XXXXXXXXXXXXXXXX>.jsonl
//!
//! Every line is one self-describing record; the file is the source of
//! truth and is only ever appended to (or atomically replaced on
//! checkpoint), never mutated in place.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::PRODUCT_NAME;
use crate::message::Transcript;

/// Journal record kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecordKind {
    /// A message (user, assistant, or tool result).
    Message,
    /// A tool call (assistant-initiated).
    ToolCall,
    /// A tool result.
    ToolResult,
    /// A loop-guard decision (blocked or not).
    LoopDecision,
    /// A checkpoint marker (transcript snapshot taken).
    Checkpoint,
    /// Session metadata updates (title, model, profile).
    Meta,
}

/// One stored record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Monotonic-ish sequence number within the journal.
    pub seq: u64,
    pub timestamp: SystemTime,
    pub kind: SessionRecordKind,
    pub session_id: SessionId,
    /// Message role, when relevant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Textual content, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tool name, when relevant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Tool call id, when relevant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    /// Tool arguments (JSON), when relevant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<JsonValue>,
    /// Tool result (JSON), when relevant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<JsonValue>,
    /// Model that produced the record, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Extra context (bounded).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<JsonValue>,
}

/// A stable session identifier derived from the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

impl SessionId {
    /// Derive from a canonical workspace path.
    pub fn from_workspace(canonical_workspace: &Path) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(canonical_workspace.as_os_str().as_encoded_bytes());
        let digest = hasher.finalize();
        let hex: String = digest
            .iter()
            .take(8)
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Self(format!("ws-{hex}"))
    }

    /// The raw identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Journal file name for this session.
    #[must_use]
    pub fn journal_file_name(&self) -> String {
        format!("{}.jsonl", self.0)
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for SessionId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("ws-") && s.len() == 19 {
            Ok(Self(s.to_owned()))
        } else {
            Err(format!(
                "not a valid session id: `{s}` (expected ws-<16 hex>)"
            ))
        }
    }
}

/// Errors from session journal operations.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("invalid record {seq}: {reason}")]
    InvalidRecord { seq: u64, reason: String },
}

/// Append-only journal store for one session.
pub struct SessionStore {
    session_id: SessionId,
    path: PathBuf,
    next_seq: u64,
}

impl SessionStore {
    /// Open (creating if needed) the journal for `session_id` under `sessions_dir`.
    pub fn open(sessions_dir: &Path, session_id: SessionId) -> Result<Self, SessionError> {
        std::fs::create_dir_all(sessions_dir)?;
        let path = sessions_dir.join(session_id.journal_file_name());
        let next_seq = if path.exists() {
            // Resume: seq = 1 + max stored seq (scan is cheap; journals are bounded).
            let file = std::fs::File::open(&path)?;
            let reader = BufReader::new(file);
            let mut max: u64 = 0;
            for line in reader.lines().map_while(std::result::Result::ok) {
                if let Ok(record) = serde_json::from_str::<SessionRecord>(&line) {
                    max = max.max(record.seq);
                }
            }
            max + 1
        } else {
            0
        };
        Ok(Self {
            session_id,
            path,
            next_seq,
        })
    }

    /// The journal path (for tests and diagnostics).
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append a record; assigns and returns its sequence number.
    pub fn append(&mut self, mut record: SessionRecord) -> Result<u64, SessionError> {
        record.seq = self.next_seq;
        self.next_seq += 1;
        let line = serde_json::to_string(&record)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        Ok(record.seq)
    }

    /// Convenience: append a full transcript snapshot as a checkpoint record.
    pub fn checkpoint(&mut self, transcript: &Transcript) -> Result<u64, SessionError> {
        let mut record = SessionRecord::new(&self.session_id, SessionRecordKind::Checkpoint);
        record.extra = Some(serde_json::to_value(transcript).unwrap_or_default());
        self.append(record)
    }

    /// Read all valid records (skips a torn trailing line safely).
    pub fn read_all(&self) -> Result<Vec<SessionRecord>, SessionError> {
        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().collect::<std::result::Result<_, _>>()?;
        let mut out = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<SessionRecord>(line) {
                Ok(record) => out.push(record),
                Err(err) => {
                    // Tolerate one torn trailing line (crash mid-write); fail on
                    // anything else to keep the journal trustworthy.
                    if index + 1 == lines.len() {
                        eprintln!(
                            "spackle: warning: skipping torn journal line {index} in {}: {err}",
                            self.path.display()
                        );
                    } else {
                        return Err(SessionError::InvalidRecord {
                            seq: index as u64,
                            reason: err.to_string(),
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    /// Rebuild the in-memory transcript from message-shaped records.
    pub fn load_transcript(&self) -> Result<Transcript, SessionError> {
        let records = self.read_all()?;
        let mut transcript = Transcript::default();
        for record in &records {
            match record.kind {
                SessionRecordKind::Message => {
                    if let (Some(role), Some(content)) = (&record.role, &record.content) {
                        let message = match role.as_str() {
                            "system" => crate::message::Message::system(content.clone()),
                            "assistant" => crate::message::Message::assistant(
                                content.clone(),
                                None,
                                Vec::new(),
                            ),
                            "tool" => crate::message::Message::tool_result(
                                record.call_id.clone().unwrap_or_default(),
                                record.tool.clone().unwrap_or_default(),
                                content.clone(),
                                false,
                            ),
                            _ => crate::message::Message::user(content.clone()),
                        };
                        transcript.push(message);
                    }
                }
                SessionRecordKind::ToolCall => {
                    if let (Some(call_id), Some(arguments)) = (&record.call_id, &record.arguments) {
                        let name = record.tool.clone().unwrap_or_default();
                        transcript.push(crate::message::Message::assistant(
                            String::new(),
                            None,
                            vec![crate::message::ToolCall {
                                id: call_id.clone(),
                                name,
                                arguments: arguments.clone(),
                            }],
                        ));
                    }
                }
                SessionRecordKind::ToolResult => {
                    if let (Some(call_id), Some(result)) = (&record.call_id, &record.result) {
                        transcript.push(crate::message::Message::tool_result(
                            call_id.clone(),
                            record.tool.clone().unwrap_or_default(),
                            serde_json::to_string(result).unwrap_or_default(),
                            false,
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok(transcript)
    }
}

impl SessionRecord {
    /// Build a new record with required fields filled in.
    pub fn new(session_id: &SessionId, kind: SessionRecordKind) -> Self {
        Self {
            seq: 0,
            timestamp: SystemTime::now(),
            kind,
            session_id: session_id.clone(),
            role: None,
            content: None,
            tool: None,
            call_id: None,
            arguments: None,
            result: None,
            model: None,
            extra: None,
        }
    }
}

/// Metadata describing a resumable session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Stable, workspace-derived identifier (serialized as its canonical string).
    pub id: SessionId,
    /// Human-facing name (first user message, truncated).
    pub name: String,
    /// Creation time (UTC).
    pub created_at: SystemTime,
    /// Last modification time (UTC).
    pub updated_at: SystemTime,
    /// Number of stored events.
    pub event_count: u64,
    /// Number of stored assistant messages.
    pub message_count: u64,
    /// Model used for the most recent assistant message, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_model: Option<String>,
}

impl SessionInfo {
    /// Derive the info record from the stored JSONL content.
    pub fn from_records(records: &[SessionRecord]) -> Self {
        let (event_count, message_count, last_model, updated_at) = records.iter().fold(
            (
                0_u64,
                0_u64,
                None,
                records
                    .first()
                    .map(|r| r.timestamp)
                    .unwrap_or(SystemTime::now()),
            ),
            |(events, messages, model, updated), record| {
                let events = events + 1;
                let messages = if record.kind == SessionRecordKind::Message {
                    messages + 1
                } else {
                    messages
                };
                let model = match &record.model {
                    Some(name) if record.kind == SessionRecordKind::Message => Some(name.clone()),
                    _ => model,
                };
                (events, messages, model, record.timestamp.max(updated))
            },
        );
        let name = records
            .iter()
            .find(|record| {
                record.kind == SessionRecordKind::Message && record.role.as_deref() == Some("user")
            })
            .map(|record| {
                record
                    .content
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(60)
                    .collect()
            })
            .unwrap_or_else(|| "spackle session".to_owned());
        Self {
            id: records[0].session_id.clone(),
            name,
            created_at: records[0].timestamp,
            updated_at,
            event_count,
            message_count,
            last_model,
        }
    }
}

/// Resolve the platform directories for session storage.
#[derive(Debug, Clone)]
pub struct SessionPaths {
    /// Directory containing per-session journals.
    pub sessions_dir: PathBuf,
    /// Directory for checkpoints.
    pub checkpoints_dir: PathBuf,
}

impl SessionPaths {
    /// Build from an explicit state directory (used by tests and overrides).
    pub fn from_state_dir(state_dir: impl Into<PathBuf>) -> Self {
        let state_dir = state_dir.into();
        Self {
            sessions_dir: state_dir.join("sessions"),
            checkpoints_dir: state_dir.join("checkpoints"),
        }
    }

    /// Journal path for a session.
    pub fn journal_path(&self, session: &SessionId) -> PathBuf {
        self.sessions_dir.join(session.journal_file_name())
    }
}

/// The product name, re-exported for path construction.
#[must_use]
pub fn product_name() -> &'static str {
    PRODUCT_NAME
}

/// A bounded queue of recent records (for loop detection over the journal).
#[derive(Debug, Clone)]
pub struct RecentWindow {
    window: VecDeque<SessionRecord>,
    capacity: usize,
}

impl RecentWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, record: SessionRecord) {
        if self.window.len() == self.capacity {
            self.window.pop_front();
        }
        self.window.push_back(record);
    }

    pub fn iter(&self) -> impl Iterator<Item = &SessionRecord> {
        self.window.iter()
    }

    pub fn len(&self) -> usize {
        self.window.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.window.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_workspace_same_id() {
        let a = SessionId::from_workspace(Path::new("/tmp/some/workspace"));
        let b = SessionId::from_workspace(Path::new("/tmp/some/workspace"));
        assert_eq!(a, b);
    }

    #[test]
    fn different_workspace_different_id() {
        let a = SessionId::from_workspace(Path::new("/tmp/workspace-a"));
        let b = SessionId::from_workspace(Path::new("/tmp/workspace-b"));
        assert_ne!(a, b);
    }

    #[test]
    fn id_has_stable_shape() {
        let id = SessionId::from_workspace(Path::new("/tmp/whatever"));
        let raw = id.as_str();
        assert!(raw.starts_with("ws-"), "{raw}");
        let hex = raw.strip_prefix("ws-").expect("prefix");
        assert_eq!(hex.len(), 16, "{hex}");
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn journal_file_name_ends_in_jsonl() {
        let id = SessionId::from_workspace(Path::new("/tmp/x"));
        assert!(id.journal_file_name().ends_with(".jsonl"));
    }

    #[test]
    fn append_and_read_back() {
        let dir = tempfile_dir();
        let id = SessionId::from_workspace(Path::new("/tmp/ws"));
        let mut store = SessionStore::open(&dir, id.clone()).unwrap();
        let mut rec = SessionRecord::new(&id, SessionRecordKind::Message);
        rec.role = Some("user".into());
        rec.content = Some("hello".into());
        store.append(rec).unwrap();

        let mut rec = SessionRecord::new(&id, SessionRecordKind::Message);
        rec.role = Some("assistant".into());
        rec.content = Some("hi there".into());
        rec.model = Some("qwen3.8-27b-local".into());
        store.append(rec).unwrap();

        let records = store.read_all().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].seq, 0);
        assert_eq!(records[1].seq, 1);

        let info = SessionInfo::from_records(&records);
        assert_eq!(info.event_count, 2);
        assert_eq!(info.message_count, 2);
        assert_eq!(info.name, "hello");
        assert_eq!(info.last_model.as_deref(), Some("qwen3.8-27b-local"));
    }

    #[test]
    fn resume_continues_sequence() {
        let dir = tempfile_dir();
        let id = SessionId::from_workspace(Path::new("/tmp/ws"));
        {
            let mut store = SessionStore::open(&dir, id.clone()).unwrap();
            let mut rec = SessionRecord::new(&id, SessionRecordKind::Message);
            rec.content = Some("x".into());
            store.append(rec).unwrap();
        }
        let store = SessionStore::open(&dir, id.clone()).unwrap();
        assert_eq!(store.next_seq, 1);
    }

    #[test]
    fn torn_trailing_line_is_skipped_with_warning() {
        let dir = tempfile_dir();
        let id = SessionId::from_workspace(Path::new("/tmp/ws"));
        let mut store = SessionStore::open(&dir, id.clone()).unwrap();
        let mut rec = SessionRecord::new(&id, SessionRecordKind::Message);
        rec.content = Some("ok".into());
        store.append(rec).unwrap();
        // Simulate a torn write: append a partial JSON line without newline.
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(store.path())
            .unwrap();
        file.write_all(b"{\"seq\": 1, \"kind\": \"message\"")
            .unwrap();

        let records = store.read_all().unwrap();
        assert_eq!(records.len(), 1);
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spackle-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
