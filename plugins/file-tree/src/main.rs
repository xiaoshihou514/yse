use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

const HELP: &str = "📁 文件树插件
/cd <path>    — 切换目录
/pwd          — 显示当前目录
/ls [path]    — 列出目录（可点选进入）
/tree [path]  — 树形展示
/cat <file>   — 查看文件（前 50 行）
/stat <path>  — 文件详情
/find <name>  — 递归搜索
/size         — 目录总大小
/help 或 ?     — 此帮助";

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    // Per-user working directories
    let mut cwds: HashMap<String, PathBuf> = HashMap::new();
    let mut state_dir: Option<String> = None;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let val: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                let _ = writeln!(stdout, "{}", log("warn", format!("invalid JSON: {trimmed}")));
                let _ = stdout.flush();
                continue;
            }
        };

        let method = val["method"].as_str().unwrap_or("").to_string();

        match method.as_str() {
            "message" => {
                let params = &val["params"];
                let from = params["from"].as_str().unwrap_or("unknown");
                let to = params["to"].as_str().unwrap_or("");

                let cwd = cwds
                    .entry(from.to_string())
                    .or_insert_with(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

                // Check if this is a list selection response
                let response_value = params["meta"]["plugin"]["response"]["value"]
                    .as_str()
                    .map(|s| s.to_string());

                if let Some(cmd_text) = response_value {
                    // Handle list selection: treat the value as a command
                    handle_and_respond(cwd, &cmd_text, from, to, &mut stdout);
                } else {
                    let text = params["text"].as_str().unwrap_or("").trim().to_string();
                    handle_and_respond(cwd, &text, from, to, &mut stdout);
                }

                let _ = writeln!(stdout, "{}", log("info", format!("handled from {from}")));
                let _ = stdout.flush();

                // Persist CWD state for recovery on next start
                if let Some(ref dir) = state_dir {
                    let cwd_map: HashMap<String, String> = cwds
                        .iter()
                        .map(|(k, v)| (k.clone(), v.display().to_string()))
                        .collect();
                    if let Ok(json) = serde_json::to_string(&cwd_map) {
                        let _ = fs::write(format!("{dir}/cwd.json"), json);
                    }
                }
            }
            "config" => {
                state_dir = val["params"]["state_dir"].as_str().map(|s| s.to_string());
                if let Some(ref dir) = state_dir {
                    // Restore persisted CWD state
                    if let Ok(content) = fs::read_to_string(format!("{dir}/cwd.json")) {
                        if let Ok(saved) = serde_json::from_str::<HashMap<String, String>>(&content) {
                            for (user, path) in saved {
                                cwds.insert(user, PathBuf::from(path));
                            }
                        }
                    }
                }
            }
            "shutdown" => break,
            _ => {}
        }
    }
}

fn send_text(to: &str, from: &str, text: &str, stdout: &mut impl Write) {
    let response = serde_json::json!({
        "method": "send",
        "params": { "from": to, "to": from, "text": text }
    });
    let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
    let _ = stdout.flush();
}

fn send_list(
    to: &str,
    from: &str,
    text: &str,
    title: &str,
    options: &[serde_json::Value],
    stdout: &mut impl Write,
) {
    let response = serde_json::json!({
        "method": "send",
        "params": {
            "from": to,
            "to": from,
            "text": text,
            "meta": {
                "plugin": {
                    "component": {
                        "type": "list",
                        "title": title,
                        "options": options
                    }
                }
            }
        }
    });
    let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
    let _ = stdout.flush();
}

fn option(label: &str, value: &str, desc: &str) -> serde_json::Value {
    serde_json::json!({
        "label": label,
        "value": value,
        "description": desc,
    })
}

fn handle_and_respond(cwd: &mut PathBuf, text: &str, from: &str, to: &str, stdout: &mut impl Write) {
    let text = text.trim();
    if text.is_empty() {
        send_text(to, from, "输入 ? 查看可用命令", stdout);
        return;
    }

    // ? triggers help
    if text == "?" || text == "？" || text == "help" {
        send_text(to, from, HELP, stdout);
        return;
    }

    // Commands must start with /
    let Some(cmd_text) = text.strip_prefix('/') else {
        send_text(to, from, &format!("未知命令: {text}\n输入 ? 查看可用命令"), stdout);
        return;
    };

    let (cmd, arg) = match cmd_text.split_once(char::is_whitespace) {
        Some((c, a)) => (c, a.trim()),
        None => (cmd_text, ""),
    };

    match cmd {
        "pwd" => send_text(to, from, &format!("📍 {}", cwd.display()), stdout),
        "cd" => {
            let result = cmd_cd(cwd, arg);
            send_text(to, from, &result, stdout);
            // After cd, auto ls
            let ls_result = cmd_ls(cwd, "");
            let ls_text = format!("📍 {}\n{}", cwd.display(), ls_result);
            send_text(to, from, &ls_text, stdout);
        }
        "ls" => {
            let list = cmd_ls_list(cwd, arg);
            if let Some((title, opts)) = list {
                send_list(to, from, "点选目录进入，点选文件查看：", &title, &opts, stdout);
            } else {
                send_text(to, from, "不是目录", stdout);
            }
        }
        "tree" => {
            let result = cmd_tree(cwd, arg);
            send_text(to, from, &result, stdout);
        }
        "cat" => {
            let result = cmd_cat(cwd, arg);
            send_text(to, from, &result, stdout);
        }
        "stat" => {
            let result = cmd_stat(cwd, arg);
            send_text(to, from, &result, stdout);
        }
        "find" => {
            let list = cmd_find_list(cwd, arg);
            if let Some((title, opts)) = list {
                send_list(to, from, "点选文件查看：", &title, &opts, stdout);
            } else {
                send_text(to, from, "未找到匹配的文件", stdout);
            }
        }
        "size" => {
            let result = cmd_size(cwd);
            send_text(to, from, &result, stdout);
        }
        _ => send_text(to, from, &format!("未知命令: /{cmd}\n输入 ? 查看可用命令"), stdout),
    }
}

// ---- Core commands ----

fn resolve_path(cwd: &Path, input: &str) -> PathBuf {
    let input = input.trim();
    if input.is_empty() {
        return cwd.to_path_buf();
    }
    if input.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            let rest = input.strip_prefix('~').unwrap_or("").trim_start_matches('/');
            let p = Path::new(&home).join(rest);
            return if p.is_absolute() { p } else { cwd.join(input) };
        }
    }
    let p = Path::new(input);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

fn cmd_cd(cwd: &mut PathBuf, arg: &str) -> String {
    let target = resolve_path(cwd, arg);
    if !target.exists() {
        return format!("路径不存在: {}", target.display());
    }
    if !target.is_dir() {
        return format!("不是目录: {}", target.display());
    }
    match target.canonicalize() {
        Ok(canon) => {
            *cwd = canon;
            format!("📍 {}", cwd.display())
        }
        Err(_) => {
            *cwd = target;
            format!("📍 {}", cwd.display())
        }
    }
}

fn cmd_ls(cwd: &Path, arg: &str) -> String {
    let target = resolve_path(cwd, arg);
    if !target.is_dir() {
        return "不是目录".into();
    }
    let entries = match fs::read_dir(&target) {
        Ok(e) => e,
        Err(e) => return format!("读取失败: {e}"),
    };
    let mut dirs: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        match entry.file_type() {
            Ok(t) if t.is_dir() => dirs.push(format!("📁 {name}")),
            _ => files.push(format!("📄 {name}")),
        }
    }
    dirs.sort();
    files.sort();
    let mut lines = vec![format!("({})", dirs.len() + files.len())];
    lines.extend(dirs);
    lines.extend(files);
    lines.join("\n")
}

fn cmd_ls_list(cwd: &Path, arg: &str) -> Option<(String, Vec<serde_json::Value>)> {
    let target = resolve_path(cwd, arg);
    if !target.is_dir() {
        return None;
    }
    let entries = match fs::read_dir(&target) {
        Ok(e) => e,
        Err(_) => return None,
    };
    let mut items: Vec<(bool, String)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        items.push((is_dir, name));
    }
    items.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

    let mut opts = Vec::new();
    for (is_dir, name) in &items {
        let full = target.join(name);
        let display_path = full.display().to_string();
        if *is_dir {
            opts.push(option(&format!("📁 {name}"), &format!("/cd {name}"), &display_path));
        } else {
            let size = fs::metadata(&full).ok().map(|m| m.len()).unwrap_or(0);
            let size_str = if size < 1024 {
                format!("{size} B")
            } else if size < 1024 * 1024 {
                format!("{:.1} KB", size as f64 / 1024.0)
            } else {
                format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
            };
            opts.push(option(&format!("📄 {name}"), &format!("/cat {name}"), &size_str));
        }
    }
    Some((format!("📂 {}", target.display()), opts))
}

fn cmd_tree(cwd: &Path, arg: &str) -> String {
    let target = resolve_path(cwd, arg);
    if !target.is_dir() {
        return format!("不是目录: {}", target.display());
    }
    let mut lines = Vec::new();
    lines.push(format!("🌳 {}", target.display()));
    build_tree(&target, "", 2, 0, &mut lines);
    lines.join("\n")
}

fn build_tree(path: &Path, prefix: &str, max_depth: usize, depth: usize, lines: &mut Vec<String>) {
    if depth >= max_depth {
        return;
    }
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut items: Vec<(bool, String)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        items.push((is_dir, name));
    }
    items.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for (i, (is_dir, name)) in items.iter().enumerate() {
        let is_last = i == items.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let icon = if *is_dir { "📁" } else { "📄" };
        lines.push(format!("{prefix}{connector}{icon} {name}"));
        if *is_dir {
            let next_prefix = if is_last { "    " } else { "│   " };
            build_tree(&path.join(name), &format!("{prefix}{next_prefix}"), max_depth, depth + 1, lines);
        }
    }
}

fn cmd_cat(cwd: &Path, arg: &str) -> String {
    if arg.is_empty() {
        return "用法: /cat <file>".into();
    }
    let target = resolve_path(cwd, arg);
    if !target.exists() {
        return format!("文件不存在: {}", target.display());
    }
    if !target.is_file() {
        return format!("不是文件: {}", target.display());
    }
    let content = match fs::read_to_string(&target) {
        Ok(c) => c,
        Err(e) => return format!("读取失败: {e}"),
    };
    let max_lines = 50;
    let lines: Vec<&str> = content.lines().collect();
    let ext = Path::new(arg).extension().and_then(|e| e.to_str()).unwrap_or("");
    if lines.len() <= max_lines {
        format!("```{}\n{}\n```", ext, content.trim_end())
    } else {
        let truncated: Vec<&str> = lines[..max_lines].to_vec();
        format!(
            "```{}\n{}\n```\n... (共 {} 行, 只显示前 {max_lines} 行)",
            ext, truncated.join("\n"), lines.len()
        )
    }
}

fn cmd_stat(cwd: &Path, arg: &str) -> String {
    let target = if arg.is_empty() {
        cwd.to_path_buf()
    } else {
        resolve_path(cwd, arg)
    };
    if !target.exists() {
        return format!("不存在: {}", target.display());
    }
    let metadata = match fs::metadata(&target) {
        Ok(m) => m,
        Err(e) => return format!("读取失败: {e}"),
    };
    let kind = if metadata.is_dir() {
        "📁 目录"
    } else if metadata.is_symlink() {
        "🔗 符号链接"
    } else {
        "📄 文件"
    };
    let size = metadata.len();
    let size_str = if size < 1024 {
        format!("{size} B")
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    };
    format!("{} {}\n大小: {size_str}", kind, target.display())
}

fn cmd_find_list(cwd: &Path, arg: &str) -> Option<(String, Vec<serde_json::Value>)> {
    if arg.is_empty() {
        return None;
    }
    let mut results = Vec::new();
    find_files(cwd, arg, 3, 0, &mut results);
    if results.is_empty() {
        return None;
    }
    results.sort();
    results.truncate(50);
    let opts: Vec<serde_json::Value> = results
        .iter()
        .map(|r| option(&format!("📄 {}", r), &format!("/cat {}", r), ""))
        .collect();
    Some((format!("🔍 搜索「{arg}」(共 {} 个)", results.len()), opts))
}

fn find_files(dir: &Path, query: &str, max_depth: usize, depth: usize, results: &mut Vec<String>) {
    if depth > max_depth {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            find_files(&path, query, max_depth, depth + 1, results);
        } else if name.to_lowercase().contains(&query.to_lowercase()) {
            if let Ok(rel) = path.strip_prefix(dir) {
                results.push(rel.display().to_string());
            }
        }
    }
}

fn cmd_size(cwd: &Path) -> String {
    let total = dir_size(cwd);
    let size_str = if total < 1024 {
        format!("{total} B")
    } else if total < 1024 * 1024 {
        format!("{:.1} KB", total as f64 / 1024.0)
    } else if total < 1024 * 1024 * 1024 {
        format!("{:.1} MB", total as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", total as f64 / (1024.0 * 1024.0 * 1024.0))
    };
    format!("📂 {} — {size_str}", cwd.display())
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                total += dir_size(&path);
            } else if let Ok(m) = entry.metadata() {
                total += m.len();
            }
        }
    }
    total
}

fn log(level: &str, msg: String) -> String {
    serde_json::json!({
        "method": "log",
        "params": { "level": level, "msg": msg }
    })
    .to_string()
}
