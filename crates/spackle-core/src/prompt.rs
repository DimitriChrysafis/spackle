//! Runtime system prompt assembly.
//!
//! The rendered prompt must be byte-stable across turns for a fixed set of
//! inputs so llama.cpp can reuse its prefix/KV cache. Sections are fixed and
//! deterministically ordered:
//!
//! 1. base product prompt (static, embedded at build time)
//! 2. project instructions (`.spackle/instructions.md`), when present
//! 3. environment facts (workspace, model, platform), when known
//!
//! llama.cpp's chat template owns tool-call formatting; we never duplicate
//! its XML/tool instructions here.

/// The static base prompt, byte-stable across turns.
pub const BASE_PROMPT: &str = include_str!("../../../prompts/coding-agent.md");

/// Inputs for assembling the runtime system prompt.
#[derive(Debug, Clone, Default)]
pub struct SystemPromptInput {
    /// Project instructions (`.spackle/instructions.md`), trimmed.
    pub project_instructions: Option<String>,
    /// Canonical workspace path.
    pub workspace: Option<String>,
    /// Model alias served by llama.cpp.
    pub model: Option<String>,
    /// Target triple / OS description for platform notes.
    pub platform: Option<String>,
    /// Generation profile name (affects sampling, not prompt text; included
    /// only as an environment fact for model self-awareness).
    pub profile: Option<String>,
}

/// An assembled, deterministic system prompt.
#[derive(Debug, Clone)]
pub struct SystemPrompt {
    rendered: String,
}

impl SystemPrompt {
    /// Render from inputs. Same inputs always yield the same bytes.
    pub fn assemble(input: &SystemPromptInput) -> Self {
        let mut out = String::with_capacity(BASE_PROMPT.len() + 512);
        let base = BASE_PROMPT.trim_end();
        out.push_str(base);
        out.push('\n');

        if let Some(instructions) = input
            .project_instructions
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            out.push_str("\n## Project instructions\n");
            out.push_str(instructions);
            out.push('\n');
        }

        let facts: Vec<(&str, &str)> = [
            ("workspace", input.workspace.as_deref()),
            ("model", input.model.as_deref()),
            ("profile", input.profile.as_deref()),
            ("platform", input.platform.as_deref()),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
        .collect();
        if !facts.is_empty() {
            out.push_str("\n## Environment\n");
            for (key, value) in facts {
                out.push_str(&format!("- {key}: {value}\n"));
            }
        }

        Self { rendered: out }
    }

    /// The rendered prompt text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.rendered
    }

    /// Stable hash for cache-keying and change detection.
    #[must_use]
    pub fn hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.rendered.as_bytes());
        let digest = hasher.finalize();
        digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> SystemPromptInput {
        SystemPromptInput {
            project_instructions: Some("  Prefer small diffs.  ".to_owned()),
            workspace: Some("/tmp/repo".to_owned()),
            model: Some("qwen3.8-27b-local".to_owned()),
            platform: Some("aarch64-apple-darwin".to_owned()),
            profile: Some("balanced".to_owned()),
        }
    }

    #[test]
    fn assembly_is_byte_stable() {
        let a = SystemPrompt::assemble(&input()).as_str().to_owned();
        let b = SystemPrompt::assemble(&input()).as_str().to_owned();
        assert_eq!(a, b);
    }

    #[test]
    fn sections_appear_in_fixed_order() {
        let text = SystemPrompt::assemble(&input());
        let base_pos = text.as_str().find("You are Spackle").expect("base");
        let project_pos = text
            .as_str()
            .find("## Project instructions")
            .expect("project section");
        let env_pos = text.as_str().find("## Environment").expect("env section");
        assert!(base_pos < project_pos && project_pos < env_pos);
    }

    #[test]
    fn instructions_are_trimmed_and_included() {
        let text = SystemPrompt::assemble(&input()).as_str().to_owned();
        assert!(text.contains("Prefer small diffs."));
    }

    #[test]
    fn missing_sections_are_omitted_cleanly() {
        let text = SystemPrompt::assemble(&SystemPromptInput::default());
        assert!(!text.as_str().contains("## Project instructions"));
        assert!(!text.as_str().contains("## Environment"));
        assert!(text.as_str().contains("You are Spackle"));
    }

    #[test]
    fn blank_instructions_are_treated_as_absent() {
        let text = SystemPrompt::assemble(&SystemPromptInput {
            project_instructions: Some("   \n  ".to_owned()),
            ..Default::default()
        });
        assert!(!text.as_str().contains("## Project instructions"));
    }

    #[test]
    fn hash_is_stable_and_short() {
        let a = SystemPrompt::assemble(&input()).hash();
        let b = SystemPrompt::assemble(&input()).hash();
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn base_prompt_embeds_repo_file() {
        // The embedded prompt must be the repository's canonical file so it
        // cannot drift from prompts/coding-agent.md.
        assert!(BASE_PROMPT.contains("local coding agent"));
        assert!(BASE_PROMPT.contains("evidence"));
    }
}
