//! Integration tests for the sandboxed workspace tools.

use serde_json::{Value, json};
use spackle_core::agent::{ToolExecutor, ToolOutput};
use spackle_core::cancel::CancellationToken;
use spackle_core::message::ToolCall;
use spackle_tools::{
    CommandClass, EditFile, Grep, ListFiles, ReadFile, RunCommand, WorkspaceRoot, WriteFile,
    classify_command,
};

fn workspace() -> (tempfile::TempDir, WorkspaceRoot) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = WorkspaceRoot::new(dir.path()).expect("root");
    (dir, root)
}

fn call(name: &str, arguments: Value) -> ToolCall {
    ToolCall {
        id: "test-call".to_owned(),
        name: name.to_owned(),
        arguments,
    }
}

async fn run(
    tool: &dyn ToolExecutor,
    c: &ToolCall,
) -> Result<ToolOutput, spackle_core::agent::ToolError> {
    tool.execute(c, CancellationToken::new()).await
}

// --- read_file ------------------------------------------------------------

#[tokio::test]
async fn read_file_returns_numbered_lines() {
    let (_dir, root) = workspace();
    std::fs::write(root.as_path().join("a.txt"), "one\ntwo\nthree\n").unwrap();
    let tool = ReadFile::new(root, Default::default());
    let out = run(&tool, &call("read_file", json!({"path": "a.txt"})))
        .await
        .unwrap();
    assert!(out.text.contains("1|one"));
    assert!(out.text.contains("3|three"));
    assert!(!out.is_error);
}

#[tokio::test]
async fn read_file_pages_with_offset_and_limit() {
    let (_dir, root) = workspace();
    let body: String = (1..=50).map(|i| format!("line{i}\n")).collect();
    std::fs::write(root.as_path().join("big.txt"), body).unwrap();
    let tool = ReadFile::new(root, Default::default());
    let out = run(
        &tool,
        &call(
            "read_file",
            json!({"path": "big.txt", "offset": 10, "limit": 3}),
        ),
    )
    .await
    .unwrap();
    assert!(out.text.contains("10|line10"));
    assert!(out.text.contains("12|line12"));
    assert!(!out.text.contains("13|"));
}

#[tokio::test]
async fn read_file_rejects_traversal_and_missing() {
    let (_dir, root) = workspace();
    let tool = ReadFile::new(root, Default::default());
    for args in [
        json!({"path": "../escape"}),
        json!({"path": "/etc/passwd"}),
        json!({"path": "nope.txt"}),
    ] {
        let result = run(&tool, &call("read_file", args.clone())).await;
        assert!(result.is_err(), "{args}");
    }
}

#[tokio::test]
async fn read_file_flags_secret_paths_for_approval() {
    let (_dir, root) = workspace();
    let tool = ReadFile::new(root, Default::default());
    let secret = call("read_file", json!({"path": ".env"}));
    let normal = call("read_file", json!({"path": "src/main.rs"}));
    assert!(tool.requires_approval(&secret));
    assert!(!tool.requires_approval(&normal));
    assert!(tool.describe(&secret).starts_with("secret:"));
}

// --- list_files -----------------------------------------------------------

#[tokio::test]
async fn list_files_lists_and_respects_gitignore() {
    let (_dir, root) = workspace();
    std::fs::create_dir_all(root.as_path().join("src")).unwrap();
    std::fs::write(root.as_path().join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(root.as_path().join(".gitignore"), "ignored.txt\n").unwrap();
    std::fs::write(root.as_path().join("ignored.txt"), "x").unwrap();
    std::fs::write(root.as_path().join("keep.txt"), "x").unwrap();
    let tool = ListFiles::new(root);
    let out = run(&tool, &call("list_files", json!({}))).await.unwrap();
    assert!(out.text.contains("src/main.rs"), "{}", out.text);
    assert!(out.text.contains("keep.txt"));
    assert!(!out.text.contains("ignored.txt"), "gitignore honored");
}

#[tokio::test]
async fn list_files_glob_filters() {
    let (_dir, root) = workspace();
    std::fs::write(root.as_path().join("a.rs"), "x").unwrap();
    std::fs::write(root.as_path().join("b.py"), "x").unwrap();
    let tool = ListFiles::new(root);
    let out = run(&tool, &call("list_files", json!({"pattern": "*.rs"})))
        .await
        .unwrap();
    assert!(out.text.contains("a.rs"));
    assert!(!out.text.contains("b.py"));
}

// --- grep -----------------------------------------------------------------

#[tokio::test]
async fn grep_finds_matches_with_line_numbers() {
    let (_dir, root) = workspace();
    std::fs::write(root.as_path().join("a.rs"), "fn alpha() {}\nfn beta() {}\n").unwrap();
    let tool = Grep::new(root);
    let out = run(&tool, &call("grep", json!({"pattern": "beta"})))
        .await
        .unwrap();
    assert!(out.text.contains("a.rs:2:fn beta() {}"), "{}", out.text);
}

#[tokio::test]
async fn grep_skips_secret_files() {
    let (_dir, root) = workspace();
    std::fs::write(root.as_path().join(".env"), "TOKEN=hunter2\n").unwrap();
    std::fs::write(root.as_path().join("plain.txt"), "TOKEN=public\n").unwrap();
    let tool = Grep::new(root);
    let out = run(&tool, &call("grep", json!({"pattern": "TOKEN"})))
        .await
        .unwrap();
    assert!(out.text.contains("plain.txt"));
    assert!(
        !out.text.contains("hunter2"),
        "secret content must not leak"
    );
    assert!(out.text.contains("skipped"));
}

#[tokio::test]
async fn grep_rejects_bad_regex() {
    let (_dir, root) = workspace();
    let tool = Grep::new(root);
    let result = run(&tool, &call("grep", json!({"pattern": "(((("}))).await;
    assert!(result.is_err());
}

// --- write_file / edit_file -----------------------------------------------

#[tokio::test]
async fn write_then_edit_round_trip() {
    let (_dir, root) = workspace();
    let writer = WriteFile::new(root.clone(), None);
    let editor = EditFile::new(root.clone(), None);
    run(
        &writer,
        &call(
            "write_file",
            json!({"path": "code.py", "content": "x = 1\nprint(x)\n"}),
        ),
    )
    .await
    .unwrap();
    let out = run(
        &editor,
        &call(
            "edit_file",
            json!({"path": "code.py", "old_text": "x = 1", "new_text": "x = 2"}),
        ),
    )
    .await
    .unwrap();
    assert!(out.text.contains("edited code.py"));
    assert!(
        out.text.contains("-x = 1"),
        "diff shows removal: {}",
        out.text
    );
    assert!(
        out.text.contains("+x = 2"),
        "diff shows addition: {}",
        out.text
    );
    assert_eq!(
        std::fs::read_to_string(root.as_path().join("code.py")).unwrap(),
        "x = 2\nprint(x)\n"
    );
}

#[tokio::test]
async fn edit_requires_unique_match() {
    let (_dir, root) = workspace();
    std::fs::write(root.as_path().join("dup.txt"), "same\nsame\n").unwrap();
    let tool = EditFile::new(root, None);
    let result = run(
        &tool,
        &call(
            "edit_file",
            json!({"path": "dup.txt", "old_text": "same", "new_text": "x"}),
        ),
    )
    .await;
    let err = result.expect_err("ambiguous match must fail");
    assert!(err.to_string().contains("2 places"), "{err}");

    let result = run(
        &tool,
        &call(
            "edit_file",
            json!({"path": "dup.txt", "old_text": "missing", "new_text": "x"}),
        ),
    )
    .await;
    assert!(
        result
            .expect_err("missing must fail")
            .to_string()
            .contains("not found")
    );
}

#[tokio::test]
async fn edit_replace_all_covers_every_match() {
    let (_dir, root) = workspace();
    std::fs::write(root.as_path().join("dup.txt"), "same\nsame\n").unwrap();
    let tool = EditFile::new(root.clone(), None);
    run(
        &tool,
        &call(
            "edit_file",
            json!({"path": "dup.txt", "old_text": "same", "new_text": "x", "replace_all": true}),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.as_path().join("dup.txt")).unwrap(),
        "x\nx\n"
    );
}

#[tokio::test]
async fn write_refuses_workspace_escape() {
    let (_dir, root) = workspace();
    let tool = WriteFile::new(root, None);
    for path in ["../out.txt", "/tmp/out.txt", "a/../../out.txt"] {
        let result = run(
            &tool,
            &call("write_file", json!({"path": path, "content": "x"})),
        )
        .await;
        assert!(result.is_err(), "{path} must be refused");
    }
}

#[tokio::test]
async fn read_guard_blocks_blind_edits_until_read() {
    let (_dir, root) = workspace();
    std::fs::write(root.as_path().join("f.txt"), "alpha\nbeta\n").unwrap();
    let reads: spackle_tools::ReadTracker = Default::default();
    let reader = ReadFile::new(root.clone(), reads.clone());
    let editor = EditFile::new(root.clone(), Some(reads.clone()));
    let writer = WriteFile::new(root.clone(), Some(reads));

    let blind = call(
        "edit_file",
        json!({"path": "f.txt", "old_text": "alpha", "new_text": "A"}),
    );
    let err = run(&editor, &blind)
        .await
        .expect_err("blind edit must fail");
    assert!(err.to_string().contains("read_file"), "{err}");

    let blind_write = run(
        &writer,
        &call("write_file", json!({"path": "f.txt", "content": "new\n"})),
    )
    .await;
    assert!(blind_write.is_err(), "blind overwrite must fail");

    run(&reader, &call("read_file", json!({"path": "f.txt"})))
        .await
        .unwrap();
    run(&editor, &blind).await.expect("edit after read");

    run(
        &writer,
        &call(
            "write_file",
            json!({"path": "fresh.txt", "content": "new file ok\n"}),
        ),
    )
    .await
    .expect("new files never need a prior read");
}

// --- run_command ----------------------------------------------------------

#[tokio::test]
async fn run_command_captures_output_and_exit_code() {
    let (_dir, root) = workspace();
    std::fs::write(root.as_path().join("marker.txt"), "hi").unwrap();
    let tool = RunCommand::new(root);
    let out = run(
        &tool,
        &call("run_command", json!({"command": "ls marker.txt && exit 3"})),
    )
    .await
    .unwrap();
    assert!(out.is_error, "exit 3 should mark error");
    assert!(out.text.contains("exit=3"), "{}", out.text);
    assert!(out.text.contains("marker.txt"));
}

#[tokio::test]
async fn run_command_kills_on_timeout() {
    let (_dir, root) = workspace();
    let tool = RunCommand::new(root);
    let start = std::time::Instant::now();
    let out = run(
        &tool,
        &call(
            "run_command",
            json!({"command": "sleep 30", "timeout_seconds": 1}),
        ),
    )
    .await
    .unwrap();
    assert!(start.elapsed() < std::time::Duration::from_secs(10));
    assert!(out.is_error);
    assert!(out.text.contains("killed"), "{}", out.text);
}

#[tokio::test]
async fn run_command_cancel_kills_child() {
    let (_dir, root) = workspace();
    let tool = RunCommand::new(root);
    let cancel = CancellationToken::new();
    let cancel2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        cancel2.cancel();
    });
    let result = tool
        .execute(&call("run_command", json!({"command": "sleep 30"})), cancel)
        .await;
    assert!(matches!(
        result,
        Err(spackle_core::agent::ToolError::Cancelled)
    ));
}

#[tokio::test]
async fn run_command_output_is_bounded() {
    let (_dir, root) = workspace();
    let tool = RunCommand::new(root);
    let out = run(
        &tool,
        &call("run_command", json!({"command": "seq 1 200000"})),
    )
    .await
    .unwrap();
    assert!(out.text.len() < 80 * 1024, "output must be capped");
    assert!(out.text.contains("truncated"));
}

#[tokio::test]
async fn approval_needed_only_for_mutating_commands() {
    let (_dir, root) = workspace();
    let tool = RunCommand::new(root);
    let read = call("run_command", json!({"command": "ls"}));
    let build = call("run_command", json!({"command": "cargo test"}));
    let wipe = call("run_command", json!({"command": "rm -rf x"}));
    assert!(!tool.requires_approval(&read));
    assert!(tool.requires_approval(&build));
    assert!(tool.requires_approval(&wipe));
    assert_eq!(classify_command("rm -rf x"), CommandClass::Destructive);
    assert!(tool.describe(&wipe).starts_with("command-destructive:"));
    assert!(tool.describe(&build).starts_with("command:"));
}
