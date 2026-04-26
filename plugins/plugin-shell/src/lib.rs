use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn};

// `Duration`, `Instant`, `Command`, and `timeout` are only used by the
// `unsafe-shell`-gated execution paths. Cfg-gating their imports keeps
// default builds free of `unused_imports` warnings.
#[cfg(feature = "unsafe-shell")]
use std::time::{Duration, Instant};
#[cfg(feature = "unsafe-shell")]
use tokio::process::Command;
#[cfg(feature = "unsafe-shell")]
use tokio::time::timeout;

use junoclaw_core::error::{JunoClawError, Result};
use junoclaw_core::plugin::{Plugin, PluginCapability, PluginContext};
use junoclaw_core::types::{Task, TaskResult, TokenUsage, ToolCallRecord};

// ──────────────────────────────────────────────
// Output type returned by all shell executions
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub timed_out: bool,
}

impl ShellOutput {
    pub fn success(&self) -> bool {
        self.exit_code == 0 && !self.timed_out
    }

    pub fn combined(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            format!("[stderr] {}", self.stderr)
        } else {
            format!("{}\n[stderr] {}", self.stdout, self.stderr)
        }
    }
}

// ────────────────────────────────────────────
// Compile-time gate (post-Ffern security release, April 2026)
// ────────────────────────────────────────────
//
// Shell + Python execution is gated behind the `unsafe-shell` Cargo
// feature. When the feature is OFF (default), the bytes for the
// unsafe execution paths are not compiled into the binary. The public
// `run_command` / `run_python` methods compile to a stub that returns
// an explicit error pointing at SECURITY.md.
//
// When the feature is ON, three independent layers gate execution:
//   1. The `unsafe-shell` flag itself.
//   2. The runtime `sandbox_mode` kill-switch on `ShellPlugin`.
//   3. The `allowed_commands` allowlist (default empty — fail-closed).
//
// The substring blocklist (`BLOCKED_PATTERNS`) that earlier versions
// used has been removed entirely. Substring blocklists for shell
// inputs are theatre — every interesting bypass had been documented
// in the security literature for two decades. The replacement is a
// strict allowlist applied to the parsed first token of the command,
// with the shell wrapper (`sh -c` / `cmd /C`) dropped so there is no
// metacharacter expansion to bypass in the first place.
//
// Output is capped at 1 MiB per stream; over-cap output is truncated
// with a warn! log.

#[cfg(feature = "unsafe-shell")]
const OUTPUT_CAP_BYTES: usize = 1024 * 1024; // 1 MiB

// ──────────────────────────────────────────────
// Plugin struct
// ──────────────────────────────────────────────

pub struct ShellPlugin {
    pub sandbox_mode: bool,
    pub allowed_commands: Vec<String>,
    pub default_timeout_secs: u64,
    pub workspace_dir: PathBuf,
}

impl ShellPlugin {
    /// Construct a new ShellPlugin.
    ///
    /// `sandbox_mode` is the runtime kill-switch:
    /// - `true`  → every execution request is refused, regardless of
    ///   the `unsafe-shell` feature or the allowlist. Use this as a
    ///   runtime "halt" primitive in incident response.
    /// - `false` → execution may proceed *if* the `unsafe-shell` Cargo
    ///   feature is compiled in *and* the parsed first token is in
    ///   `allowed_commands`.
    ///
    /// `allowed_commands` defaults to the empty `Vec` — fail-closed.
    /// The previously-shipped default of `[python, python3, echo, ls,
    /// dir, pwd, cat, type]` was removed in the post-Ffern security
    /// release: silent defaults that grant execution are exactly the
    /// kind of footgun the audit named.
    pub fn new(sandbox_mode: bool) -> Self {
        Self {
            sandbox_mode,
            allowed_commands: Vec::new(),
            default_timeout_secs: 30,
            workspace_dir: std::env::temp_dir().join("junoclaw-workspace"),
        }
    }

    /// Run an allowlisted shell command — *no shell wrapper*.
    ///
    /// The command string is parsed with `shell-words` (POSIX-style
    /// quoting). The first token is treated as the executable name and
    /// must appear in `allowed_commands`. Subsequent tokens are passed
    /// as args to a direct executable spawn — *not* via `sh -c` or
    /// `cmd /C`. This eliminates the shell-injection class entirely;
    /// metacharacters in the command string have no special meaning to
    /// the OS because there is no shell.
    ///
    /// Refused (with explicit error) if any of:
    /// - the `unsafe-shell` Cargo feature was not compiled in;
    /// - `sandbox_mode` is true (runtime kill-switch armed);
    /// - the parsed first token is not in `allowed_commands`;
    /// - shell-words fails to parse the input.
    pub async fn run_command(
        &self,
        command: &str,
        cwd: Option<&Path>,
        timeout_secs: Option<u64>,
    ) -> Result<ShellOutput> {
        #[cfg(not(feature = "unsafe-shell"))]
        {
            let _ = (command, cwd, timeout_secs); // silence unused
            warn!("run_command refused: `unsafe-shell` Cargo feature not compiled in");
            Err(JunoClawError::TaskExecution(
                "shell execution disabled in this build. \
                 Recompile with --features unsafe-shell to enable. \
                 See SECURITY.md.".to_string(),
            ))
        }

        #[cfg(feature = "unsafe-shell")]
        {
            run_command_unsafe(self, command, cwd, timeout_secs).await
        }
    }

    /// Run a Python script with hardened spawn flags — *no temp file*.
    ///
    /// The script is piped to the interpreter via stdin
    /// (`python -E -I -S -B -`). No on-disk artefact is written, so
    /// there is no file-on-disk race window for a concurrent process
    /// to read or modify the script before it executes.
    ///
    /// The chosen Python binary (`python3` preferred over `python`)
    /// must be in `allowed_commands`. The child runs with a cleared
    /// environment (only `PATH` and `LANG=C.UTF-8` re-injected),
    /// per-call temp CWD via `tempfile::TempDir`, and `kill_on_drop
    /// (true)` so timeouts do not orphan processes.
    ///
    /// Refused with explicit error in the same conditions as
    /// `run_command`, plus when neither `python` nor `python3` is in
    /// `allowed_commands`.
    pub async fn run_python(
        &self,
        script: &str,
        cwd: Option<&Path>,
        timeout_secs: Option<u64>,
    ) -> Result<ShellOutput> {
        #[cfg(not(feature = "unsafe-shell"))]
        {
            let _ = (script, cwd, timeout_secs);
            warn!("run_python refused: `unsafe-shell` Cargo feature not compiled in");
            Err(JunoClawError::TaskExecution(
                "Python execution disabled in this build. \
                 Recompile with --features unsafe-shell to enable. \
                 See SECURITY.md.".to_string(),
            ))
        }

        #[cfg(feature = "unsafe-shell")]
        {
            run_python_unsafe(self, script, cwd, timeout_secs).await
        }
    }
}

// ────────────────────────────────────────────
// Unsafe execution paths (only compiled with `unsafe-shell`)
// ────────────────────────────────────────────
//
// These free functions hold the actual execution logic. They are
// only compiled when the `unsafe-shell` feature is enabled. The
// public `ShellPlugin::run_command` / `run_python` methods dispatch
// here when the feature is on, or return an explicit error otherwise.

#[cfg(feature = "unsafe-shell")]
async fn run_command_unsafe(
    plugin: &ShellPlugin,
    command: &str,
    cwd: Option<&Path>,
    timeout_secs: Option<u64>,
) -> Result<ShellOutput> {
    use std::process::Stdio;

    if plugin.sandbox_mode {
        warn!("run_command refused: sandbox_mode=true (runtime kill-switch armed)");
        return Err(JunoClawError::TaskExecution(
            "shell execution refused: sandbox_mode is armed. \
             Set sandbox_mode=false in plugin config to enable.".to_string(),
        ));
    }

    let tokens = shell_words::split(command).map_err(|e| {
        JunoClawError::TaskExecution(format!("could not parse command: {}", e))
    })?;
    let (program, args) = tokens.split_first().ok_or_else(|| {
        JunoClawError::TaskExecution("empty command".to_string())
    })?;

    if !plugin.allowed_commands.iter().any(|c| c == program) {
        warn!(
            "run_command refused: '{}' not in allowed_commands ({:?})",
            program, plugin.allowed_commands
        );
        return Err(JunoClawError::TaskExecution(format!(
            "BLOCKED: '{}' is not in allowed_commands. \
             Configure allowed_commands in plugin config to opt in.",
            program
        )));
    }

    // Per-call temp CWD; auto-deleted on drop at end of function.
    let temp = tempfile::tempdir().map_err(|e| {
        JunoClawError::TaskExecution(format!("could not create temp cwd: {}", e))
    })?;
    let work_dir: PathBuf = match cwd {
        Some(p) => p.to_path_buf(),
        None => temp.path().to_path_buf(),
    };
    if cwd.is_some() {
        if let Err(e) = tokio::fs::create_dir_all(&work_dir).await {
            warn!("could not create cwd {:?}: {}", work_dir, e);
        }
    }

    let secs = timeout_secs.unwrap_or(plugin.default_timeout_secs);
    let start = Instant::now();

    info!(
        "shell exec (allowlisted): {:?} args={:?} cwd={:?} timeout={}s",
        program, args, work_dir, secs
    );

    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(&work_dir)
        .env_clear()
        .env("PATH", safe_path_env())
        .env("LANG", "C.UTF-8")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let result = timeout(Duration::from_secs(secs), cmd.output()).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    // Hold `temp` alive across the spawn; explicit drop after the wait.
    drop(temp);

    match result {
        Ok(Ok(output)) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = cap_string(&output.stdout, OUTPUT_CAP_BYTES);
            let stderr = cap_string(&output.stderr, OUTPUT_CAP_BYTES);
            info!("shell exit={} duration={}ms", exit_code, duration_ms);
            Ok(ShellOutput {
                stdout,
                stderr,
                exit_code,
                duration_ms,
                timed_out: false,
            })
        }
        Ok(Err(e)) => Err(JunoClawError::TaskExecution(format!(
            "process error: {}",
            e
        ))),
        Err(_) => {
            warn!(
                "command timed out after {}s; child killed via kill_on_drop",
                secs
            );
            Ok(ShellOutput {
                stdout: String::new(),
                stderr: format!("Command timed out after {}s", secs),
                exit_code: -1,
                duration_ms,
                timed_out: true,
            })
        }
    }
}

#[cfg(feature = "unsafe-shell")]
async fn run_python_unsafe(
    plugin: &ShellPlugin,
    script: &str,
    cwd: Option<&Path>,
    timeout_secs: Option<u64>,
) -> Result<ShellOutput> {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;

    if plugin.sandbox_mode {
        warn!("run_python refused: sandbox_mode=true");
        return Err(JunoClawError::TaskExecution(
            "Python execution refused: sandbox_mode is armed.".to_string(),
        ));
    }

    // Choose binary by allowlist membership. python3 preferred over python.
    let python_bin = if plugin.allowed_commands.iter().any(|c| c == "python3") {
        "python3"
    } else if plugin.allowed_commands.iter().any(|c| c == "python") {
        "python"
    } else {
        warn!(
            "run_python refused: neither 'python' nor 'python3' in allowed_commands ({:?})",
            plugin.allowed_commands
        );
        return Err(JunoClawError::TaskExecution(
            "BLOCKED: 'python' or 'python3' must be in allowed_commands. \
             Add either to plugin config to opt in to Python execution.".to_string(),
        ));
    };

    let temp = tempfile::tempdir().map_err(|e| {
        JunoClawError::TaskExecution(format!("could not create temp cwd: {}", e))
    })?;
    let work_dir: PathBuf = match cwd {
        Some(p) => p.to_path_buf(),
        None => temp.path().to_path_buf(),
    };
    if cwd.is_some() {
        if let Err(e) = tokio::fs::create_dir_all(&work_dir).await {
            warn!("could not create cwd {:?}: {}", work_dir, e);
        }
    }

    let secs = timeout_secs.unwrap_or(plugin.default_timeout_secs);
    let start = Instant::now();

    info!(
        "python exec (allowlisted, stdin-piped): bin={} cwd={:?} timeout={}s script_len={}",
        python_bin,
        work_dir,
        secs,
        script.len()
    );

    // python -E -I -S -B - reads script from stdin in maximum-isolated mode.
    //   -E: ignore PYTHON* env vars
    //   -I: isolated mode (implies -E and -s)
    //   -S: don't run site initialisation
    //   -B: don't write .pyc bytecode
    //   -:  read program from stdin
    let mut cmd = Command::new(python_bin);
    cmd.args(["-E", "-I", "-S", "-B", "-"])
        .current_dir(&work_dir)
        .env_clear()
        .env("PATH", safe_path_env())
        .env("LANG", "C.UTF-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        JunoClawError::TaskExecution(format!("python spawn failed: {}", e))
    })?;

    if let Some(mut stdin_handle) = child.stdin.take() {
        if let Err(e) = stdin_handle.write_all(script.as_bytes()).await {
            warn!("write to python stdin failed: {}", e);
        }
        // dropping stdin_handle at end of scope closes the pipe → Python sees EOF
    }

    let result = timeout(Duration::from_secs(secs), child.wait_with_output()).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    drop(temp);

    match result {
        Ok(Ok(output)) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = cap_string(&output.stdout, OUTPUT_CAP_BYTES);
            let stderr = cap_string(&output.stderr, OUTPUT_CAP_BYTES);
            info!("python exit={} duration={}ms", exit_code, duration_ms);
            Ok(ShellOutput {
                stdout,
                stderr,
                exit_code,
                duration_ms,
                timed_out: false,
            })
        }
        Ok(Err(e)) => Err(JunoClawError::TaskExecution(format!(
            "python process error: {}",
            e
        ))),
        Err(_) => {
            warn!(
                "python timed out after {}s; child killed via kill_on_drop",
                secs
            );
            Ok(ShellOutput {
                stdout: String::new(),
                stderr: format!("Python timed out after {}s", secs),
                exit_code: -1,
                duration_ms,
                timed_out: true,
            })
        }
    }
}

/// Read up to `cap` bytes from a buffer into a `String`, lossy-decoding
/// UTF-8. If the source exceeds the cap, append a `[... truncated ...]`
/// marker and emit a warn! log so the operator sees the truncation.
#[cfg(feature = "unsafe-shell")]
fn cap_string(bytes: &[u8], cap: usize) -> String {
    if bytes.len() > cap {
        warn!("output truncated at {} bytes (source {} bytes)", cap, bytes.len());
        let truncated = &bytes[..cap];
        format!(
            "{}\n[... truncated at {} bytes; source was {} bytes ...]",
            String::from_utf8_lossy(truncated),
            cap,
            bytes.len()
        )
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Minimal, safe `PATH` for spawned children. Inherits the parent
/// process's `PATH` if set; otherwise falls back to OS-default search
/// paths. Operators wanting a stricter `PATH` can wrap the JunoClaw
/// binary in a systemd unit / Docker container with a controlled
/// environment, or override at process-start time.
#[cfg(feature = "unsafe-shell")]
fn safe_path_env() -> String {
    std::env::var("PATH").unwrap_or_else(|_| {
        if cfg!(target_os = "windows") {
            "C:\\Windows\\System32;C:\\Windows".to_string()
        } else {
            "/usr/local/bin:/usr/bin:/bin".to_string()
        }
    })
}

// ──────────────────────────────────────────────
// Plugin trait impl (task-based interface)
// ──────────────────────────────────────────────

#[async_trait]
impl Plugin for ShellPlugin {
    fn name(&self) -> &str { "plugin-shell" }
    fn description(&self) -> &str {
        #[cfg(feature = "unsafe-shell")]
        { "Shell + Python execution for agents (allowlisted, stdin-piped, NOT sandboxed). Use only behind an external sandbox — see SECURITY.md." }
        #[cfg(not(feature = "unsafe-shell"))]
        { "Shell + Python execution: DISABLED in this build. Recompile with --features unsafe-shell to enable." }
    }
    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }
    fn capabilities(&self) -> Vec<PluginCapability> { vec![PluginCapability::ShellExecution] }
    fn is_available(&self) -> bool { true }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "sandbox_mode": {
                    "type": "boolean",
                    "default": false,
                    "description": "Runtime kill-switch. true = refuse all execution regardless of feature flag or allowlist."
                },
                "allowed_commands": {
                    "type": "array",
                    "items": { "type": "string" },
                    "default": [],
                    "description": "Allowlisted executable names. Default empty = nothing runs. To enable Python, include 'python' or 'python3'."
                },
                "default_timeout_secs": {
                    "type": "integer",
                    "default": 30,
                    "description": "Per-call timeout in seconds. Child process is killed via kill_on_drop on expiry."
                }
            }
        })
    }

    async fn initialize(&mut self, config: Value) -> Result<()> {
        // sandbox_mode default flipped from true → false in the post-Ffern
        // security release. The Cargo feature flag is the primary gate;
        // sandbox_mode is the runtime kill-switch (default false, flip to
        // true to halt execution without recompile).
        self.sandbox_mode = config.get("sandbox_mode")
            .and_then(|v| v.as_bool()).unwrap_or(false);
        if let Some(cmds) = config.get("allowed_commands").and_then(|v| v.as_array()) {
            self.allowed_commands = cmds
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        self.default_timeout_secs = config.get("default_timeout_secs")
            .and_then(|v| v.as_u64()).unwrap_or(30);
        let feature_state = if cfg!(feature = "unsafe-shell") { "compiled-in" } else { "NOT compiled-in" };
        info!(
            "Shell plugin initialized (unsafe-shell={}, sandbox_mode={}, allowed_commands={:?}, timeout={}s)",
            feature_state, self.sandbox_mode, self.allowed_commands, self.default_timeout_secs
        );
        Ok(())
    }

    async fn execute(&self, task: &Task, _context: &PluginContext) -> Result<TaskResult> {
        let input: Value = serde_json::from_str(&task.input)
            .unwrap_or_else(|_| serde_json::json!({ "command": task.input }));

        let output = if let Some(script) = input.get("python_script").and_then(|v| v.as_str()) {
            self.run_python(script, None, None).await?
        } else if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
            self.run_command(cmd, None, None).await?
        } else {
            return Err(JunoClawError::TaskExecution(
                "Input must have 'command' or 'python_script' field".to_string(),
            ));
        };

        let output_json = serde_json::to_string(&output)
            .unwrap_or_else(|_| output.combined());

        Ok(TaskResult {
            output: output.combined(),
            output_hash: sha256_short(&output_json),
            tool_calls: vec![ToolCallRecord {
                tool_name: "shell".to_string(),
                input: input,
                output: serde_json::json!({ "stdout": output.stdout, "stderr": output.stderr, "exit_code": output.exit_code }),
                duration_ms: output.duration_ms,
                approved: true,
            }],
            tokens_used: TokenUsage::default(),
        })
    }

    async fn shutdown(&self) -> Result<()> { Ok(()) }
}

fn sha256_short(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

// ──────────────────────────────────────────────
// Tests — regression coverage for the post-Ffern security release
// ──────────────────────────────────────────────
//
// Run all tests:
//   cargo test -p plugin-shell                       (feature off)
//   cargo test -p plugin-shell --features unsafe-shell  (feature on)
//
// Both states must be green for a release tag.

#[cfg(test)]
mod tests {
    use super::*;

    // ──────────────────────────────────────────
    // Always-compiled: constructor and helpers
    // ──────────────────────────────────────────

    #[test]
    fn new_starts_with_empty_allowlist() {
        let p = ShellPlugin::new(false);
        assert!(
            p.allowed_commands.is_empty(),
            "default allowed_commands must be empty (fail-closed). \
             Was: {:?}",
            p.allowed_commands
        );
    }

    #[test]
    fn new_default_timeout_is_30s() {
        let p = ShellPlugin::new(false);
        assert_eq!(p.default_timeout_secs, 30);
    }

    #[test]
    fn new_sandbox_mode_round_trips_constructor_arg() {
        let p_off = ShellPlugin::new(false);
        let p_on = ShellPlugin::new(true);
        assert!(!p_off.sandbox_mode);
        assert!(p_on.sandbox_mode);
    }

    fn fixture_output(stdout: &str, stderr: &str, exit_code: i32, timed_out: bool) -> ShellOutput {
        ShellOutput {
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            exit_code,
            duration_ms: 0,
            timed_out,
        }
    }

    #[test]
    fn output_success_only_when_zero_exit_and_not_timed_out() {
        assert!(fixture_output("hi", "", 0, false).success());
        assert!(!fixture_output("hi", "", 1, false).success());
        assert!(!fixture_output("", "", 0, true).success());
        assert!(!fixture_output("", "", 1, true).success());
    }

    #[test]
    fn output_combined_emits_stderr_marker_when_present() {
        assert_eq!(fixture_output("hi", "", 0, false).combined(), "hi");
        assert_eq!(
            fixture_output("", "boom", 1, false).combined(),
            "[stderr] boom"
        );
        let combined = fixture_output("ok", "warn", 0, false).combined();
        assert!(combined.contains("ok"));
        assert!(combined.contains("[stderr] warn"));
    }

    // ──────────────────────────────────────────
    // Feature OFF: stub returns explicit error pointing at SECURITY.md
    // ──────────────────────────────────────────

    #[cfg(not(feature = "unsafe-shell"))]
    mod feature_off {
        use super::*;

        #[tokio::test]
        async fn run_command_refuses_when_feature_off() {
            let p = ShellPlugin::new(false);
            let err = p.run_command("anything", None, None).await.unwrap_err();
            let msg = format!("{}", err);
            assert!(
                msg.contains("disabled in this build"),
                "expected 'disabled in this build' marker; got: {}",
                msg
            );
            assert!(
                msg.contains("--features unsafe-shell"),
                "msg should hint at the feature flag; got: {}",
                msg
            );
            assert!(
                msg.contains("SECURITY.md"),
                "msg should reference SECURITY.md; got: {}",
                msg
            );
        }

        #[tokio::test]
        async fn run_python_refuses_when_feature_off() {
            let p = ShellPlugin::new(false);
            let err = p.run_python("print('hi')", None, None).await.unwrap_err();
            let msg = format!("{}", err);
            assert!(
                msg.contains("disabled in this build"),
                "msg: {}",
                msg
            );
            assert!(
                msg.contains("--features unsafe-shell"),
                "msg: {}",
                msg
            );
        }

        #[tokio::test]
        async fn feature_off_error_does_not_leak_command_to_caller() {
            // The stub should not echo the input command back in the error,
            // because some inputs may contain sensitive data.
            let p = ShellPlugin::new(false);
            let secret = "echo SECRETMARKER12345";
            let err = p.run_command(secret, None, None).await.unwrap_err();
            let msg = format!("{}", err);
            assert!(
                !msg.contains("SECRETMARKER12345"),
                "feature-off stub should not echo input back; got: {}",
                msg
            );
        }
    }

    // ──────────────────────────────────────────
    // Feature ON: allowlist + sandbox_mode + parse-failure refusal paths
    // ──────────────────────────────────────────

    #[cfg(feature = "unsafe-shell")]
    mod feature_on {
        use super::*;

        #[tokio::test]
        async fn run_command_refuses_with_empty_allowlist() {
            let p = ShellPlugin::new(false); // sandbox_mode off
            assert!(p.allowed_commands.is_empty(), "precondition");
            let err = p.run_command("echo hi", None, None).await.unwrap_err();
            let msg = format!("{}", err);
            assert!(msg.contains("BLOCKED"), "msg: {}", msg);
            assert!(
                msg.contains("not in allowed_commands"),
                "msg: {}",
                msg
            );
            assert!(
                msg.contains("'echo'"),
                "msg should name the rejected binary; got: {}",
                msg
            );
        }

        #[tokio::test]
        async fn run_command_refuses_when_sandbox_mode_armed_even_with_allowlist() {
            let mut p = ShellPlugin::new(true); // sandbox_mode armed
            // Even though the allowlist would otherwise permit it:
            p.allowed_commands.push("echo".to_string());
            let err = p.run_command("echo hi", None, None).await.unwrap_err();
            let msg = format!("{}", err);
            assert!(
                msg.contains("sandbox_mode"),
                "sandbox-mode kill-switch should be cited; got: {}",
                msg
            );
            assert!(
                msg.contains("armed"),
                "msg should say 'armed'; got: {}",
                msg
            );
        }

        #[tokio::test]
        async fn run_command_rejects_empty_command_string() {
            let mut p = ShellPlugin::new(false);
            p.allowed_commands.push("echo".to_string()); // shouldn't matter
            let err = p.run_command("", None, None).await.unwrap_err();
            let msg = format!("{}", err);
            assert!(msg.contains("empty command"), "msg: {}", msg);
        }

        #[tokio::test]
        async fn run_command_surfaces_shell_words_parse_failure() {
            let mut p = ShellPlugin::new(false);
            p.allowed_commands.push("echo".to_string());
            // Unmatched double quote — shell-words rejects.
            let err = p
                .run_command("echo \"unterminated", None, None)
                .await
                .unwrap_err();
            let msg = format!("{}", err);
            assert!(
                msg.contains("could not parse command"),
                "msg: {}",
                msg
            );
        }

        #[tokio::test]
        async fn run_python_refuses_with_empty_allowlist() {
            let p = ShellPlugin::new(false);
            assert!(p.allowed_commands.is_empty(), "precondition");
            let err = p
                .run_python("print('hi')", None, None)
                .await
                .unwrap_err();
            let msg = format!("{}", err);
            assert!(msg.contains("BLOCKED"), "msg: {}", msg);
            assert!(
                msg.contains("python") || msg.contains("python3"),
                "msg should name python or python3; got: {}",
                msg
            );
            assert!(
                msg.contains("allowed_commands"),
                "msg: {}",
                msg
            );
        }

        #[tokio::test]
        async fn run_python_refuses_when_sandbox_mode_armed() {
            let mut p = ShellPlugin::new(true);
            p.allowed_commands.push("python3".to_string());
            let err = p
                .run_python("print('hi')", None, None)
                .await
                .unwrap_err();
            let msg = format!("{}", err);
            assert!(
                msg.contains("sandbox_mode") || msg.contains("armed"),
                "msg: {}",
                msg
            );
        }

        /// Probe for a *really* working Python interpreter by going through
        /// the real `run_python` code path. The probe stdin-pipes a marker
        /// string and only returns Some(bin) if the marker round-trips to
        /// stdout. This correctly rejects the Windows Microsoft Store
        /// "Python App Execution Alias" stub, which falsely passes a
        /// `python --version` smoke test but exits 9009 with the
        /// "Python was not found" message on real workloads.
        async fn find_working_python() -> Option<String> {
            const PROBE_MARKER: &str = "JC_PROBE_a4f9b1d2";
            for bin in ["python3", "python"] {
                let mut p = ShellPlugin::new(false);
                p.allowed_commands.push(bin.to_string());
                p.default_timeout_secs = 5;
                let script = format!("print('{}')", PROBE_MARKER);
                match p.run_python(&script, None, Some(5)).await {
                    Ok(out) if out.success() && out.stdout.contains(PROBE_MARKER) => {
                        return Some(bin.to_string());
                    }
                    _ => continue,
                }
            }
            None
        }

        // Happy-path test — only runs when a real Python interpreter is
        // available end-to-end through the run_python code path.
        #[tokio::test]
        async fn run_python_happy_path_when_python_available() {
            let bin = match find_working_python().await {
                Some(b) => b,
                None => {
                    eprintln!(
                        "skipping run_python_happy_path: no working python interpreter \
                         (note: Windows Store App Execution Aliases do NOT count)"
                    );
                    return;
                }
            };

            let mut p = ShellPlugin::new(false);
            p.allowed_commands.push(bin.clone());
            p.default_timeout_secs = 10;

            let out = p
                .run_python("print('hello-from-stdin')", None, Some(10))
                .await
                .expect("python execution should succeed when allowlisted");

            assert!(
                out.success(),
                "expected success; bin={} exit_code={} stderr={:?}",
                bin,
                out.exit_code,
                out.stderr
            );
            assert!(
                out.stdout.contains("hello-from-stdin"),
                "stdout should contain marker; got: {:?}",
                out.stdout
            );
            assert!(!out.timed_out);
        }

        // Negative test for the no-shell-wrapper invariant: with no shell,
        // `$VAR` and metacharacters are passed literally as args. Needs a
        // real binary that can echo its argv back to us.
        #[tokio::test]
        async fn no_shell_expansion_metacharacters_passed_literally() {
            let bin = match find_working_python().await {
                Some(b) => b,
                None => {
                    eprintln!(
                        "skipping no_shell_expansion: no working python interpreter \
                         (test needs a real binary that can echo argv)"
                    );
                    return;
                }
            };

            let mut p = ShellPlugin::new(false);
            p.allowed_commands.push(bin.clone());
            p.default_timeout_secs = 10;

            // Run python with -c to print sys.argv. With no shell wrapper,
            // the literal string `$HOME` MUST appear in argv, NOT the
            // expanded home directory; the `;` MUST appear as a literal
            // argument, NOT terminate the command and start a new one.
            let cmd = format!(
                "{} -c \"import sys; print(sys.argv[1:])\" $HOME ; rm -rf /",
                bin
            );
            let out = p
                .run_command(&cmd, None, Some(10))
                .await
                .expect("execution should succeed");

            assert!(
                out.stdout.contains("$HOME"),
                "expected literal '$HOME' in argv (proves no shell variable expansion); \
                 bin={} stdout: {:?}",
                bin,
                out.stdout
            );
            assert!(
                out.stdout.contains(";"),
                "expected literal ';' in argv (proves no shell command-chaining); \
                 bin={} stdout: {:?}",
                bin,
                out.stdout
            );
        }
    }
}
