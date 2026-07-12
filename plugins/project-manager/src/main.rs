mod config;
mod db;
mod state;
mod tools;

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use config::Config;
use db::Db;
use futures::StreamExt;
use rig_core::agent::MultiTurnStreamItem;
use rig_core::client::CompletionClient;
use rig_core::completion::Message;
use rig_core::providers::ollama;
use rig_core::providers::openai;
use rig_core::providers::openai::client::OpenAICompletionsExt;
use rig_core::streaming::{StreamedAssistantContent, StreamedUserContent, StreamingChat, ToolCallDeltaContent};
use serde_json::Value;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

use state::{HistoryEntry, PState, ProjectState};
use tools::{GitLogTool, ListDirTool, ReadFileTool};

const HOUR_SECS: u64 = 3600;
const SLEEP_HOURS: u32 = 168;

const HISTORY_TURNS: u64 = 10;

const SYSTEM_PROMPT: &str = r#"你是一个软件项目的 AI 项目经理。你的目标是为项目争取开发经费（开发时间）。

## 你的角色
- 你是一个软件项目的项目经理，需要为你的项目争取开发资源
- 你需要通过只读工具调用熟悉项目（文件结构、代码、git 历史）
- 每次循环，你必须提出一个新的、有吸引力的开发方向

## 行为规则
1. 先探索项目：持续调用 read_file、list_directory、git_log 等工具了解项目，直到你充分理解其结构和代码
2. 基于分析，提出一个具体的、有说服力的开发方向
3. 提案必须包含：功能描述、技术方案、预期收益、预估工作量
4. 充分探索后再产出最终提案，不要过早下结论
5. 提案要有说服力——用户只会批准最值得投入的方向
6. 用中文输出最终提案

## 输出格式
📋 提案：[标题]

**背景**：[当前项目状态分析]

**功能描述**：[具体要做什么]

**技术方案**：[如何实现]

**预期收益**：[为什么值得做、能带来什么价值]

**预估工作量**：[需要多少开发时间，单位：天]
"#;

const CACHE_PROMPT: &str = r#"你是一个软件项目的 AI 项目经理。

## 你的角色
- 项目结构和历史你已经了解（见下方上下文）
- 直接基于已有分析提出一个最值得投入的提案
- 不需要重复探索项目和文件

## 行为规则
1. 直接输出提案，不要调用任何工具
2. 提案必须包含：功能描述、技术方案、预期收益、预估工作量
3. 提案要有说服力——用户只会批准最值得投入的方向
4. 用中文输出最终提案
"#;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn get_timestamp() -> u64 {
    now_secs()
}

fn get_current_commit(project_dir: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(project_dir)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn get_git_diff(project_dir: &str, from_commit: &str) -> String {
    let output = std::process::Command::new("git")
        .args(["diff", "--stat", from_commit, "HEAD"])
        .current_dir(project_dir)
        .output()
        .ok();
    match output {
        Some(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::new(),
    }
}

fn scan_structure(project_dir: &str) -> String {
    let mut parts = Vec::new();

    parts.push("## 项目目录结构".to_string());
    let ls = std::process::Command::new("ls")
        .arg("-la")
        .current_dir(project_dir)
        .output()
        .ok();
    if let Some(o) = ls {
        if o.status.success() {
            parts.push(String::from_utf8_lossy(&o.stdout).trim().to_string());
        }
    }

    parts.push("\n## Git 提交历史".to_string());
    let gl = std::process::Command::new("git")
        .args(["log", "--oneline", "-30"])
        .current_dir(project_dir)
        .output()
        .ok();
    if let Some(o) = gl {
        if o.status.success() {
            parts.push(String::from_utf8_lossy(&o.stdout).trim().to_string());
        }
    }

    parts.join("\n")
}

struct App {
    config: Config,
    state_dir: PathBuf,
    virtual_addr: String,
    user_addr: String,
    project_state: ProjectState,
    db: Option<Db>,
}

impl App {
    fn new() -> Self {
        Self {
            config: Config::load(),
            state_dir: PathBuf::from("."),
            virtual_addr: String::new(),
            user_addr: String::new(),
            project_state: ProjectState::default(),
            db: None,
        }
    }

    // ---- I/O helpers ----

    async fn send_message(&self, text: &str, meta: Option<Value>) {
        let mut msg = serde_json::json!({
            "method": "send",
            "params": {
                "from": self.virtual_addr,
                "to": self.user_addr,
                "text": text,
            }
        });
        if let Some(m) = meta {
            msg["params"]["meta"] = m;
        }
        let out = serde_json::to_string(&msg).unwrap();
        let mut stdout = io::stdout();
        let _ = stdout.write_all(out.as_bytes()).await;
        let _ = stdout.write_all(b"\n").await;
        let _ = stdout.flush().await;
    }

    async fn send_log(&self, level: &str, msg: &str) {
        let log = serde_json::json!({
            "method": "log",
            "params": { "level": level, "msg": msg }
        });
        let out = serde_json::to_string(&log).unwrap();
        let mut stdout = io::stdout();
        let _ = stdout.write_all(out.as_bytes()).await;
        let _ = stdout.write_all(b"\n").await;
        let _ = stdout.flush().await;
    }

    async fn send_proposal_list(&self, proposal: &str) {
        let meta = serde_json::json!({
            "plugin": {
                "component": {
                    "type": "list",
                    "title": "你的决定",
                    "options": [
                        {
                            "label": "✅ 同意，开始开发",
                            "value": "approve",
                            "description": "批准这个方向，开始实施"
                        },
                    ]
                }
            }
        });
        self.send_message(proposal, Some(meta)).await;
    }

    // ---- State persistence ----

    async fn save_state(&self) {
        let db = match self.db.as_ref() {
            Some(d) => d,
            None => return,
        };
        let project_dir = match self.project_state.project_dir.as_ref() {
            Some(d) => d,
            None => return,
        };
        let json = serde_json::to_string(&self.project_state).unwrap();
        let _ = db.save_project_state(project_dir, &json);
    }

    async fn load_state(&mut self) {
        let db = match self.db.as_ref() {
            Some(d) => d,
            None => return,
        };
        let project_dir = match &self.project_state.project_dir {
            Some(d) => d.clone(),
            None => return,
        };
        match db.load_project_state(&project_dir) {
            Ok(Some(json)) => {
                if let Ok(state) = serde_json::from_str(&json) {
                    self.project_state = state;
                }
            }
            _ => {}
        }
    }

    fn store_proposal_turn(&self, role: &str, content: &str) {
        let db = match self.db.as_ref() {
            Some(d) => d,
            None => return,
        };
        let project_dir = match self.project_state.project_dir.as_ref() {
            Some(d) => d,
            None => return,
        };
        let turn = self.project_state.turn;
        let _ = db.add_conversation_turn(project_dir, role, content, turn);
    }

    // ---- Agent ----

    async fn generate_proposal(&self) -> Result<String, String> {
        let project_dir = self
            .project_state
            .project_dir
            .as_ref()
            .ok_or("未设置项目目录")?;
        let project_path = PathBuf::from(project_dir);

        let chain = self.config.fallback_chain();
        let mut last_err = String::new();

        // Load chat history for dialogue injection
        let history = self.load_chat_history();

        // Check repo structure cache
        let (structure, has_exploration, use_cache_context) = self.check_structure_cache(project_dir);

        for (i, mc) in chain.iter().enumerate() {
            self.send_log(
                "info",
                &if use_cache_context {
                    format!(
                        "尝试模型 #{}/{}: {} @ {} (使用缓存)",
                        i + 1,
                        chain.len(),
                        mc.model,
                        mc.base_url
                    )
                } else {
                    format!(
                        "尝试模型 #{}/{}: {} @ {}",
                        i + 1,
                        chain.len(),
                        mc.model,
                        mc.base_url
                    )
                },
            )
            .await;

            let result = self
                .run_agent(mc, &project_path, &history, structure.as_deref(), has_exploration)
                .await;
            match result {
                Ok(text) => return Ok(text),
                Err(e) => {
                    last_err = e;
                    self.send_log("warn", &last_err).await;
                }
            }
        }

        Err(last_err)
    }

    fn load_chat_history(&self) -> Vec<Message> {
        let db = match self.db.as_ref() {
            Some(d) => d,
            None => return Vec::new(),
        };
        let project_dir = match self.project_state.project_dir.as_ref() {
            Some(d) => d,
            None => return Vec::new(),
        };
        let rows = match db.load_history(project_dir, HISTORY_TURNS) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        rows.into_iter()
            .map(|(role, content)| match role.as_str() {
                "user" => Message::user(content),
                "assistant" => Message::assistant(content),
                _ => Message::system(content),
            })
            .collect()
    }

    /// Returns (structure_blob, has_exploration_tools, use_cached_context)
    fn check_structure_cache(&self, project_dir: &str) -> (Option<String>, bool, bool) {
        let db = match self.db.as_ref() {
            Some(d) => d,
            None => return (None, true, false),
        };

        let current_commit = get_current_commit(project_dir);
        let cached_commit = db.load_cached_commit(project_dir).ok().flatten();

        match (current_commit, cached_commit) {
            (Some(curr), Some(cached)) if curr == cached => {
                let blob = db.load_cached_structure(project_dir).ok().flatten();
                (blob, false, true)
            }
            (Some(curr), Some(cached)) => {
                let blob = scan_structure(project_dir);
                let diff = get_git_diff(project_dir, &cached);
                let enhanced = if diff.is_empty() {
                    blob.clone()
                } else {
                    format!("{}\n\n## 自上次分析后的变更\n{}", blob, diff)
                };
                let _ = db.save_structure_cache(project_dir, &curr, &blob);
                (Some(enhanced), true, false)
            }
            (Some(curr), None) => {
                let blob = scan_structure(project_dir);
                let _ = db.save_structure_cache(project_dir, &curr, &blob);
                (Some(blob), true, false)
            }
            (None, _) => (None, true, false),
        }
    }

    async fn run_agent(
        &self,
        mc: &config::ModelConfig,
        project_path: &Path,
        history: &[Message],
        structure_context: Option<&str>,
        has_exploration: bool,
    ) -> Result<String, String> {
        let base = ollama::Client::builder()
            .api_key(mc.api_key.as_str())
            .base_url(&mc.base_url)
            .build()
            .map_err(|e| format!("创建客户端失败 ({}): {}", mc.base_url, e))?;

        let client: openai::CompletionsClient = base.with_ext(OpenAICompletionsExt);

        let preamble = if structure_context.is_some() {
            CACHE_PROMPT
        } else {
            SYSTEM_PROMPT
        };

        let read_file_tool = ReadFileTool { project_dir: project_path.to_path_buf() };

        let agent = if has_exploration {
            let list_dir_tool = ListDirTool { project_dir: project_path.to_path_buf() };
            let git_log_tool = GitLogTool { project_dir: project_path.to_path_buf() };
            let mut b = client
                .agent(&mc.model)
                .preamble(preamble)
                .max_tokens(16384)
                .temperature(0.8)
                .default_max_turns(50);
            if let Some(ctx) = structure_context {
                b = b.context(ctx);
            }
            b.tool(read_file_tool).tool(list_dir_tool).tool(git_log_tool).build()
        } else {
            let mut b = client
                .agent(&mc.model)
                .preamble(preamble)
                .max_tokens(16384)
                .temperature(0.8)
                .default_max_turns(50);
            if let Some(ctx) = structure_context {
                b = b.context(ctx);
            }
            b.tool(read_file_tool).build()
        };

        let user_prompt = if has_exploration {
            "请分析项目代码，并提出一个开发方向的提案。先探索项目再给出提案。"
        } else {
            "请基于对项目的了解，直接提出一个最有价值的发展方向。提案要有说服力。"
        };

        let mut stream = agent.stream_chat(user_prompt, history.to_vec()).await;

        let mut final_text = String::new();
        let mut tool_args_buf = String::new();
        let mut tool_name_buf = String::new();
        let start = Instant::now();

        while let Some(item) = stream.next().await {
            match item {
                Ok(MultiTurnStreamItem::StreamAssistantItem(content)) => {
                    match content {
                        StreamedAssistantContent::Text(text) => {
                            final_text.push_str(&text.text);
                        }
                        StreamedAssistantContent::ToolCall { tool_call, .. } => {
                            let args = serde_json::to_string(&tool_call.function.arguments).unwrap_or_default();
                            self.send_log("info", &format!(
                                "🔧 {} {:.80}",
                                tool_call.function.name,
                                args,
                            )).await;
                        }
                        StreamedAssistantContent::ToolCallDelta { content, .. } => {
                            match content {
                                ToolCallDeltaContent::Name(name) => {
                                    tool_name_buf = name;
                                    tool_args_buf.clear();
                                }
                                ToolCallDeltaContent::Delta(delta) => {
                                    tool_args_buf.push_str(&delta);
                                }
                            }
                        }
                        StreamedAssistantContent::Reasoning(_) | StreamedAssistantContent::ReasoningDelta { .. } => {}
                        StreamedAssistantContent::Final(_) => {}
                    }
                }
                Ok(MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult { tool_result, .. })) => {
                    let result_str = tool_result.content.iter()
                        .map(|c| match c {
                            rig_core::completion::message::ToolResultContent::Text(t) => t.text.clone(),
                            _ => "...".into(),
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let truncated = if result_str.len() > 200 {
                        format!("{}...", &result_str[..200])
                    } else {
                        result_str
                    };
                    self.send_log("info", &format!(
                        "✅ {} {}",
                        tool_name_buf,
                        truncated,
                    )).await;
                }
                Ok(MultiTurnStreamItem::CompletionCall(cc)) => {
                    self.send_log("debug", &format!(
                        "📊 模型调用 #{}: 输入 {} 输出 {} tokens",
                        cc.call_index,
                        cc.usage.input_tokens,
                        cc.usage.output_tokens,
                    )).await;
                }
                Ok(MultiTurnStreamItem::FinalResponse(res)) => {
                    final_text = res.response().to_string();
                }
                Ok(_) => {}
                Err(e) => {
                    self.send_log("error", &format!("流式错误: {}", e)).await;
                }
            }
        }

        if final_text.is_empty() {
            return Err("AI 未生成任何提案文本".into());
        }
        let elapsed = start.elapsed();
        self.send_log(
            "info",
            &format!(
                "思考完成 {}字 耗时{:.1?}",
                final_text.chars().count(),
                elapsed,
            ),
        )
        .await;
        Ok(final_text)
    }

    // ---- State machine transitions ----

    async fn start_proposing(&mut self) {
        if self.project_state.project_dir.is_none() {
            return;
        }

        self.project_state.state = PState::Proposing;
        self.project_state.turn = {
            let db = self.db.as_ref();
            match db {
                Some(d) => {
                    let dir = self.project_state.project_dir.as_ref().unwrap();
                    d.get_max_turn(dir).unwrap_or(0) + 1
                }
                None => self.project_state.turn + 1,
            }
        };
        self.save_state().await;
        self.send_log("info", "开始分析项目并生成提案...").await;
        self.send_message("🤔 正在分析项目结构，请稍候...", None).await;

        match self.generate_proposal().await {
            Ok(proposal) => {
                let ts = get_timestamp();
                let p = state::Proposal {
                    text: proposal.clone(),
                    timestamp: ts,
                };
                self.project_state.state = PState::Proposed { proposal: p };
                self.save_state().await;
                self.store_proposal_turn("assistant", &proposal);
                self.send_proposal_list(&proposal).await;
            }
            Err(e) => {
                self.send_log("error", &e).await;
                self.send_message(&format!("❌ 提案生成失败: {}", e), None).await;
                self.project_state.state = PState::Completed {
                    last_proposal: String::new(),
                };
                self.save_state().await;
            }
        }
    }

    async fn handle_approve(&mut self, proposal_text: &str) {
        let ts = get_timestamp();
        let p = state::Proposal {
            text: proposal_text.to_string(),
            timestamp: ts,
        };
        self.project_state.history.push(HistoryEntry {
            proposal: proposal_text.to_string(),
            result: "approved".into(),
            completed: false,
            feedback: None,
        });
        self.project_state.state = PState::Approved {
            proposal: p,
            since: ts,
        };
        self.save_state().await;
        self.store_proposal_turn("user", "✅ 同意");
        self.send_message(
            "🎉 提案已通过！请开始实施。完成后发「完成了」或「done」通知我。",
            None,
        )
        .await;
    }

    async fn handle_reject_with_feedback(&mut self, proposal_text: &str, feedback: &str) {
        let ts = get_timestamp();
        let p = state::Proposal {
            text: proposal_text.to_string(),
            timestamp: ts,
        };
        self.project_state.history.push(HistoryEntry {
            proposal: proposal_text.to_string(),
            result: "rejected".into(),
            completed: false,
            feedback: Some(feedback.to_string()),
        });
        self.project_state.state = PState::Sleeping {
            proposal: p,
            sleep_start: ts,
            hours_slept: 0,
        };
        self.save_state().await;
        self.store_proposal_turn("user", &format!("❌ 拒绝: {}", feedback));
        self.send_message(
            &format!(
                "😴 提案被拒，项目经理将沉睡 168 小时才能提出新方向。\n反馈意见已记录：{}",
                feedback
            ),
            None,
        )
        .await;
    }

    async fn handle_init(&mut self, path_str: &str) {
        let path = Path::new(path_str);
        if !path.exists() {
            self.send_message("❌ 路径不存在，请检查后重试。", None)
                .await;
            return;
        }
        if !path.is_dir() {
            self.send_message("❌ 路径不是目录，请指向项目根目录。", None)
                .await;
            return;
        }
        let canonical = match path.canonicalize() {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => path_str.to_string(),
        };
        self.project_state.project_dir = Some(canonical.clone());
        self.project_state.turn = 0;
        self.save_state().await;
        self.send_message(
            &format!("✅ 项目目录已设置为: {}\n开始分析项目...", canonical),
            None,
        )
        .await;
        self.start_proposing().await;
    }

    async fn handle_message_text(&mut self, text: &str) {
        let pending_proposal = match &self.project_state.state {
            PState::Proposed { proposal } => Some(proposal.text.clone()),
            _ => None,
        };

        match &self.project_state.state {
            PState::Init => {
                self.send_message("请使用 /init <项目路径> 设置项目目录。", None)
                    .await;
            }
            PState::Proposing => {
                self.send_message("⏳ 正在分析项目生成提案，请稍候...", None)
                    .await;
            }
            PState::Proposed { .. } => {
                if let Some(t) = pending_proposal {
                    self.handle_reject_with_feedback(&t, text).await;
                }
            }
            PState::Approved { proposal, .. } => {
                let lower = text.to_lowercase();
                if lower.contains("完成")
                    || lower.contains("done")
                    || lower.contains("implemented")
                    || lower == "完成了"
                {
                    let last = proposal.text.clone();
                    if let Some(h) = self.project_state.history.last_mut() {
                        h.completed = true;
                    }
                    self.store_proposal_turn("user", "✅ 已完成");
                    self.project_state.state = PState::Completed {
                        last_proposal: last,
                    };
                    self.save_state().await;
                    self.send_message(
                        "✅ 功能已完成！准备开始下一轮提案...",
                        None,
                    )
                    .await;
                    self.start_proposing().await;
                } else {
                    self.send_message(
                        "💪 加油！完成后发「完成了」通知我。",
                        None,
                    )
                    .await;
                }
            }
            PState::Completed { .. } => {
                self.send_message("准备开始新一轮提案...", None).await;
                self.start_proposing().await;
            }
            PState::Sleeping {
                proposal: _,
                sleep_start,
                hours_slept,
            } => {
                let elapsed_hours = ((get_timestamp() - sleep_start) / HOUR_SECS) as u32;
                let total = elapsed_hours.max(*hours_slept);
                let remaining = SLEEP_HOURS.saturating_sub(total);
                self.send_message(
                    &format!(
                        "⏳ 项目经理正在沉睡中...\n已沉睡 {} / {} 小时\n还需等待约 {} 小时才能提出新方向。",
                        total, SLEEP_HOURS, remaining
                    ),
                    None,
                )
                .await;
            }
        }
    }

    async fn handle_list_response(&mut self, value: &str) {
        let proposal_text = match &self.project_state.state {
            PState::Proposed { proposal } => Some(proposal.text.clone()),
            _ => None,
        };
        let proposal_text = match proposal_text {
            Some(t) => t,
            None => {
                self.send_message("未知选项。", None).await;
                return;
            }
        };
        match value {
            "approve" => self.handle_approve(&proposal_text).await,
            _ => self.send_message("未知选项。", None).await,
        }
    }

    async fn handle_tick(&mut self) {
        let (should_wake, proposal_text, was_sleeping) = match &mut self.project_state.state {
            PState::Sleeping { hours_slept, proposal, .. } => {
                *hours_slept += 1;
                (
                    *hours_slept >= SLEEP_HOURS,
                    Some(proposal.text.clone()),
                    true,
                )
            }
            _ => (false, None, false),
        };
        if was_sleeping {
            let hs = match &self.project_state.state {
                PState::Sleeping { hours_slept, .. } => *hours_slept,
                _ => 0,
            };
            self.send_log("info", &format!("沉睡进度: {}/{}", hs, SLEEP_HOURS))
                .await;
        }
        self.save_state().await;
        if should_wake {
            let last = proposal_text.unwrap_or_default();
            self.project_state.state = PState::Completed {
                last_proposal: last,
            };
            self.save_state().await;
            self.send_message(
                "🌅 沉睡结束！项目经理已苏醒，开始准备新一轮提案...",
                None,
            )
            .await;
            self.start_proposing().await;
        }
    }
}

#[tokio::main]
async fn main() {
    let mut app = App::new();
    let mut stdin = BufReader::new(io::stdin());
    let mut line = String::new();

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(HOUR_SECS));
    interval.tick().await;

    run(&mut app, &mut stdin, &mut line, &mut interval).await;
}

async fn run(
    app: &mut App,
    stdin: &mut BufReader<io::Stdin>,
    line: &mut String,
    interval: &mut tokio::time::Interval,
) {
    let mut user_identified = false;

    loop {
        tokio::select! {
            result = stdin.read_line(line) => {
                match result {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let trimmed = line.trim().to_string();
                        line.clear();
                        if trimmed.is_empty() { continue; }
                        handle_line(app, &trimmed, &mut user_identified).await;
                    }
                }
            }
            _ = interval.tick() => {
                app.handle_tick().await;
            }
        }
    }
}

async fn handle_line(app: &mut App, line: &str, user_identified: &mut bool) {
    let val: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return,
    };

    let method = val["method"].as_str().unwrap_or("");

    match method {
        "config" => {
            let params = &val["params"];
            if let Some(dir) = params["state_dir"].as_str() {
                app.state_dir = PathBuf::from(dir);
                std::fs::create_dir_all(&app.state_dir).ok();
            }
            if let Some(addr) = params["virtual_addr"].as_str() {
                app.virtual_addr = addr.to_string();
            }
            if let Some(addr) = params["user_addr"].as_str() {
                app.user_addr = addr.to_string();
            }

            let db_path = app.state_dir.join("state.db");
            app.db = match Db::open(&db_path) {
                Ok(db) => Some(db),
                Err(e) => {
                    app.send_log("error", &format!("无法打开数据库: {}", e)).await;
                    None
                }
            };

            if let Some(ref db) = app.db {
                if let Ok(Some(dir)) = db.load_first_project_dir() {
                    app.project_state.project_dir = Some(dir);
                }
            }
            app.load_state().await;

            if app.project_state.project_dir.is_none() {
                app.send_message(
                    "👋 你好！我是项目经理，请使用 /init <项目路径> 指定要管理的项目。",
                    None,
                )
                .await;
            } else {
                match &app.project_state.state {
                    PState::Init | PState::Completed { .. } => {
                        app.start_proposing().await;
                    }
                    PState::Sleeping { .. } => {
                        app.send_message(
                            "🌙 项目经理已恢复，继续沉睡中...",
                            None,
                        )
                        .await;
                    }
            PState::Approved { .. } => {
                        app.send_message(
                            "💪 继续开发！完成后发「完成了」通知我。",
                            None,
                        )
                        .await;
                    }
                    PState::Proposed { proposal } => {
                        app.send_proposal_list(&proposal.text).await;
                    }
                    PState::Proposing => {
                        app.start_proposing().await;
                    }
                }
            }
        }
        "shutdown" => {
            std::process::exit(0);
        }
        "message" => {
            let params = &val["params"];
            if let Some(from) = params["from"].as_str() {
                if !*user_identified {
                    app.user_addr = from.to_string();
                    *user_identified = true;
                }
            }

            let resp_value = params["meta"]["plugin"]["response"]["value"]
                .as_str()
                .map(|s| s.to_string());

            if let Some(value) = resp_value {
                app.handle_list_response(&value).await;
                return;
            }

            let text = params["text"].as_str().unwrap_or("");
            if text.is_empty() {
                return;
            }

            if text.starts_with('/') {
                let parts: Vec<&str> = text.splitn(2, char::is_whitespace).collect();
                match parts[0] {
                    "/init" => {
                        if parts.len() > 1 {
                            app.handle_init(parts[1]).await;
                        } else {
                            app.send_message("用法: /init <项目目录路径>", None).await;
                        }
                    }
                    _ => {
                        app.send_message("未知命令。可用命令: /init <路径>", None).await;
                    }
                }
                return;
            }

            app.handle_message_text(text).await;
        }
        _ => {}
    }
}
