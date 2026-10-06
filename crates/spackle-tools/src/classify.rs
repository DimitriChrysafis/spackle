//! Risk classification for tool calls: which paths look like secrets and
//! which commands are read-only, mutating, destructive, or networked.
//!
//! The approval gate only sees a `describe` string, so each tool embeds a
//! `kind:` prefix (see `ApprovalKind` in the callers' `describe`) that the
//! CLI gate maps onto the configured `ConfirmationPolicy`.

use std::path::Path;

/// Coarse risk class for a shell command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClass {
    /// Pure read/inspect commands (ls, rg, git status, ...). No approval.
    SafeRead,
    /// Anything else: may mutate the workspace. Maps to `confirmations.writes`.
    Write,
    /// Removes/overwrites/privileged operations. Maps to `confirmations.destructive`.
    Destructive,
    /// Touches the network. Maps to `confirmations.network_commands`.
    Network,
}

/// Programs that only read. Test/build runners are *not* here: they execute
/// arbitrary project code, so they still need approval by default.
const SAFE_READ: &[&str] = &[
    "ls", "pwd", "echo", "cat", "head", "tail", "wc", "file", "stat", "which", "find", "rg",
    "grep", "sed", "awk", "sort", "uniq", "tr", "cut", "diff", "cmp", "md5", "shasum", "basename",
    "dirname", "realpath", "date", "uname", "jq", "xxd", "strings", "tree", "true", "false",
];

const SAFE_GIT_SUBCOMMANDS: &[&str] = &[
    "status",
    "log",
    "diff",
    "show",
    "branch",
    "rev-parse",
    "describe",
    "ls-files",
    "blame",
    "shortlog",
    "tag",
    "remote",
    "stash list",
    "show-ref",
];

const DESTRUCTIVE: &[&str] = &[
    "rm", "rmdir", "unlink", "shred", "dd", "mkfs", "fdisk", "diskutil", "kill", "pkill",
    "killall", "shutdown", "reboot", "sudo", "truncate", "chflags",
];

const NETWORK: &[&str] = &[
    "curl", "wget", "ssh", "scp", "sftp", "rsync", "nc", "ncat", "ping", "telnet", "ftp", "brew",
    "npm", "npx", "pnpm", "yarn", "pip", "pip3", "uv", "docker", "kubectl", "helm", "apt",
    "apt-get", "mas",
];

/// Classify a shell command string. The command is split with shell rules;
/// anything unparsable falls back to `Write` (ask first).
#[must_use]
pub fn classify_command(command: &str) -> CommandClass {
    let Ok(tokens) = shell_words::split(command) else {
        return CommandClass::Write;
    };
    // Skip leading VAR=value assignments and bare `env`.
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if token == "env"
            || token
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && token.contains('=')
                && !token.starts_with('-')
        {
            index += 1;
        } else {
            break;
        }
    }
    let Some(program) = tokens.get(index) else {
        return CommandClass::SafeRead; // empty command: nothing runs
    };
    let program = program_name(program);
    let rest: Vec<&str> = tokens[index + 1..].iter().map(String::as_str).collect();

    // Shell composition makes the whole thing unpredictable: `rm x && y`,
    // `> file`, pipes into sh, etc.
    let mutating_combinators = ["|", ">", ">>", "&&", "||", ";", "`", "$("];
    if rest
        .iter()
        .any(|token| mutating_combinators.iter().any(|c| token.starts_with(c)))
        || tokens[index + 1..].iter().any(|t| t.contains("$("))
    {
        // A combinator after a safe read is still a write-class command.
        if DESTRUCTIVE.contains(&program) {
            return CommandClass::Destructive;
        }
        return CommandClass::Write;
    }

    if DESTRUCTIVE.contains(&program) {
        return CommandClass::Destructive;
    }
    if NETWORK.contains(&program) {
        return CommandClass::Network;
    }
    match program {
        "git" => match rest.first().copied() {
            Some(sub) if SAFE_GIT_SUBCOMMANDS.contains(&sub) => CommandClass::SafeRead,
            Some("push") | Some("pull") | Some("fetch") | Some("clone") | Some("ls-remote") => {
                CommandClass::Network
            }
            Some("reset") | Some("clean") | Some("rebase") => CommandClass::Destructive,
            Some(_) => CommandClass::Write,
            None => CommandClass::SafeRead,
        },
        "cargo" => match rest.first().copied() {
            Some("add") | Some("install") | Some("publish") | Some("update") | Some("login") => {
                CommandClass::Network
            }
            Some("clean") => CommandClass::Destructive,
            Some(_) => CommandClass::Write,
            None => CommandClass::SafeRead,
        },
        _ if SAFE_READ.contains(&program) && rest.iter().all(|t| !t.contains("$(")) => {
            CommandClass::SafeRead
        }
        _ => CommandClass::Write,
    }
}

fn program_name(token: &str) -> &str {
    token.rsplit('/').next().unwrap_or(token)
}

/// Whether `path` (workspace-relative or absolute) looks like a file that
/// commonly holds secrets. Conservative name matching only.
#[must_use]
pub fn looks_secret(path: &Path) -> bool {
    let components: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let name = components.last().map(String::as_str).unwrap_or("");
    if components
        .iter()
        .any(|c| matches!(c.as_str(), ".ssh" | ".aws" | ".gnupg" | ".config/gh"))
    {
        return true;
    }
    name.starts_with(".env")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || name.starts_with("id_ecdsa")
        || name.starts_with("id_dsa")
        || matches!(
            name,
            "credentials" | ".netrc" | ".git-credentials" | ".dockercfg" | ".npmrc" | ".pypirc"
        )
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name.ends_with(".p12")
        || name.ends_with(".pfx")
        || name.ends_with(".keystore")
        || name.ends_with("_rsa")
        || name.ends_with("_ed25519")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_commands_are_safe() {
        for cmd in [
            "ls -la",
            "rg foo src/",
            "cat README.md",
            "git status",
            "git log --oneline -5",
            "git diff HEAD",
            "find . -name '*.rs'",
            "FOO=bar ls",
        ] {
            assert_eq!(
                classify_command(cmd),
                CommandClass::SafeRead,
                "{cmd} should be a safe read"
            );
        }
    }

    #[test]
    fn destructive_commands_are_classified() {
        for cmd in [
            "rm -rf target",
            "git reset --hard HEAD",
            "git clean -fd",
            "sudo make install",
            "kill -9 1234",
            "dd if=/dev/zero of=disk.img",
        ] {
            assert_eq!(
                classify_command(cmd),
                CommandClass::Destructive,
                "{cmd} should be destructive"
            );
        }
    }

    #[test]
    fn network_commands_are_classified() {
        for cmd in [
            "curl https://example.com",
            "git push origin main",
            "npm install",
            "cargo add serde",
            "pip install requests",
        ] {
            assert_eq!(
                classify_command(cmd),
                CommandClass::Network,
                "{cmd} should be network"
            );
        }
    }

    #[test]
    fn mutating_commands_need_write_approval() {
        for cmd in [
            "cargo test",
            "python3 -m unittest",
            "cargo build --release",
            "mkdir build",
            "git commit -m x",
            "cat foo > bar.txt",
        ] {
            assert_eq!(
                classify_command(cmd),
                CommandClass::Write,
                "{cmd} should be write-class"
            );
        }
    }

    #[test]
    fn secret_paths_are_detected() {
        for path in [
            ".env",
            ".env.local",
            "config/.env.production",
            ".ssh/id_rsa",
            "keys/server.pem",
            "cert.p12",
            ".aws/credentials",
            ".netrc",
            "deploy/id_ed25519",
        ] {
            assert!(looks_secret(Path::new(path)), "{path} should look secret");
        }
        for path in ["src/main.rs", "README.md", "Cargo.toml", "environment.yml"] {
            assert!(
                !looks_secret(Path::new(path)),
                "{path} should not look secret"
            );
        }
    }
}
