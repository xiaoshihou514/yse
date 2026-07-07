# file-tree 插件

YSE 文件浏览器插件。通过聊天命令浏览、查看文件。

## 构建

```sh
cd plugins/file-tree && cargo build
# 产物: plugins/file-tree/target/debug/file-tree
```

## 命令

| 命令          | 说明                 | 交互                                |
| ------------- | -------------------- | ----------------------------------- |
| `ls [path]`   | 列出目录             | 列表组件——点目录→`cd`，点文件→`cat` |
| `cd <path>`   | 切换目录             | 文字（自动跟 ls）                   |
| `pwd`         | 显示当前目录         | 文字                                |
| `tree [path]` | 树形展示（2 层）     | 文字                                |
| `cat <file>`  | 查看文件（前 50 行） | 文字                                |
| `stat <path>` | 文件详情             | 文字                                |
| `find <name>` | 递归搜索文件名       | 列表组件——点选→`cat`                |
| `size`        | 目录总大小           | 文字                                |
| `help`        | 帮助                 | 文字                                |

## 架构

### 通信协议

- stdin/stdout JSON-RPC 行协议（同 YSE 插件规范）
- 列表选择: 插件发 `send` + `meta.plugin.component`（`type: "list"`），用户点选后收到 `meta.plugin.response.value`
- `ls` 和 `find` 使用列表组件，其他命令用文字回复

### 状态

- 每个 `from`（用户地址）独立维护 CWD
- 首次启动 CWD = 插件进程当前目录
- 列表点选 `cd <name>` → 实际执行 `cd` 命令切换 CWD

### 依赖

- 仅 `serde_json`（无额外系统依赖）
