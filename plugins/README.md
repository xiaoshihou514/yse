# 盐水鹅 插件开发指南

## 概述

盐水鹅 插件是独立的子进程，通过 stdin/stdout JSON-RPC 行协议与 Core 通信。

## 通信协议

每行一个 JSON 对象。Core → 插件（stdin）为通知，插件 → Core（stdout）为请求。

### Core → 插件（通知）

```json
{"method":"message","params":{"from":"name#hash@hostname","to":"name#hash@hostname","text":"...","meta":{...},"files":[...]}}
{"method":"config","params":{"key":"...","value":...}}
{"method":"shutdown","params":{}}
```

### 插件 → Core（请求）

```json
{"method":"send","params":{"from":"...","to":"...","text":"...","meta":{...},"files":[...]},"id":1}
{"method":"log","params":{"level":"info","msg":"..."}}
```

## 消息文本支持 Markdown

`text` 字段使用 GitHub Flavored Markdown 渲染。支持：

| 语法 | 效果 |
|------|------|
| `# 标题` | 标题 |
| `**粗体**` `*斜体*` `~~删除线~~` | 文本样式 |
| `` `代码` ``  ```` ```rust ```` | 内联/块级代码（highlight.js 高亮）|
| `- 列表` `1. 列表` | 无序/有序列表 |
| `[链接](url)` | 超链接 |
| `\| 表头 \|` | 表格 |
| `> 引用` | 引用块 |
| `- [x] 任务` | 任务列表 |

示例：
```rust
let text = "# 日报\n\n| 项目 | 状态 |\n|------|------|\n| 开发 | ✅ 完成 |\n| 测试 | ⏳ 进行中 |\n\n> 由自动报表插件生成";
```

## 插件 UI 组件

通过 `meta.plugin.component` 字段定义交互组件。

### 列表选择（List）

让用户从一组选项中选取。

```json
{
  "meta": {
    "plugin": {
      "component": {
        "type": "list",
        "id": "ticket-category",
        "title": "请选择工单类型",
        "options": [
          { "label": "技术支持", "value": "tech", "description": "系统故障或使用问题" },
          { "label": "账单", "value": "billing", "description": "费用与发票" },
          { "label": "其他", "value": "other" }
        ]
      }
    }
  }
}
```

用户点击选项后，前端发送回复消息，`meta.plugin.response` 带回所选值：

```json
{
  "text": "[ticket-category] tech",
  "meta": {
    "plugin": {
      "response": { "component_id": "ticket-category", "value": "tech" }
    }
  }
}
```

### 进度条（Progress）

展示长时间运行任务的进度。

```json
{
  "meta": {
    "plugin": {
      "component": {
        "type": "progress",
        "id": "export-job-001",
        "value": 45,
        "max": 100,
        "status": "正在导出第 45/100 条记录..."
      }
    }
  }
}
```

`value` 和 `status` 可以通过后续消息更新（相同 `id` 的可选，前端会单独渲染每条消息）。

## 插件配置（plugin.toml）

```toml
[plugin]
id = "my-plugin"
name = "我的插件"
version = "0.1.0"
exec = "my-plugin"          # 可执行文件名（需在 PATH 或指定绝对路径）
description = "插件说明"
```

## 完整示例

见 [`echo-bot/`](./echo-bot/) —— 一个简单的回声插件。

关键点：
1. `shutdown` 通知收到后应退出循环，以便 Core 正常终止进程
2. `config` 通知可以忽略或用于热更新配置
3. `send` 请求中的 `from` 应设为消息的 `to`（即插件地址），`to` 设为消息的 `from`（即发件人）
4. 日志通过 `log` 请求发送，由 Core 统一管理
5. `meta` 字段会原样透传，插件可在此存放上下文数据
