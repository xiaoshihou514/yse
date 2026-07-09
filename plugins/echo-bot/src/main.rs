use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = val["method"].as_str().unwrap_or("");

        match method {
            "message" => {
                let params = &val["params"];
                let from = params["from"].as_str().unwrap_or("unknown");
                let _to = params["to"].as_str().unwrap_or("");
                let text = params["text"].as_str().unwrap_or("");

                let help = text == "?" || text == "？";
                let reply = if help {
                    "🔊 回声机器人\n\
                     \n\
                     发送任意文本，机器人会原样回复。\n\
                     \n\
                     可用命令：\n\
                     /help  — 此帮助"
                } else {
                    &format!("Echo: {}", text)
                };

                let mut response = serde_json::json!({
                    "method": "send",
                    "params": {
                        "from": _to,
                        "to": from,
                        "text": reply
                    },
                    "id": 1
                });

                if let Some(meta) = params["meta"].as_object() {
                    if !meta.is_empty() {
                        response["params"]["meta"] = params["meta"].clone();
                    }
                }

                let output = serde_json::to_string(&response).unwrap();
                writeln!(stdout, "{}", output).ok();
                stdout.flush().ok();

                // Log
                let log = serde_json::json!({
                    "method": "log",
                    "params": {
                        "level": "info",
                        "msg": format!("echoed message from {}", from)
                    }
                });
                let log_line = serde_json::to_string(&log).unwrap();
                writeln!(stdout, "{}", log_line).ok();
                stdout.flush().ok();
            }
            "config" => {
                // no-op for echo bot
            }
            "shutdown" => {
                break;
            }
            _ => {}
        }
    }
}
