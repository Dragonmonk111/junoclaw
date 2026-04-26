use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{info, warn};

use junoclaw_core::config::JunoClawConfig;
use junoclaw_core::plugin::StreamEvent;
use junoclaw_core::types::{
    gen_id, now_millis, AgentInfo, ChatMessage, CompletionRequest, LlmMessage, MessageRole,
    Session, Task, ToolCall, WsClientMessage, WsServerMessage,
};
use plugin_llm::ollama::OllamaProvider;
use plugin_llm::LlmProviderRegistry;
use plugin_shell::ShellPlugin;

pub struct Runtime {
    _config: JunoClawConfig,
    agents: Arc<RwLock<HashMap<String, AgentInfo>>>,
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    tasks: Arc<RwLock<Vec<Task>>>,
    llm: Arc<RwLock<LlmProviderRegistry>>,
    shell: Arc<ShellPlugin>,
    /// Pending tool-call approvals: task_id → oneshot sender (true=approve, false=deny)
    pending_approvals: Arc<RwLock<HashMap<String, oneshot::Sender<bool>>>>,
    /// Directory where agents.json and sessions.json are persisted
    data_dir: PathBuf,
}

impl Runtime {
    pub async fn new(config: &JunoClawConfig) -> anyhow::Result<Self> {
        info!("Initializing JunoClaw runtime...");

        let mut llm_registry = LlmProviderRegistry::new();

        // Initialize Ollama provider from config
        let ollama_cfg = config
            .llm
            .providers
            .ollama
            .clone()
            .unwrap_or_default();
        let provider = OllamaProvider::new(&ollama_cfg.endpoint, &ollama_cfg.default_model);
        llm_registry.register(Box::new(provider));
        info!(
            "Registered Ollama provider: {} @ {}",
            ollama_cfg.default_model, ollama_cfg.endpoint
        );

        // Resolve data dir: ~/.junoclaw/
        let data_dir = dirs_data_dir();
        tokio::fs::create_dir_all(&data_dir).await
            .unwrap_or_else(|e| warn!("Could not create data dir {:?}: {}", data_dir, e));

        // Load persisted state
        let agents = load_agents(&data_dir).await;
        let sessions = load_sessions(&data_dir).await;
        info!("Loaded {} agent(s) and {} session(s) from disk", agents.len(), sessions.len());

        let runtime = Self {
            _config: config.clone(),
            agents: Arc::new(RwLock::new(agents)),
            sessions: Arc::new(RwLock::new(sessions)),
            tasks: Arc::new(RwLock::new(Vec::new())),
            llm: Arc::new(RwLock::new(llm_registry)),
            // sandbox_mode = false: per post-Ffern design, the Cargo `unsafe-shell`
            // feature is the primary gate. With feature off (default), run_command /
            // run_python compile to error stubs regardless of this value. With feature
            // on, sandbox_mode acts as the runtime kill-switch (operator flips to true
            // at runtime via config to halt execution without a redeploy).
            shell: Arc::new(ShellPlugin::new(false)),
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            data_dir,
        };

        info!(
            "Runtime ready. LLM provider: {}, chain: {}",
            config.llm.default_provider,
            if config.chain.enabled {
                &config.chain.chain_id
            } else {
                "disabled"
            }
        );

        Ok(runtime)
    }

    pub async fn list_agents(&self) -> Vec<AgentInfo> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Handle a WS message, sending responses (possibly multiple for streaming) via the tx channel.
    pub async fn handle_message(
        &self,
        msg: WsClientMessage,
        tx: mpsc::Sender<WsServerMessage>,
    ) {
        match msg {
            WsClientMessage::ListAgents => {
                let agents = self.list_agents().await;
                let _ = tx.send(WsServerMessage::AgentList(agents)).await;
            }

            WsClientMessage::CreateAgent(agent_info) => {
                let id = agent_info.id.clone();
                let agents_list = {
                    let mut agents = self.agents.write().await;
                    agents.insert(id.clone(), agent_info);
                    info!("Agent created: {}", id);
                    agents.values().cloned().collect::<Vec<_>>()
                };
                save_agents(&self.data_dir, &agents_list).await;
                let _ = tx.send(WsServerMessage::AgentList(agents_list)).await;
            }

            WsClientMessage::SendMessage { agent_id, content } => {
                // Check agent exists
                {
                    let agents = self.agents.read().await;
                    if !agents.contains_key(&agent_id) {
                        let _ = tx.send(WsServerMessage::Error {
                            message: format!("Agent not found: {}", agent_id),
                        }).await;
                        return;
                    }
                }

                info!("Message to agent {}: {}", agent_id, &content[..content.len().min(80)]);

                // Store user message
                let user_msg = ChatMessage {
                    id: gen_id(),
                    role: MessageRole::User,
                    content: content.clone(),
                    tool_calls: None,
                    timestamp: now_millis(),
                };
                {
                    let mut sessions = self.sessions.write().await;
                    let session = sessions.entry(agent_id.clone()).or_insert_with(|| Session {
                        id: gen_id(),
                        agent_id: agent_id.clone(),
                        messages: Vec::new(),
                        created_at: now_millis(),
                        updated_at: now_millis(),
                    });
                    session.messages.push(user_msg);
                }

                let model = {
                    let agents = self.agents.read().await;
                    agents.get(&agent_id)
                        .map(|a| a.model.clone())
                        .unwrap_or_else(|| "llama3.2:3b".to_string())
                };

                // Tool call loop: LLM → detect tool → execute → feed result → repeat
                let max_tool_rounds = 5usize;
                for _round in 0..=max_tool_rounds {
                    let llm_messages: Vec<LlmMessage> = {
                        let sessions = self.sessions.read().await;
                        let mut msgs: Vec<LlmMessage> = vec![LlmMessage {
                            role: MessageRole::System,
                            content: TOOL_SYSTEM_PROMPT.to_string(),
                        }];
                        if let Some(s) = sessions.get(&agent_id) {
                            msgs.extend(s.messages.iter().map(|m| LlmMessage {
                                role: m.role.clone(),
                                content: m.content.clone(),
                            }));
                        }
                        msgs
                    };

                    let req = CompletionRequest {
                        messages: llm_messages,
                        model: model.clone(),
                        temperature: Some(0.7),
                        max_tokens: Some(1024),
                        tools: None,
                        stream: true,
                    };

                    let llm = self.llm.read().await;
                    let stream_result = llm.stream(&req).await;
                    drop(llm);

                    let mut full_content = String::new();
                    let mut stream_error: Option<String> = None;

                    match stream_result {
                        Ok(mut stream_rx) => {
                            // Buffer tokens; hold back <tool_call> block if present
                            let mut pending_buf = String::new();
                            let mut in_tool_call = false;

                            while let Some(event) = stream_rx.recv().await {
                                match event {
                                    StreamEvent::Token(token) => {
                                        full_content.push_str(&token);
                                        pending_buf.push_str(&token);

                                        // Detect start of <tool_call> — stop streaming to user
                                        if pending_buf.contains("<tool_call>") && !in_tool_call {
                                            in_tool_call = true;
                                            // Flush text before the tag
                                            if let Some(pos) = pending_buf.find("<tool_call>") {
                                                let visible = pending_buf[..pos].to_string();
                                                if !visible.trim().is_empty() {
                                                    let _ = tx.send(WsServerMessage::StreamToken {
                                                        agent_id: agent_id.clone(),
                                                        token: visible,
                                                    }).await;
                                                }
                                            }
                                        } else if !in_tool_call {
                                            let _ = tx.send(WsServerMessage::StreamToken {
                                                agent_id: agent_id.clone(),
                                                token: token.clone(),
                                            }).await;
                                        }
                                    }
                                    StreamEvent::Done(response) => {
                                        if !response.content.is_empty() {
                                            full_content = response.content;
                                        }
                                        break;
                                    }
                                    StreamEvent::ToolCall(_) => {}
                                    StreamEvent::Error(err) => {
                                        stream_error = Some(err);
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            stream_error = Some(e.to_string());
                        }
                    }

                    if let Some(err) = stream_error {
                        warn!("LLM error: {}", err);
                        let _ = tx.send(WsServerMessage::Error {
                            message: format!("LLM error: {}", err),
                        }).await;
                        return;
                    }

                    // ── Detect tool call in response ──────────────────────────
                    if let Some((tc, visible_text)) = extract_tool_call(&full_content) {
                        info!("Tool call detected: {} {:?}", tc.name, tc.arguments);

                        // Store the visible part (text before tool call) as assistant msg
                        if !visible_text.trim().is_empty() {
                            let partial_msg = ChatMessage {
                                id: gen_id(),
                                role: MessageRole::Assistant,
                                content: visible_text.clone(),
                                tool_calls: None,
                                timestamp: now_millis(),
                            };
                            let mut sessions = self.sessions.write().await;
                            if let Some(s) = sessions.get_mut(&agent_id) {
                                s.messages.push(partial_msg);
                            }
                        }

                        // Register pending approval
                        let (approval_tx, approval_rx) = oneshot::channel::<bool>();
                        {
                            let mut pending = self.pending_approvals.write().await;
                            pending.insert(tc.id.clone(), approval_tx);
                        }

                        // Ask frontend for approval
                        let task_id = tc.id.clone();
                        let _ = tx.send(WsServerMessage::ToolCallRequest {
                            task_id: task_id.clone(),
                            tool_call: tc.clone(),
                        }).await;

                        // Wait for user decision (30s timeout)
                        let approved = match tokio::time::timeout(
                            std::time::Duration::from_secs(30),
                            approval_rx,
                        ).await {
                            Ok(Ok(v)) => v,
                            _ => {
                                warn!("Tool call approval timed out");
                                // Clean up
                                let mut pending = self.pending_approvals.write().await;
                                pending.remove(&task_id);
                                false
                            }
                        };

                        if !approved {
                            let denied_msg = ChatMessage {
                                id: gen_id(),
                                role: MessageRole::Tool,
                                content: format!("Tool call '{}' was denied by user.", tc.name),
                                tool_calls: None,
                                timestamp: now_millis(),
                            };
                            let mut sessions = self.sessions.write().await;
                            if let Some(s) = sessions.get_mut(&agent_id) {
                                s.messages.push(denied_msg);
                            }
                            // Continue loop so LLM can respond to the denial
                            continue;
                        }

                        // Execute the tool
                        info!("Executing tool: {}", tc.name);
                        let tool_result = self.execute_tool(&tc).await;

                        let tool_output = match &tool_result {
                            Ok(out) => format!("Tool '{}' output:\n{}", tc.name, out),
                            Err(e) => format!("Tool '{}' error: {}", tc.name, e),
                        };

                        info!("Tool result: {}", &tool_output[..tool_output.len().min(200)]);

                        // Add tool result to session so LLM sees it
                        let tool_msg = ChatMessage {
                            id: gen_id(),
                            role: MessageRole::Tool,
                            content: tool_output.clone(),
                            tool_calls: None,
                            timestamp: now_millis(),
                        };
                        {
                            let mut sessions = self.sessions.write().await;
                            if let Some(s) = sessions.get_mut(&agent_id) {
                                s.messages.push(tool_msg);
                            }
                        }
                        // Continue to next LLM round with tool result in context
                        continue;
                    }

                    // ── No tool call — this is the final response ─────────────
                    let clean_content = full_content.trim().to_string();
                    let assistant_msg = ChatMessage {
                        id: gen_id(),
                        role: MessageRole::Assistant,
                        content: clean_content,
                        tool_calls: None,
                        timestamp: now_millis(),
                    };
                    {
                        let mut sessions = self.sessions.write().await;
                        if let Some(session) = sessions.get_mut(&agent_id) {
                            session.messages.push(assistant_msg.clone());
                            session.updated_at = now_millis();
                        }
                    }
                    // Persist sessions after final response
                    let sessions_snapshot = {
                        let s = self.sessions.read().await;
                        s.values().cloned().collect::<Vec<_>>()
                    };
                    save_sessions(&self.data_dir, &sessions_snapshot).await;

                    let _ = tx.send(WsServerMessage::StreamComplete {
                        agent_id,
                        message: assistant_msg,
                    }).await;
                    return;
                }

                // Exceeded max tool rounds
                let _ = tx.send(WsServerMessage::Error {
                    message: "Exceeded maximum tool call rounds (5)".to_string(),
                }).await;
            }

            WsClientMessage::ListTasks { agent_id } => {
                let tasks = self.tasks.read().await;
                let filtered: Vec<Task> = match agent_id {
                    Some(aid) => tasks.iter().filter(|t| t.agent_id == aid).cloned().collect(),
                    None => tasks.clone(),
                };
                let _ = tx.send(WsServerMessage::TaskList(filtered)).await;
            }

            WsClientMessage::CancelTask { task_id } => {
                warn!("Task cancel requested: {} (not yet implemented)", task_id);
                let _ = tx.send(WsServerMessage::Error {
                    message: "Task cancellation not yet implemented".to_string(),
                }).await;
            }

            WsClientMessage::ApproveToolCall { task_id: _, tool_call_id } => {
                info!("Tool call approved: {}", tool_call_id);
                let mut pending = self.pending_approvals.write().await;
                if let Some(sender) = pending.remove(&tool_call_id) {
                    let _ = sender.send(true);
                } else {
                    warn!("No pending approval found for tool_call_id: {}", tool_call_id);
                }
            }

            WsClientMessage::DenyToolCall { task_id: _, tool_call_id } => {
                info!("Tool call denied: {}", tool_call_id);
                let mut pending = self.pending_approvals.write().await;
                if let Some(sender) = pending.remove(&tool_call_id) {
                    let _ = sender.send(false);
                } else {
                    warn!("No pending denial found for tool_call_id: {}", tool_call_id);
                }
            }
        }
    }

    /// Execute a tool call using the appropriate plugin.
    async fn execute_tool(&self, tc: &ToolCall) -> anyhow::Result<String> {
        use plugin_shell::ShellOutput;
        let args = &tc.arguments;

        match tc.name.as_str() {
            "run_shell" => {
                let cmd = args.get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("run_shell requires 'command' argument"))?;
                let out: ShellOutput = self.shell.run_command(cmd, None, None).await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                Ok(out.combined())
            }
            "run_python" => {
                let script = args.get("script")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("run_python requires 'script' argument"))?;
                let out: ShellOutput = self.shell.run_python(script, None, None).await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                Ok(out.combined())
            }
            other => Err(anyhow::anyhow!("Unknown tool: {}", other)),
        }
    }
}

// ──────────────────────────────────────────────
// Tool call parsing helpers
// ──────────────────────────────────────────────

/// System prompt injected into every completion request.
const TOOL_SYSTEM_PROMPT: &str = r#"You are a helpful AI assistant with access to tools.
When you need to run code or a shell command, output ONLY a JSON block wrapped in <tool_call> tags.
The block must be valid JSON. DO NOT add any text inside the tags other than JSON.

Available tools:
- run_shell: Run a shell command. Args: {"command": "<cmd>"}
- run_python: Run a Python script. Args: {"script": "<python code>"}

Examples:
<tool_call>
{"tool": "run_shell", "command": "echo Hello World"}
</tool_call>

<tool_call>
{"tool": "run_python", "script": "import sys\nprint(sys.version)"}
</tool_call>

After receiving a tool result, continue your response naturally.
If no tool is needed, respond normally without any <tool_call> tags."#;

// ──────────────────────────────────────────────
// Persistence helpers
// ──────────────────────────────────────────────

fn dirs_data_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".junoclaw")
}

async fn save_agents(dir: &Path, agents: &[AgentInfo]) {
    let path = dir.join("agents.json");
    match serde_json::to_string_pretty(agents) {
        Ok(json) => {
            if let Err(e) = tokio::fs::write(&path, json).await {
                warn!("Failed to save agents: {}", e);
            } else {
                info!("Saved {} agent(s) to {:?}", agents.len(), path);
            }
        }
        Err(e) => warn!("Failed to serialize agents: {}", e),
    }
}

async fn load_agents(dir: &Path) -> HashMap<String, AgentInfo> {
    let path = dir.join("agents.json");
    match tokio::fs::read_to_string(&path).await {
        Ok(json) => {
            match serde_json::from_str::<Vec<AgentInfo>>(&json) {
                Ok(list) => {
                    info!("Loaded {} agent(s) from {:?}", list.len(), path);
                    list.into_iter().map(|a| (a.id.clone(), a)).collect()
                }
                Err(e) => { warn!("Failed to parse agents.json: {}", e); HashMap::new() }
            }
        }
        Err(_) => HashMap::new(), // No file yet — fresh start
    }
}

async fn save_sessions(dir: &Path, sessions: &[Session]) {
    let path = dir.join("sessions.json");
    match serde_json::to_string_pretty(sessions) {
        Ok(json) => {
            if let Err(e) = tokio::fs::write(&path, json).await {
                warn!("Failed to save sessions: {}", e);
            }
        }
        Err(e) => warn!("Failed to serialize sessions: {}", e),
    }
}

async fn load_sessions(dir: &Path) -> HashMap<String, Session> {
    let path = dir.join("sessions.json");
    match tokio::fs::read_to_string(&path).await {
        Ok(json) => {
            match serde_json::from_str::<Vec<Session>>(&json) {
                Ok(list) => list.into_iter().map(|s| (s.agent_id.clone(), s)).collect(),
                Err(e) => { warn!("Failed to parse sessions.json: {}", e); HashMap::new() }
            }
        }
        Err(_) => HashMap::new(),
    }
}

/// Extract a tool call from LLM output.
/// Returns `Some((ToolCall, visible_text_before_tag))` if found.
fn extract_tool_call(text: &str) -> Option<(ToolCall, String)> {
    let start_tag = "<tool_call>";
    let end_tag = "</tool_call>";

    let start = text.find(start_tag)?;
    let after_start = start + start_tag.len();
    let end = text[after_start..].find(end_tag).map(|i| after_start + i)?;

    let json_str = text[after_start..end].trim();
    let visible_text = text[..start].trim().to_string();

    // Parse JSON inside tags
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;

    let tool_name = v.get("tool").and_then(|t| t.as_str())?.to_string();

    // Collect remaining fields as arguments
    let mut args = serde_json::Map::new();
    for (k, val) in v.as_object()? {
        if k != "tool" {
            args.insert(k.clone(), val.clone());
        }
    }

    Some((
        ToolCall {
            id: gen_id(),
            name: tool_name,
            arguments: serde_json::Value::Object(args),
        },
        visible_text,
    ))
}
