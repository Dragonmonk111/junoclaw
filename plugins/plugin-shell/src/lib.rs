use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{info, warn};

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

// ──────────────────────────────────────────────
// Dangerous command patterns (sandbox blocklist)
// ──────────────────────────────────────────────

const BLOCKED_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "del /s /q c:\\",
    "format c:",
    "mkfs",
    "dd if=/dev/zero",
    "> /dev/sda",
    ":(){ :|:& };:",   // fork bomb
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "reg delete",
    "reg add hklm",
];

fn is_dangerous(cmd: &str) -> Option<&'static str> {
    let lower = cmd.to_lowercase();
    BLOCKED_PATTERNS
        .iter()
        .find(|&&pat| lower.contains(pat))
        .copied()
}

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
    pub fn new(sandbox_mode: bool) -> Self {
        Self {
            sandbox_mode,
            allowed_commands: vec![
                "python".to_string(),
                "python3".to_string(),
                "echo".to_string(),
                "ls".to_string(),
                "dir".to_string(),
                "pwd".to_string(),
                "cat".to_string(),
                "type".to_string(),
            ],
            default_timeout_secs: 30,
            workspace_dir: std::env::temp_dir().join("junoclaw-workspace"),
        }
    }

    /// Run an arbitrary shell command string.
    /// Returns `Err` if blocked by sandbox; `Ok(ShellOutput)` otherwise.
    pub async fn run_command(
        &self,
        command: &str,
        cwd: Option<&Path>,
        timeout_secs: Option<u64>,
    ) -> Result<ShellOutput> {
        // Sandbox check
        if self.sandbox_mode {
            if let Some(pattern) = is_dangerous(command) {
                warn!("Blocked dangerous command: {:?} (matched '{}')", command, pattern);
                return Err(JunoClawError::TaskExecution(
                    format!("BLOCKED: command matches dangerous pattern '{}'", pattern),
                ));
            }
        }

        let secs = timeout_secs.unwrap_or(self.default_timeout_secs);
        let work_dir = cwd.unwrap_or(&self.workspace_dir);

        // Ensure workspace exists
        if let Err(e) = tokio::fs::create_dir_all(work_dir).await {
            warn!("Could not create workspace dir: {}", e);
        }

        info!("Shell exec: {:?} (cwd={:?}, timeout={}s)", command, work_dir, secs);

        let start = Instant::now();

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        cmd.current_dir(work_dir)
           .stdout(std::process::Stdio::piped())
           .stderr(std::process::Stdio::piped());

        let result = timeout(Duration::from_secs(secs), cmd.output()).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                let exit_code = output.status.code().unwrap_or(-1);
                info!("Shell exit={} duration={}ms", exit_code, duration_ms);
                Ok(ShellOutput { stdout, stderr, exit_code, duration_ms, timed_out: false })
            }
            Ok(Err(e)) => Err(JunoClawError::TaskExecution(format!("Process error: {}", e))),
            Err(_) => {
                warn!("Command timed out after {}s", secs);
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

    /// Write `script` to a temp .py file and execute it with the system Python.
    pub async fn run_python(
        &self,
        script: &str,
        cwd: Option<&Path>,
        timeout_secs: Option<u64>,
    ) -> Result<ShellOutput> {
        let work_dir = cwd
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.workspace_dir.clone());

        if let Err(e) = tokio::fs::create_dir_all(&work_dir).await {
            warn!("Could not create workspace dir: {}", e);
        }

        // Write script to temp file
        let script_path = work_dir.join(format!("script_{}.py", uuid_short()));
        tokio::fs::write(&script_path, script)
            .await
            .map_err(|e| JunoClawError::TaskExecution(format!("Failed to write script: {}", e)))?;

        info!("Running Python script: {:?}", script_path);

        // Try python3 first, fall back to python
        let python_bin = if which_python("python3").await { "python3" } else { "python" };

        let secs = timeout_secs.unwrap_or(self.default_timeout_secs);
        let start = Instant::now();

        let result = timeout(
            Duration::from_secs(secs),
            Command::new(python_bin)
                .arg(&script_path)
                .current_dir(&work_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output(),
        )
        .await;

        // Clean up temp file
        let _ = tokio::fs::remove_file(&script_path).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                let exit_code = output.status.code().unwrap_or(-1);
                info!("Python exit={} duration={}ms", exit_code, duration_ms);
                Ok(ShellOutput { stdout, stderr, exit_code, duration_ms, timed_out: false })
            }
            Ok(Err(e)) => Err(JunoClawError::TaskExecution(format!("Python error: {}", e))),
            Err(_) => Ok(ShellOutput {
                stdout: String::new(),
                stderr: format!("Python script timed out after {}s", secs),
                exit_code: -1,
                duration_ms,
                timed_out: true,
            }),
        }
    }
}

// ──────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────

fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{:08x}", ns)
}

async fn which_python(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

// ──────────────────────────────────────────────
// Plugin trait impl (task-based interface)
// ──────────────────────────────────────────────

#[async_trait]
impl Plugin for ShellPlugin {
    fn name(&self) -> &str { "plugin-shell" }
    fn description(&self) -> &str { "Sandboxed shell/Python execution for agents" }
    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }
    fn capabilities(&self) -> Vec<PluginCapability> { vec![PluginCapability::ShellExecution] }
    fn is_available(&self) -> bool { true }

    fn config_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "sandbox_mode": { "type": "boolean", "default": true },
                "allowed_commands": { "type": "array", "items": { "type": "string" } },
                "default_timeout_secs": { "type": "integer", "default": 30 }
            }
        })
    }

    async fn initialize(&mut self, config: Value) -> Result<()> {
        self.sandbox_mode = config.get("sandbox_mode")
            .and_then(|v| v.as_bool()).unwrap_or(true);
        if let Some(cmds) = config.get("allowed_commands").and_then(|v| v.as_array()) {
            self.allowed_commands = cmds
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        self.default_timeout_secs = config.get("default_timeout_secs")
            .and_then(|v| v.as_u64()).unwrap_or(30);
        info!("Shell plugin initialized (sandbox={}, timeout={}s)",
            self.sandbox_mode, self.default_timeout_secs);
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
