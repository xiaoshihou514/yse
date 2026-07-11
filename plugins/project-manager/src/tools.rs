use std::path::PathBuf;

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ToolError(pub String);

// ---- read_file ----

#[derive(Deserialize)]
pub struct ReadFileArgs {
    pub path: String,
}

pub struct ReadFileTool {
    pub project_dir: PathBuf,
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";

    type Error = ToolError;
    type Args = ReadFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        serde_json::from_value(serde_json::json!({
            "name": "read_file",
            "description": "读取项目文件内容。返回文件的前 8000 字符。",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "相对于项目根目录的文件路径"
                    }
                },
                "required": ["path"]
            }
        }))
        .unwrap()
    }

    async fn call(&self, args: Self::Args) -> Result<String, Self::Error> {
        let path = self.project_dir.join(&args.path);
        let canonical = path.canonicalize().map_err(|e| ToolError(e.to_string()))?;
        if !canonical.starts_with(&self.project_dir) {
            return Err(ToolError("路径超出项目目录".into()));
        }
        let content = tokio::fs::read_to_string(&canonical)
            .await
            .map_err(|e| ToolError(e.to_string()))?;
        let truncated: String = content.chars().take(8000).collect();
        Ok(truncated)
    }
}

// ---- list_directory ----

#[derive(Deserialize)]
pub struct ListDirArgs {
    pub path: String,
}

pub struct ListDirTool {
    pub project_dir: PathBuf,
}

impl Tool for ListDirTool {
    const NAME: &'static str = "list_directory";

    type Error = ToolError;
    type Args = ListDirArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        serde_json::from_value(serde_json::json!({
            "name": "list_directory",
            "description": "列出目录中的文件和子目录。返回排序后的条目列表。",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "相对于项目根目录的目录路径，例如 \".\" 表示根目录"
                    }
                },
                "required": ["path"]
            }
        }))
        .unwrap()
    }

    async fn call(&self, args: Self::Args) -> Result<String, Self::Error> {
        let path = self.project_dir.join(&args.path);
        let canonical = path.canonicalize().map_err(|e| ToolError(e.to_string()))?;
        if !canonical.starts_with(&self.project_dir) {
            return Err(ToolError("路径超出项目目录".into()));
        }
        let mut entries = tokio::fs::read_dir(&canonical)
            .await
            .map_err(|e| ToolError(e.to_string()))?;
        let mut names = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ToolError(e.to_string()))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            names.push(name);
        }
        names.sort();
        Ok(names.join("\n"))
    }
}

// ---- git_log ----

#[derive(Deserialize)]
pub struct GitLogArgs {
    pub count: u32,
}

pub struct GitLogTool {
    pub project_dir: PathBuf,
}

impl Tool for GitLogTool {
    const NAME: &'static str = "git_log";

    type Error = ToolError;
    type Args = GitLogArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        serde_json::from_value(serde_json::json!({
            "name": "git_log",
            "description": "查看项目的 git 提交历史。返回最近 N 条提交。",
            "parameters": {
                "type": "object",
                "properties": {
                    "count": {
                        "type": "number",
                        "description": "要获取的提交数量"
                    }
                },
                "required": ["count"]
            }
        }))
        .unwrap()
    }

    async fn call(&self, args: Self::Args) -> Result<String, Self::Error> {
        let output = tokio::process::Command::new("git")
            .args([
                "log",
                "--oneline",
                &format!("-{}", args.count.clamp(1, 100)),
            ])
            .current_dir(&self.project_dir)
            .output()
            .await
            .map_err(|e| ToolError(e.to_string()))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !output.status.success() {
            return Err(ToolError(format!("git log 失败: {}", stderr)));
        }
        Ok(stdout)
    }
}
