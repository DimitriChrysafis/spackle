//! Byte-safe Server-Sent Events parser for llama.cpp chat streams.
//!
//! The parser is incremental and byte-oriented: callers feed raw TCP
//! chunks of any size (possibly splitting a line, a UTF-8 character, or
//! the `data:` prefix itself), and complete `data` payloads come out in
//! order. Only complete lines are ever decoded, so multi-byte UTF-8
//! sequences split across chunks reassemble correctly.
//!
//! Per the SSE spec, a field is `name:value` with one optional leading
//! space in the value; multiple `data` lines in one event are joined
//! with `\n`; a blank line dispatches the event; lines starting with
//! `:` are comments. llama.cpp emits `data: <json>` events followed by
//! `data: [DONE]`.

use std::fmt;

/// Parse error (a complete line was malformed at the byte level).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseError {
    /// A line that was not decodable as UTF-8 once complete.
    InvalidUtf8,
}

impl fmt::Display for SseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SseError::InvalidUtf8 => {
                write!(f, "SSE line contained invalid UTF-8")
            }
        }
    }
}

impl std::error::Error for SseError {}

/// Incremental SSE `data` parser.
#[derive(Debug, Default)]
pub struct SseParser {
    /// Bytes of the not-yet-dispatched current event (its `data` lines).
    event_lines: Vec<String>,
    /// Bytes of the current line not yet terminated by a newline.
    pending: Vec<u8>,
}

impl SseParser {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Consume one raw chunk and return all `data` payloads that
    /// completed inside it, in order.
    pub fn feed(
        &mut self,
        chunk: &[u8],
    ) -> Result<Vec<String>, SseError> {
        self.pending.extend_from_slice(chunk);
        let mut out: Vec<String> = Vec::new();
        loop {
            let newline = match self.pending.iter().position(|b| *b == b'\n') {
                Some(pos) => pos,
                None => break,
            };
            let mut line: Vec<u8> = self.pending.drain(..=newline).collect();
            // Tolerate CRLF: strip one trailing CR.
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            match self.handle_line(&line)? {
                Some(payload) => out.push(payload),
                None => {}
            }
        }
        Ok(out)
    }

    /// Dispatch any event left pending when the stream ends (llama.cpp
    /// terminates with `[DONE]`, so this is defensive).
    pub fn finish(&mut self) -> Option<String> {
        if !self.pending.is_empty() {
            let line: Vec<u8> = std::mem::take(&mut self.pending);
            if let Some(payload) = self.handle_line(&line).expect("finish: pending bytes were already validated per line") {
                return Some(payload);
            }
        }
        if !self.event_lines.is_empty() {
            let payload = self.event_lines.join("\n");
            self.event_lines.clear();
            if !payload.is_empty() {
                return Some(payload);
            }
        }
        None
    }

    /// Reset state (between independent streams).
    pub fn reset(&mut self) {
        self.event_lines.clear();
        self.pending.clear();
    }

    fn handle_line(&mut self, line: &[u8]) -> Result<Option<String>, SseError> {
        let text = String::from_utf8(line.to_vec())
            .map_err(|_| SseError::InvalidUtf8)?;
        if text.is_empty() {
            // Blank line dispatches the event.
            if self.event_lines.is_empty() {
                return Ok(None);
            }
            let payload = self.event_lines.join("\n");
            self.event_lines.clear();
            if payload.is_empty() {
                return Ok(None);
            }
            return Ok(Some(payload));
        }
        if text.starts_with(':') {
            // Comment (e.g. `: ping` keep-alive).
            return Ok(None);
        }
        let (name, value) = match text.find(':') {
            Some(pos) => {
                let name = &text[..pos];
                let rest = &text[pos + 1..];
                // One optional leading space, per the SSE spec.
                let value = rest.strip_prefix(' ').unwrap_or(rest);
                (name, value)
            }
            None => (text.as_str(), ""),
        };
        if name == "data" {
            self.event_lines.push(value.to_owned());
            Ok(None)
        } else {
            // `event:`, `id:`, `retry:` are not used by llama.cpp; ignore.
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(parser: &mut SseParser, text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for chunk in text.as_bytes().chunks(1) {
            out.extend(parser.feed(chunk).expect("feed"));
        }
        if let Some(tail) = parser.finish() {
            out.push(tail);
        }
        out
    }

    #[test]
    fn parses_single_data_event() {
        let mut parser = SseParser::new();
        let events = parser
            .feed(b"data: {\"x\":1}\n\n")
            .expect("feed");
        assert_eq!(events, vec!["{\"x\":1}"]);
    }

    #[test]
    fn parses_byte_by_byte() {
        let mut parser = SseParser::new();
        let text = "data: hello world\n\ndata: second\n\n";
        let events = feed_all(&mut parser, text);
        assert_eq!(events, vec!["hello world", "second"]);
    }

    #[test]
    fn handles_crlf_line_endings() {
        let mut parser = SseParser::new();
        let events = parser.feed(b"data: a\r\n\r\ndata: b\r\n\r\n").expect("feed");
        assert_eq!(events, vec!["a", "b"]);
    }

    #[test]
    fn ignores_comments_and_unknown_fields() {
        let mut parser = SseParser::new();
        let events = parser
            .feed(b": ping\nevent: message\ndata: payload\nid: 7\nretry: 3000\n\n")
            .expect("feed");
        assert_eq!(events, vec!["payload"]);
    }

    #[test]
    fn multi_line_data_is_joined_with_newline() {
        let mut parser = SseParser::new();
        let events = parser.feed(b"data: line1\ndata: line2\n\n").expect("feed");
        assert_eq!(events, vec!["line1\nline2"]);
    }

    #[test]
    fn splits_multi_byte_utf8_across_chunks() {
        let mut parser = SseParser::new();
        // "héllo → 世界" contains 2-byte (é), 3-byte (→), and 3-byte (世/界) chars.
        let text = "data: héllo → 世界\n\n";
        let bytes = text.as_bytes().to_vec();
        // Feed in chunks that split inside multi-byte sequences.
        let mut out = Vec::new();
        let split_points = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut start = 0;
        for split in split_points {
            if split >= bytes.len() {
                break;
            }
            let chunk = &bytes[start..split];
            out.extend(parser.feed(chunk).expect("feed"));
            start = split;
        }
        out.extend(parser.feed(&bytes[start..]).expect("feed"));
        assert_eq!(out, vec!["héllo → 世界"]);
    }

    #[test]
    fn done_sentinel_split_across_chunks() {
        let mut parser = SseParser::new();
        // Feed one byte at a time so `[DONE]` is split many ways.
        let mut out = Vec::new();
        for chunk in b"data: [DONE]\n\n".chunks(1) {
            out.extend(parser.feed(chunk).expect("feed"));
        }
        assert_eq!(out, vec!["[DONE]"]);
    }

    #[test]
    fn no_event_emitted_without_blank_line() {
        let mut parser = SseParser::new();
        let events = parser.feed(b"data: incomplete\n").expect("feed");
        assert!(events.is_empty());
        let events = parser.feed(b"more\n\n").expect("feed");
        assert!(events.is_empty(), "non-data lines must not create an event");
    }

    #[test]
    fn data_value_with_leading_colon_is_preserved() {
        let mut parser = SseParser::new();
        let events = parser.feed(b"data: 1:2:3\n\n").expect("feed");
        assert_eq!(events, vec!["1:2:3"]);
    }

    #[test]
    fn invalid_utf8_complete_line_is_an_error() {
        let mut parser = SseParser::new();
        let err = parser.feed(b"data: \xff\xfe\n\n").expect_err("must fail");
        assert_eq!(err, SseError::InvalidUtf8);
    }

    #[test]
    fn many_events_in_one_chunk() {
        let mut parser = SseParser::new();
        let events = parser
            .feed(b"data: a\n\ndata: b\n\ndata: c\n\n")
            .expect("feed");
        assert_eq!(events, vec!["a", "b", "c"]);
    }

    #[test]
    fn finish_flushes_pending_event() {
        let mut parser = SseParser::new();
        parser.feed(b"data: tail\n").expect("feed");
        assert_eq!(parser.finish(), Some("tail".to_owned()));
    }

    #[test]
    fn reset_discards_state() {
        let mut parser = SseParser::new();
        parser.feed(b"data: stale\n").expect("feed");
        parser.reset();
        assert_eq!(parser.feed(b"\n\n").expect("feed"), Vec::<String>::new());
    }
}
