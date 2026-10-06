//! Configuration: types, loading with documented precedence, validation,
//! and atomic persistence.
//!
//! Precedence (highest first): CLI flags, `SPACKLE_AGENT_*` environment
//! variables, project config (`.spackle/config.toml`), global config
//! (`<config dir>/config.toml`), built-in defaults. `LLAMA_ARG_*` belongs to
//! llama.cpp and is never consumed here; its presence is reported as a
//! warning.

pub mod agent;
pub mod context;
pub mod endpoint;
pub mod error;
pub mod generation;
pub mod managed;
pub mod sessions;

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use agent::validate as validate_agent;
use context::validate as validate_context;
use endpoint::{EndpointConfig, EndpointInfo, validate as validate_endpoint};
use error::{ConfigError, Issue};
use generation::{GenerationConfig, validate as validate_generation};
use managed::{ManagedServerConfig, validate as validate_managed};
use sessions::{SessionsConfig, validate as validate_sessions};

pub use error::Issue as ConfigIssue;

pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const INSTRUCTIONS_FILE_NAME: &str = "instructions.md";
pub const PROJECT_DIR_NAME: &str = ".spackle";
pub const SUPPORTED_VERSION: u32 = 1;

pub use crate::{DEFAULT_BASE_URL, DEFAULT_MODEL};

/// The complete runtime configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Configuration schema version. Must stay `1`.
    pub version: u32,
    pub endpoint: EndpointConfig,
    pub generation: GenerationConfig,
    pub agent: agent::AgentConfig,
    pub context: context::ContextConfig,
    pub sessions: SessionsConfig,
    pub managed_server: ManagedServerConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: SUPPORTED_VERSION,
            endpoint: EndpointConfig::default(),
            generation: GenerationConfig::default(),
            agent: agent::AgentConfig::default(),
            context: context::ContextConfig::default(),
            sessions: SessionsConfig::default(),
            managed_server: ManagedServerConfig::default(),
        }
    }
}

impl Config {
    /// Run all semantic validation, collecting every issue found.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut issues: Vec<Issue> = Vec::new();
        if self.version != SUPPORTED_VERSION {
            issues.push(
                Issue::new(
                    "version",
                    format!("unsupported configuration version {}", self.version),
                )
                .with_hint(format!("spackle supports version {SUPPORTED_VERSION}")),
            );
        }
        if let Err(error) = validate_endpoint(&self.endpoint) {
            issues.extend(error.into_issues());
        }
        if let Err(error) = validate_generation(&self.generation) {
            issues.extend(error.into_issues());
        }
        if let Err(error) = validate_agent(&self.agent) {
            issues.extend(error.into_issues());
        }
        if let Err(error) = validate_context(&self.context) {
            issues.extend(error.into_issues());
        }
        if let Err(error) = validate_sessions(&self.sessions) {
            issues.extend(error.into_issues());
        }
        // Managed-server details only matter when managed mode is selected;
        // they are still validated (cheaply) to keep `config check` honest.
        if self.endpoint.mode == endpoint::EndpointMode::Managed
            && let Err(error) = validate_managed(&self.managed_server)
        {
            issues.extend(error.into_issues());
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::new(issues))
        }
    }

    /// Parse the endpoint for display and policy decisions.
    pub fn endpoint_info(&self) -> Option<EndpointInfo> {
        EndpointInfo::parse(&self.endpoint.base_url).ok()
    }
}

/// Where a configuration value ultimately came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Default,
    Global,
    Project,
    Env,
    Cli,
}

/// A fully resolved configuration plus provenance metadata.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    /// Validated configuration.
    pub config: Config,
    /// Directory containing `.spackle`, when found.
    pub project_dir: Option<PathBuf>,
    /// Project instructions (`.spackle/instructions.md`), when present.
    pub instructions: Option<String>,
    /// Global configuration path (whether or not the file exists).
    pub global_path: PathBuf,
    /// Project configuration path (whether or not the file exists).
    pub project_path: PathBuf,
    /// State directory for journals and checkpoints.
    pub state_dir: PathBuf,
    /// Per-top-level-section source.
    pub sources: BTreeMap<String, SourceKind>,
    /// Non-fatal warnings (e.g. `LLAMA_ARG_*` in the environment).
    pub warnings: Vec<String>,
}

/// Explicit file locations, overridable for tests.
#[derive(Debug, Clone)]
pub struct LoaderPaths {
    /// Directory containing the global `config.toml`.
    pub config_dir: PathBuf,
    /// Where state (journals, checkpoints) lives.
    pub state_dir: PathBuf,
}

/// Configuration loader with injectable locations and environment.
#[derive(Debug, Clone, Default)]
pub struct Loader {
    /// Starting directory for project discovery (usually the workspace or
    /// current directory).
    pub start_dir: PathBuf,
    /// Injected environment (usually `std::env::vars()`).
    pub env: HashMap<String, String>,
    /// CLI overrides as `dotted.path -> value`.
    pub cli_overrides: BTreeMap<String, toml::Value>,
    /// Optional explicit locations; derived from platform dirs when absent.
    pub paths: Option<LoaderPaths>,
}

impl Loader {
    /// Loader for the standard platform locations.
    #[must_use]
    pub fn standard(start_dir: impl Into<PathBuf>) -> Self {
        Self {
            start_dir: start_dir.into(),
            env: std::env::vars().collect(),
            ..Self::default()
        }
    }

    fn standard_paths() -> LoaderPaths {
        let dirs = directories::ProjectDirs::from("local", "", crate::PRODUCT_NAME);
        match dirs {
            Some(dirs) => LoaderPaths {
                config_dir: dirs.config_dir().to_path_buf(),
                state_dir: dirs.data_dir().to_path_buf(),
            },
            None => {
                let home = std::path::PathBuf::from(
                    std::env::var("HOME").unwrap_or_else(|_| ".".to_owned()),
                );
                LoaderPaths {
                    config_dir: home.join(".config").join(crate::PRODUCT_NAME),
                    state_dir: home.join(".local").join("state").join(crate::PRODUCT_NAME),
                }
            }
        }
    }

    /// Discover the project directory: the nearest ancestor of `start_dir`
    /// (inclusive) that contains a `.spackle` directory.
    fn find_project_dir(start_dir: &Path) -> Option<PathBuf> {
        let mut dir = start_dir;
        loop {
            if dir.join(PROJECT_DIR_NAME).is_dir() {
                return Some(dir.to_path_buf());
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => return None,
            }
        }
    }

    fn read_config_file(path: &Path) -> Result<toml::Value, Issue> {
        let text = std::fs::read_to_string(path).map_err(|err| {
            Issue::new(
                "config.file",
                format!("cannot read {}: {err}", path.display()),
            )
            .with_hint("check permissions and TOML syntax")
        })?;
        toml::from_str(&text).map_err(|err| {
            Issue::new(
                "config.file",
                format!("invalid TOML in {}: {err}", path.display()),
            )
            .with_hint("run `spackle config check` after fixing the file")
        })
    }

    fn merge(target: &mut toml::Value, overlay: toml::Value) {
        match (target, overlay) {
            (toml::Value::Table(base), toml::Value::Table(over)) => {
                for (key, value) in over.iter() {
                    match base.get_mut(key) {
                        Some(slot) => Self::merge(slot, value.clone()),
                        None => {
                            base.insert(key.clone(), value.clone());
                        }
                    }
                }
            }
            (slot, value) => *slot = value,
        }
    }

    /// Load, merge (defaults < global < project < env < CLI), and validate.
    pub fn load(self) -> Result<LoadedConfig, ConfigError> {
        let mut issues: Vec<Issue> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        // llama.cpp owns the LLAMA_ARG_* namespace; flag any use so users
        // notice accidental collisions.
        let llamacpp_arg_vars: Vec<String> = self
            .env
            .keys()
            .filter(|key| key.starts_with("LLAMA_ARG_"))
            .cloned()
            .collect();
        for var in &llamacpp_arg_vars {
            warnings.push(format!(
                "{var} is set, but spackle does not read LLAMA_ARG_* variables \
                 (that namespace belongs to llama.cpp)"
            ));
        }

        let paths = self.paths.clone().unwrap_or_else(Self::standard_paths);
        let global_path = paths.config_dir.join(CONFIG_FILE_NAME);

        let project_dir = Self::find_project_dir(&self.start_dir);
        let project_path = project_dir
            .as_ref()
            .map(|dir| dir.join(PROJECT_DIR_NAME).join(CONFIG_FILE_NAME))
            .unwrap_or_else(|| self.start_dir.join(PROJECT_DIR_NAME).join(CONFIG_FILE_NAME));

        let mut merged = toml::Value::try_from(Config::default()).expect("defaults serialize");
        let mut sources: BTreeMap<String, SourceKind> = BTreeMap::new();
        for section in top_level_sections(&merged) {
            sources.insert(section.to_owned(), SourceKind::Default);
        }

        if global_path.is_file() {
            match Self::read_config_file(&global_path) {
                Ok(value) => {
                    Self::merge(&mut merged, value);
                    for section in top_level_sections(&merged) {
                        sources.insert(section.to_owned(), SourceKind::Global);
                    }
                }
                Err(issue) => issues.push(issue),
            }
        }
        if let Some(project) = &project_dir {
            let file = project.join(PROJECT_DIR_NAME).join(CONFIG_FILE_NAME);
            if file.is_file() {
                match Self::read_config_file(&file) {
                    Ok(value) => {
                        Self::merge(&mut merged, value);
                        for section in top_level_sections(&merged) {
                            sources.insert(section.to_owned(), SourceKind::Project);
                        }
                    }
                    Err(issue) => issues.push(issue),
                }
            }
        }

        let env_sections = apply_env_overrides(&mut merged, &self.env, &mut issues);
        for section in env_sections {
            sources.insert(section, SourceKind::Env);
        }
        for (path, value) in &self.cli_overrides {
            if let Err(issue) = set_path(&mut merged, path, value.clone()) {
                issues.push(issue);
            } else if let Some(root) = path.split('.').next() {
                sources.insert(root.to_owned(), SourceKind::Cli);
            }
        }

        let config: Config = match toml::Value::try_into(merged) {
            Ok(config) => config,
            Err(err) => {
                issues.push(Issue::new(
                    "config",
                    format!("invalid configuration value: {err}"),
                ));
                return Err(ConfigError::new(issues));
            }
        };

        if let Err(error) = config.validate() {
            issues.extend(error.into_issues());
        }
        if !issues.is_empty() {
            return Err(ConfigError::new(issues));
        }

        let instructions = project_dir
            .as_ref()
            .map(|dir| dir.join(PROJECT_DIR_NAME).join(INSTRUCTIONS_FILE_NAME))
            .and_then(|path| {
                std::fs::read(path)
                    .ok()
                    .filter(|bytes| !bytes.is_empty())
                    .and_then(|bytes| String::from_utf8(bytes).ok())
            });

        Ok(LoadedConfig {
            config,
            project_dir,
            instructions,
            global_path,
            project_path,
            state_dir: paths.state_dir,
            sources,
            warnings,
        })
    }
}

fn top_level_sections(value: &toml::Value) -> Vec<&str> {
    match value {
        toml::Value::Table(table) => table.keys().map(String::as_str).collect(),
        _ => Vec::new(),
    }
}

fn set_path(root: &mut toml::Value, dotted: &str, value: toml::Value) -> Result<(), Issue> {
    let parts: Vec<&str> = dotted.split('.').collect();
    let mut cursor = root;
    for part in &parts[..parts.len() - 1] {
        match cursor {
            toml::Value::Table(table) => {
                let entry = table
                    .entry(part.to_owned())
                    .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                if !entry.is_table() {
                    return Err(Issue::new(dotted, format!("`{part}` is not a table")));
                }
                cursor = entry;
            }
            _ => return Err(Issue::new(dotted, format!("`{part}` is not a table"))),
        }
    }
    let leaf = *parts.last().expect("at least one part");
    match cursor {
        toml::Value::Table(table) => {
            table.insert(leaf.to_string(), value);
            Ok(())
        }
        _ => Err(Issue::new(dotted, "parent is not a table")),
    }
}

fn env_string<'a>(env: &'a HashMap<String, String>, var: &'a str) -> Option<(&'a str, &'a str)> {
    env.get(var).map(|value| (var, value.as_str()))
}

/// Apply `SPACKLE_AGENT_*` overrides. Values keep their natural TOML types so
/// the final deserialization performs one consistent type check. Returns the
/// top-level sections actually touched.
fn apply_env_overrides(
    merged: &mut toml::Value,
    env: &HashMap<String, String>,
    issues: &mut Vec<Issue>,
) -> Vec<String> {
    let mut touched: Vec<String> = Vec::new();
    let touch = |touched: &mut Vec<String>, section: &str| {
        if !touched.iter().any(|existing| existing == section) {
            touched.push(section.to_owned());
        }
    };
    let push = |issues: &mut Vec<Issue>, var: &str, message: String| {
        issues.push(Issue::new(format!("env:{var}"), message));
    };
    #[allow(clippy::ptr_arg)]
    fn push_issue(issues: &mut Vec<Issue>, var: &str, message: &str) {
        issues.push(Issue::new(format!("env:{var}"), message.to_owned()));
    }

    if let Some((_, value)) = env_string(env, "SPACKLE_AGENT_BASE_URL") {
        if let Err(issue) = set_path(
            merged,
            "endpoint.base_url",
            toml::Value::String(value.to_owned()),
        ) {
            issues.push(issue);
        } else {
            touch(&mut touched, "endpoint");
        }
    }
    if let Some((_, value)) = env_string(env, "SPACKLE_AGENT_MODEL") {
        if let Err(issue) = set_path(
            merged,
            "endpoint.model",
            toml::Value::String(value.to_owned()),
        ) {
            issues.push(issue);
        } else {
            touch(&mut touched, "endpoint");
        }
    }
    if let Some((var, value)) = env_string(env, "SPACKLE_AGENT_ALLOW_PRIVATE_LAN") {
        match value.parse::<bool>() {
            Ok(flag) => {
                if let Err(issue) = set_path(
                    merged,
                    "endpoint.allow_private_lan",
                    toml::Value::Boolean(flag),
                ) {
                    issues.push(issue);
                } else {
                    touch(&mut touched, "endpoint");
                }
            }
            Err(_) => push_issue(issues, var, "expected true or false"),
        }
    }
    if let Some((var, value)) = env_string(env, "SPACKLE_AGENT_MODE") {
        match value {
            "attach" | "managed" => {
                if let Err(issue) = set_path(
                    merged,
                    "endpoint.mode",
                    toml::Value::String(value.to_owned()),
                ) {
                    issues.push(issue);
                } else {
                    touch(&mut touched, "endpoint");
                }
            }
            other => push(
                issues,
                var,
                format!("expected attach or managed, got `{other}`"),
            ),
        }
    }
    if let Some((_, value)) = env_string(env, "SPACKLE_AGENT_PROFILE") {
        if let Err(issue) = set_path(
            merged,
            "generation.active_profile",
            toml::Value::String(value.to_owned()),
        ) {
            issues.push(issue);
        } else {
            touch(&mut touched, "generation");
        }
    }
    if let Some((_, value)) = env_string(env, "SPACKLE_AGENT_WORKSPACE") {
        if let Err(issue) = set_path(
            merged,
            "agent.workspace",
            toml::Value::String(value.to_owned()),
        ) {
            issues.push(issue);
        } else {
            touch(&mut touched, "agent");
        }
    }
    if let Some((var, value)) = env_string(env, "SPACKLE_AGENT_MAX_STEPS") {
        match value.parse::<i64>() {
            Ok(number) => {
                if let Err(issue) =
                    set_path(merged, "agent.max_steps", toml::Value::Integer(number))
                {
                    issues.push(issue);
                } else {
                    touch(&mut touched, "agent");
                }
            }
            Err(_) => push_issue(issues, var, "expected a whole number"),
        }
    }
    if let Some((var, value)) = env_string(env, "SPACKLE_AGENT_MAX_TOOL_OUTPUT_BYTES") {
        match value.parse::<i64>() {
            Ok(number) => {
                if let Err(issue) = set_path(
                    merged,
                    "agent.max_tool_output_bytes",
                    toml::Value::Integer(number),
                ) {
                    issues.push(issue);
                } else {
                    touch(&mut touched, "agent");
                }
            }
            Err(_) => push_issue(issues, var, "expected a whole number"),
        }
    }
    touched
}

/// Atomically write configuration, keeping exactly one recoverable backup.
pub fn save_atomic(path: &Path, config: &Config) -> std::result::Result<(), std::io::Error> {
    let text =
        toml::to_string_pretty(config).map_err(|err| std::io::Error::other(err.to_string()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let backup = PathBuf::from(format!("{}.bak", path.display()));
    let temp = PathBuf::from(format!(
        "{}.tmp-{}.partial",
        path.display(),
        std::process::id()
    ));
    let write_and_fsync = |file: &Path| -> std::result::Result<(), std::io::Error> {
        use std::io::Write;
        let mut handle = std::fs::File::create(file)?;
        handle.write_all(text.as_bytes())?;
        handle.sync_all()?;
        Ok(())
    };
    // Backup of the previous file, when present (single recoverable copy).
    let _ = std::fs::remove_file(&backup);
    if path.is_file() {
        std::fs::copy(path, &backup)?;
    }
    write_and_fsync(&temp)?;
    let keep = temp.clone();
    std::fs::rename(temp, path).inspect_err(|_err| {
        let _ = std::fs::remove_file(&keep);
    })?;
    // Best-effort directory sync.
    if let Some(parent) = path.parent()
        && let Ok(dir) = std::fs::File::open(parent)
    {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// Platform configuration directory for this product.
#[must_use]
pub fn platform_config_dir() -> PathBuf {
    Loader::standard_paths().config_dir
}

/// Platform state directory for this product.
#[must_use]
pub fn platform_state_dir() -> PathBuf {
    Loader::standard_paths().state_dir
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn tempdir() -> TempDir {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("spackle-cfg-test-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }

    fn loader(
        config_dir: &std::path::Path,
        state_dir: &std::path::Path,
        start: &std::path::Path,
    ) -> Loader {
        Loader {
            start_dir: start.to_path_buf(),
            env: HashMap::new(),
            cli_overrides: BTreeMap::new(),
            paths: Some(LoaderPaths {
                config_dir: config_dir.to_path_buf(),
                state_dir: state_dir.to_path_buf(),
            }),
        }
    }

    #[test]
    fn defaults_load_and_validate() {
        let temp = tempdir();
        let loaded = loader(temp.path(), temp.path(), temp.path())
            .load()
            .expect("defaults validate");
        assert_eq!(loaded.config.version, 1);
        assert_eq!(loaded.config.endpoint.base_url, "http://127.0.0.1:8080");
        assert!(loaded.project_dir.is_none());
        assert!(loaded.warnings.is_empty());
    }

    #[test]
    fn global_file_is_loaded() {
        let temp = tempdir();
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(&config_dir).expect("dir");
        std::fs::write(
            config_dir.join("config.toml"),
            "version = 1\n\n[endpoint]\nmodel = \"custom-model\"\n",
        )
        .expect("write");
        let loaded = loader(&config_dir, temp.path(), temp.path())
            .load()
            .expect("loads");
        assert_eq!(loaded.config.endpoint.model, "custom-model");
        assert!(
            loaded
                .sources
                .get("endpoint")
                .is_some_and(|source| *source == SourceKind::Global)
        );
    }

    #[test]
    fn project_overrides_global() {
        let temp = tempdir();
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(&config_dir).expect("dir");
        std::fs::write(
            config_dir.join("config.toml"),
            "version = 1\n\n[endpoint]\nmodel = \"global-model\"\nbase_url = \"http://127.0.0.1:9999\"\n",
        )
        .expect("write");
        let project = temp.path().join("project");
        let project_spackle = project.join(".spackle");
        std::fs::create_dir_all(&project_spackle).expect("dir");
        std::fs::write(
            project_spackle.join("config.toml"),
            "version = 1\n\n[endpoint]\nmodel = \"project-model\"\n",
        )
        .expect("write");
        std::fs::write(project_spackle.join("instructions.md"), "be careful").expect("write");
        let loaded = loader(&config_dir, temp.path(), &project)
            .load()
            .expect("loads");
        assert_eq!(loaded.config.endpoint.model, "project-model");
        assert_eq!(
            loaded.config.endpoint.base_url, "http://127.0.0.1:9999",
            "untouched project keys keep the global value"
        );
        assert_eq!(
            loaded.instructions.as_deref(),
            Some("be careful"),
            "project instructions are loaded"
        );
        assert_eq!(loaded.project_dir.as_deref(), Some(project.as_path()));
    }

    #[test]
    fn project_discovery_walks_upward() {
        let temp = tempdir();
        let project = temp.path().join("project");
        std::fs::create_dir_all(project.join(".spackle")).expect("dir");
        std::fs::write(
            project.join(".spackle").join("config.toml"),
            "version = 1\n\n[agent]\nmax_steps = 7\n",
        )
        .expect("write");
        let nested = project.join("a").join("b");
        std::fs::create_dir_all(&nested).expect("dir");
        let loaded = loader(temp.path(), temp.path(), &nested)
            .load()
            .expect("loads");
        assert_eq!(loaded.config.agent.max_steps, 7);
        assert_eq!(loaded.project_dir.as_deref(), Some(project.as_path()));
    }

    #[test]
    fn env_overrides_project() {
        let temp = tempdir();
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(&config_dir).expect("dir");
        std::fs::write(
            config_dir.join("config.toml"),
            "version = 1\n\n[endpoint]\nmodel = \"global-model\"\n",
        )
        .expect("write");
        let project = temp.path().join("project");
        std::fs::create_dir_all(project.join(".spackle")).expect("dir");
        std::fs::write(
            project.join(".spackle").join("config.toml"),
            "version = 1\n\n[endpoint]\nmodel = \"project-model\"\n",
        )
        .expect("write");
        let mut env = HashMap::new();
        env.insert("SPACKLE_AGENT_MODEL".to_owned(), "env-model".to_owned());
        let loaded = Loader {
            start_dir: project.clone(),
            env,
            paths: Some(LoaderPaths {
                config_dir: config_dir.clone(),
                state_dir: temp.path().to_path_buf(),
            }),
            ..Loader::default()
        }
        .load()
        .expect("loads");
        assert_eq!(loaded.config.endpoint.model, "env-model");
    }

    #[test]
    fn cli_overrides_env() {
        let temp = tempdir();
        let mut env = HashMap::new();
        env.insert("SPACKLE_AGENT_MODEL".to_owned(), "env-model".to_owned());
        let mut cli = BTreeMap::new();
        cli.insert(
            "endpoint.model".to_owned(),
            toml::Value::String("cli-model".to_owned()),
        );
        let loaded = Loader {
            start_dir: temp.path().to_path_buf(),
            env,
            cli_overrides: cli,
            paths: Some(LoaderPaths {
                config_dir: temp.path().join("config"),
                state_dir: temp.path().to_path_buf(),
            }),
        }
        .load()
        .expect("loads");
        assert_eq!(loaded.config.endpoint.model, "cli-model");
        assert_eq!(
            loaded.sources.get("endpoint").copied(),
            Some(SourceKind::Cli)
        );
    }

    #[test]
    fn unknown_keys_are_rejected_with_names() {
        let temp = tempdir();
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(&config_dir).expect("dir");
        std::fs::write(
            config_dir.join("config.toml"),
            "version = 1\n\n[endpoint]\nmodel = \"m\"\nbogus_key = true\n",
        )
        .expect("write");
        let error = loader(&config_dir, temp.path(), temp.path())
            .load()
            .expect_err("unknown key");
        assert!(
            error.to_string().contains("bogus_key"),
            "unknown key name must be reported: {error}"
        );
    }

    #[test]
    fn invalid_types_are_rejected() {
        let temp = tempdir();
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(&config_dir).expect("dir");
        std::fs::write(
            config_dir.join("config.toml"),
            "version = 1\n\n[endpoint]\nmodel = 42\n",
        )
        .expect("write");
        let error = loader(&config_dir, temp.path(), temp.path())
            .load()
            .expect_err("type mismatch");
        assert!(error.to_string().contains("model"));
    }

    #[test]
    fn env_parse_failures_report_the_variable() {
        let temp = tempdir();
        let mut env = HashMap::new();
        env.insert(
            "SPACKLE_AGENT_MAX_STEPS".to_owned(),
            "not-a-number".to_owned(),
        );
        let error = Loader {
            start_dir: temp.path().to_path_buf(),
            env,
            paths: Some(LoaderPaths {
                config_dir: temp.path().join("config"),
                state_dir: temp.path().to_path_buf(),
            }),
            ..Loader::default()
        }
        .load()
        .expect_err("bad env int");
        assert!(error.to_string().contains("SPACKLE_AGENT_MAX_STEPS"));
    }

    #[test]
    fn llamacpp_arg_variables_warn() {
        let temp = tempdir();
        let mut env = HashMap::new();
        env.insert("LLAMA_ARG_N_GPU_LAYERS".to_owned(), "99".to_owned());
        let loaded = Loader {
            start_dir: temp.path().to_path_buf(),
            env,
            paths: Some(LoaderPaths {
                config_dir: temp.path().join("config"),
                state_dir: temp.path().to_path_buf(),
            }),
            ..Loader::default()
        }
        .load()
        .expect("loads");
        assert!(
            loaded
                .warnings
                .iter()
                .any(|warning| warning.contains("LLAMA_ARG_N_GPU_LAYERS")),
            "warning list: {:?}",
            loaded.warnings
        );
    }

    #[test]
    fn save_is_atomic_and_keeps_one_backup() {
        let temp = tempdir();
        let path = temp.path().join("nested").join("config.toml");
        let first = Config {
            endpoint: EndpointConfig {
                model: "first".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        save_atomic(&path, &first).expect("first write");
        let second = Config {
            endpoint: EndpointConfig {
                model: "second".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        save_atomic(&path, &second).expect("second write");
        let text = std::fs::read_to_string(&path).expect("read");
        assert!(text.contains("second"));
        let backup = path.with_extension("toml.bak");
        let backup_text = std::fs::read_to_string(&backup).expect("backup exists");
        assert!(
            backup_text.contains("first"),
            "backup must hold the previous revision"
        );
        // No stray temp files.
        let leftovers: Vec<_> = std::fs::read_dir(temp.path())
            .expect("read dir")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "leftovers: {leftovers:?}");
        // Round-trips.
        let parsed: Config = toml::from_str(&text).expect("parse");
        assert_eq!(parsed, second);
    }

    #[test]
    fn validation_collects_multiple_issues() {
        let mut config = Config::default();
        config.agent.max_steps = 0;
        config.generation.active_profile = "missing".to_owned();
        let error = config.validate().expect_err("must fail");
        assert_eq!(error.issues().len(), 2);
    }

    #[test]
    fn example_config_file_shape_is_supported() {
        // The repository's example config must parse into the current schema.
        let example = std::fs::read_to_string("../../config/example.toml").expect("example");
        let value: toml::Value = toml::from_str(&example).expect("parse example");
        let mut config: Config = toml::Value::try_into(value).expect("schema");
        // Example points at a managed-style setup but attach mode; validate
        // the attach path only.
        config.endpoint.mode = endpoint::EndpointMode::Attach;
        config.validate().expect("example validates");
        assert_eq!(
            config.generation.active_profile, "balanced",
            "active profile"
        );
    }
}
