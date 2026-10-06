//! Conversation messages. The core message model is wire-agnostic; the
//! llama.cpp crate maps it onto Chat Completions structures.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// One model-issued tool invocation, captured verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ToolCall {
    /// Stable unique id. Generated locally when the server omits one.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Parsed JSON arguments (`null` when the server sent none).
    pub arguments: serde_json::Value,
}

/// Result of a tool execution, recorded back into the transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ToolResult {
    /// The `ToolCall.id` this result answers.
    pub id: String,
    /// Tool name (repeated for readable journals).
    pub name: String,
    /// Bounded output text.
    pub output: String,
    /// True when the tool reported an error (still a normal conversation
    /// continuation unless the loop policy says otherwise).
    pub is_error: bool,
}

/// A single block inside a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain assistant/user text.
    Text { text: String },
    /// Model reasoning, stored separately and hidden by default.
    Reasoning { text: String },
    /// Assistant tool invocation.
    ToolCall(ToolCall),
    /// Tool result answering a `ToolCall`.
    ToolResult(ToolResult),
}

/// One conversation message.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Message {
    pub id: String,
    pub role: Role,
    pub blocks: Vec<ContentBlock>,
    /// Authoritative token count once the server reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
}

impl Message {
    fn new(role: Role, blocks: Vec<ContentBlock>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role,
            blocks,
            tokens: None,
        }
    }

    /// A user message from plain text.
    #[must_use]
    pub fn user(text: impl Into<String>) -> Self {
        Self::new(Role::User, vec![ContentBlock::Text { text: text.into() }])
    }

    /// A system message (assembled runtime prompt).
    #[must_use]
    pub fn system(text: impl Into<String>) -> Self {
        Self::new(Role::System, vec![ContentBlock::Text { text: text.into() }])
    }

    /// An assistant message with text, reasoning, and tool calls exactly as
    /// streamed by the model. Order is preserved.
    #[must_use]
    pub fn assistant(text: String, reasoning: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        let mut blocks = Vec::new();
        if let Some(reasoning) = reasoning
            && !reasoning.is_empty()
        {
            blocks.push(ContentBlock::Reasoning { text: reasoning });
        }
        if !text.is_empty() {
            blocks.push(ContentBlock::Text { text });
        }
        for call in tool_calls {
            blocks.push(ContentBlock::ToolCall(call));
        }
        Self::new(Role::Assistant, blocks)
    }

    /// A tool result message bound to a tool call id.
    #[must_use]
    pub fn tool_result(id: String, name: String, output: String, is_error: bool) -> Self {
        Self::new(
            Role::Tool,
            vec![ContentBlock::ToolResult(ToolResult {
                id,
                name,
                output,
                is_error,
            })],
        )
    }

    /// Plain text of this message: text blocks plus tool-result output,
    /// joined in block order (reasoning is excluded; use `reasoning()`).
    #[must_use]
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                ContentBlock::ToolResult(result) => Some(result.output.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Reasoning content of this message.
    #[must_use]
    pub fn reasoning(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Reasoning { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Tool calls in this message, in wire order.
    #[must_use]
    pub fn tool_calls(&self) -> Vec<ToolCall> {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolCall(call) => Some(call.clone()),
                _ => None,
            })
            .collect()
    }

    /// Estimated size in bytes, used for compaction heuristics when the
    /// server does not report tokens.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        let mut size = 16;
        for block in &self.blocks {
            size += match block {
                ContentBlock::Text { text } => text.len() + 8,
                ContentBlock::Reasoning { text } => text.len() + 8,
                ContentBlock::ToolCall(call) => {
                    call.name.len() + call.arguments.to_string().len() + 16
                }
                ContentBlock::ToolResult(result) => result.output.len() + 24,
            };
        }
        size
    }
}

/// A full transcript (system prompt message excluded or included depending
/// on caller).
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Transcript {
    pub messages: Vec<Message>,
}

impl Transcript {
    /// Append a message.
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Total known tokens (only messages with an authoritative count).
    #[must_use]
    pub fn known_tokens(&self) -> u64 {
        self.messages
            .iter()
            .filter_map(|message| message.tokens)
            .sum()
    }

    /// Estimated bytes of the whole transcript.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        self.messages.iter().map(Message::byte_size).sum()
    }

    /// Drop the leading `count` messages (used by compaction, which must
    /// only ever run between turns).
    pub fn compact_front(&mut self, count: usize) {
        if count < self.messages.len() {
            self.messages.drain(..count);
        } else {
            self.messages.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_message_preserves_tool_calls_verbatim() {
        let call = ToolCall {
            id: "call_1".to_owned(),
            name: "inspect_files".to_owned(),
            arguments: serde_json::json!({ "paths": ["a.rs", "b.rs"] }),
        };
        let message = Message::assistant(
            "I will inspect these files.".to_owned(),
            Some("thinking...".to_owned()),
            vec![call.clone()],
        );
        assert_eq!(message.tool_calls(), vec![call]);
        assert_eq!(message.text(), "I will inspect these files.");
        assert_eq!(message.reasoning(), "thinking...");
        // Round trip must be lossless: the assistant tool-call message is
        // replayed exactly before appending results.
        let text = serde_json::to_string(&message).expect("serialize");
        let back: Message = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(back.id, message.id);
        assert_eq!(back.role, message.role);
        assert_eq!(back.text(), message.text());
        assert_eq!(back.reasoning(), message.reasoning());
        assert_eq!(back.tool_calls(), message.tool_calls());
    }

    #[test]
    fn transcript_ops_are_bounded() {
        let mut transcript = Transcript::default();
        for index in 0..10 {
            transcript.push(Message::user(format!("line {index}")));
        }
        assert_eq!(transcript.messages.len(), 10);
        transcript.compact_front(4);
        assert_eq!(transcript.messages.len(), 6);
        assert_eq!(transcript.messages[0].text(), "line 4");
        transcript.compact_front(100);
        assert!(transcript.messages.is_empty());
    }
}
