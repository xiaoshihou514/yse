mod state;
mod tools;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rig_core::client::{CompletionClient, Nothing};
use rig_core::completion::Prompt;
use rig_core::providers::ollama;
use serde_json::Value;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

use state::{HistoryEntry, PState, ProjectState};
use tools::{GitLogTool, ListDirTool, ReadFileTool};

const HOUR_SECS: u64 = 3600;
const SLEEP_HOURS: u32 = 168;
const OLLAMA_MODEL: &str = "qwen2.5:7b";

const SYSTEM_PROMPT: &str = r#"你是一个软件项目的 AI 项目经理。你的目标是为项目争取开发经费（开发时间）。

## 你的角色
- 你是一个软件项目的项目经理，需要为你的项目争取开发资源
- 你需要通过只读工具调用熟悉项目（文件结构、代码、git 历史）
- 每次循环，你必须提出一个新的、有吸引力的开发方向

## 行为规则
1. 先探索项目：调用 read_file、list_directory、git_log 等工具了解项目
2. 基于分析，提出一个具体的、有说服力的开发方向
3. 提案必须包含：功能描述、技术方案、预期收益、预估工作量
4. 最多调用 5 次工具，然后必须产出最终提案
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

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn get_timestamp() -> u64 {
    now_secs()
}

struct App {
    state_dir: PathBuf,
    virtual_addr: String,
    user_addr: String,
    project_state: ProjectState,
    state_path: PathBuf,
}

impl App {
    fn new() -> Self {
        Self {
            state_dir: PathBuf::from("."),
            virtual_addr: String::new(),
            user_addr: String::new(),
            project_state: ProjectState::default(),
            state_path: PathBuf::from("state.json"),
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
                        {
                            "label": "❌ 拒绝，重新考虑",
                            "value": "reject",
                            "description": "拒绝这个提案，项目经理将沉睡一周"
                        }
                    ]
                }
            }
        });
        self.send_message(proposal, Some(meta)).await;
    }

    // ---- State persistence ----

    async fn save_state(&self) {
        self.project_state.save(&self.state_dir).await;
    }

    async fn load_state(&mut self) {
        self.project_state = ProjectState::load(&self.state_dir).await;
    }

    // ---- Agent ----

    async fn generate_proposal(&self) -> Result<String, String> {
        let client =
            ollama::Client::new(Nothing).map_err(|e| format!("创建 Ollama 客户端失败: {}", e))?;

        let project_dir = self
            .project_state
            .project_dir
            .as_ref()
            .ok_or("未设置项目目录")?;
        let project_path = PathBuf::from(project_dir);

        let read_file_tool = ReadFileTool {
            project_dir: project_path.clone(),
        };
        let list_dir_tool = ListDirTool {
            project_dir: project_path.clone(),
        };
        let git_log_tool = GitLogTool {
            project_dir: project_path,
        };

        let agent = client
            .agent(OLLAMA_MODEL)
            .preamble(SYSTEM_PROMPT)
            .max_tokens(4096)
            .temperature(0.8)
            .tool(read_file_tool)
            .tool(list_dir_tool)
            .tool(git_log_tool)
            .build();

        self.send_log("info", "正在生成提案...").await;

        let response = agent
            .prompt("请分析项目代码，并提出一个开发方向的提案。先探索项目再给出提案。")
            .await
            .map_err(|e| format!("AI 生成提案失败: {}", e))?;

        Ok(response)
    }

    // ---- State machine transitions ----

    async fn start_proposing(&mut self) {
        if self.project_state.project_dir.is_none() {
            return;
        }

        self.project_state.state = PState::Proposing;
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
        });
        self.project_state.state = PState::Approved {
            proposal: p,
            since: ts,
        };
        self.save_state().await;
        self.send_message(
            "🎉 提案已通过！请开始实施。完成后发「完成了」或「done」通知我。",
            None,
        )
        .await;
    }

    async fn handle_reject(&mut self, proposal_text: &str) {
        let ts = get_timestamp();
        let p = state::Proposal {
            text: proposal_text.to_string(),
            timestamp: ts,
        };
        self.project_state.history.push(HistoryEntry {
            proposal: proposal_text.to_string(),
            result: "rejected".into(),
            completed: false,
        });
        self.project_state.state = PState::Sleeping {
            proposal: p,
            sleep_start: ts,
            hours_slept: 0,
        };
        self.save_state().await;
        self.send_message(
            "😴 提案被拒，项目经理将沉睡 168 小时才能提出新方向。",
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
        self.save_state().await;
        self.send_message(
            &format!("✅ 项目目录已设置为: {}\n开始分析项目...", canonical),
            None,
        )
        .await;
        self.start_proposing().await;
    }

    async fn handle_message_text(&mut self, text: &str) {
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
                self.send_message(
                    "请点选上面的列表做出决定：同意或拒绝当前提案。",
                    None,
                )
                .await;
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
            "reject" => self.handle_reject(&proposal_text).await,
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
    interval.tick().await; // skip first immediate tick

    // Rest of main loop is in a separate fn to avoid borrow issues
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
            app.state_path = app.state_dir.join("state.json");
            app.load_state().await;

            if !app.project_state.project_dir.is_some() {
                app.send_message(
                    "👋 你好！我是项目经理，请使用 /init <项目路径> 指定要管理的项目。",
                    None,
                )
                .await;
            } else {
                // Resume state machine
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
            // Clean exit
            std::process::exit(0);
        }
        "message" => {
            let params = &val["params"];
            // Identify the user on first message
            if let Some(from) = params["from"].as_str() {
                if !*user_identified {
                    app.user_addr = from.to_string();
                    *user_identified = true;
                }
            }

            // Check for list component response
            let resp_value = params["meta"]["plugin"]["response"]["value"]
                .as_str()
                .map(|s| s.to_string());

            if let Some(value) = resp_value {
                app.handle_list_response(&value).await;
                return;
            }

            // Check for text message
            let text = params["text"].as_str().unwrap_or("");
            if text.is_empty() {
                return;
            }

            // Handle /init command
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

            // Regular text — route to state machine
            app.handle_message_text(text).await;
        }
        _ => {}
    }
}
