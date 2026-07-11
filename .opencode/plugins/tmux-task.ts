import type { Plugin, ToolContext } from "@opencode-ai/plugin"
import { tool } from "@opencode-ai/plugin"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { join } from "node:path"

/* ===== Types ===== */

interface Task {
  id: string
  tmuxSession: string
  opencodeSessionID: string
  description: string
  command: string
  server?: string
  lastLine: string
  lastPct: number | null
  stalledCount: number
  createdAt: string
  lastCheckAt: string
  status: "running" | "completed" | "stalled" | "error"
  completedAt?: string
  notifiedPct?: number | null
  notifiedLine?: string
}

interface PersistedData {
  tasks: Record<string, Task>
}

/* ===== Constants ===== */

const CHECK_INTERVAL_MS = 15 * 1000
const NOTIFY_THRESHOLD = 5
const STALL_LIMIT = 6
const PERSISTENCE_DIR = ".opencode"

/* ===== Top-level Helpers (no plugin types needed) ===== */

function taskID(): string {
  return "yse-" + Math.random().toString(36).slice(2, 8)
}

function parseProgress(line: string): { pct: number | null; raw: string | null } {
  const withBar = line.match(/^\s*(\d+)%\s*\|.*?\|\s*(\d+)\/(\d+)\s*\[/)
  if (withBar) {
    const pct = Math.round((parseInt(withBar[2]) / parseInt(withBar[3])) * 100)
    return { pct, raw: `${withBar[2]}/${withBar[3]}` }
  }
  const noBar = line.match(/(\d+)\/(\d+)\s+\[[\d:]+\<[\d:]+/)
  if (noBar) {
    const pct = Math.round((parseInt(noBar[1]) / parseInt(noBar[2])) * 100)
    return { pct, raw: `${noBar[1]}/${noBar[2]}` }
  }
  const pctOnly = line.match(/(\d+)%/)
  if (pctOnly) return { pct: parseInt(pctOnly[1]), raw: pctOnly[0] }
  const counter = line.match(/(\d+)\/(\d+)/)
  if (counter) {
    const pct = Math.round((parseInt(counter[1]) / parseInt(counter[2])) * 100)
    return { pct, raw: `${counter[1]}/${counter[2]}` }
  }
  return { pct: null, raw: null }
}

function persistencePath(directory: string): string {
  return join(directory, PERSISTENCE_DIR, "tmux-tasks.json")
}

async function loadTasks(directory: string): Promise<Record<string, Task>> {
  try {
    const data = await readFile(persistencePath(directory), "utf-8")
    const parsed: PersistedData = JSON.parse(data)
    return parsed.tasks || {}
  } catch { return {} }
}

async function saveTasks(directory: string, tasks: Record<string, Task>): Promise<void> {
  const dir = join(directory, PERSISTENCE_DIR)
  await mkdir(dir, { recursive: true })
  await writeFile(persistencePath(directory), JSON.stringify({ tasks } satisfies PersistedData, null, 2))
}

/* ===== Plugin ===== */

export const TmuxPlugin: Plugin = async ({ client, $, directory }) => {
  /* ------ helpers (capture plugin types from closure) ------ */

  const tmuxCapture = async (session: string): Promise<string> => {
    const out = await $`tmux capture-pane -t ${session} -p`.quiet().nothrow().text()
    return out.trim()
  }

  const tmuxHasSession = async (session: string): Promise<boolean> => {
    try {
      await $`tmux has-session -t ${session}`.quiet().nothrow()
      return true
    } catch { return false }
  }

  const tmuxSendKeys = async (session: string, keys: string): Promise<void> => {
    await $`tmux send-keys -t ${session} ${keys}`.quiet().nothrow()
  }

  const tmuxSendKeysEnter = async (session: string, cmd: string): Promise<void> => {
    await $`tmux send-keys -t ${session} ${cmd} Enter`.quiet().nothrow()
  }

  const notifySession = async (
    sessionID: string, message: string, noReply: boolean,
  ): Promise<void> => {
    try {
      await client.session.prompt({
        path: { id: sessionID },
        body: { parts: [{ type: "text", text: message }], noReply },
      })
    } catch (e) {
      console.error("[tmux-task] notifySession failed:", e)
    }
  }

  /* ------ background check ------ */

  const checkAllTasks = async (tasks: Record<string, Task>): Promise<void> => {
    for (const task of Object.values(tasks)) {
      if (task.status !== "running") continue

      try {
        const alive = await tmuxHasSession(task.tmuxSession)
        if (!alive) {
          task.status = "completed"
          task.completedAt = new Date().toISOString()
          await notifySession(task.opencodeSessionID, "✅ 任务已结束（tmux session 已关闭）", false)
          await saveTasks(directory, tasks)
          continue
        }

        const output = await tmuxCapture(task.tmuxSession)
        const lines = output.split("\n").filter(Boolean)
        const lastLine = lines.length > 0 ? lines[lines.length - 1] : task.lastLine
        const { pct, raw } = parseProgress(lastLine)

        const prevPct = task.lastPct
        const prevLine = task.lastLine

        task.lastLine = lastLine
        task.lastCheckAt = new Date().toISOString()

        let shouldNotify = false
        let message = ""

        if (pct !== null) {
          if (prevPct !== null && pct === prevPct) {
            task.stalledCount++
          } else {
            task.stalledCount = 0
          }
          task.lastPct = pct

          const notifiedPct = task.notifiedPct ?? -100

          if (pct >= 100) {
            shouldNotify = true
            message = `🎉 任务完成: ${pct}%${raw ? ` (${raw})` : ""}`
            task.status = "completed"
            task.completedAt = new Date().toISOString()
          } else if (task.stalledCount >= STALL_LIMIT) {
            shouldNotify = true
            message = `⚠️ 进度卡住: ${raw || lastLine.slice(0, 200)}`
          } else if (pct - notifiedPct >= NOTIFY_THRESHOLD) {
            shouldNotify = true
            message = `📊 ${raw || lastLine.slice(0, 200)} (${pct}%)`
          }

          if (shouldNotify) task.notifiedPct = pct
        } else if (lastLine !== prevLine && lastLine !== task.notifiedLine) {
          shouldNotify = true
          message = `📋 ${lastLine.slice(0, 200)}`
          task.notifiedLine = lastLine
        }

        if (shouldNotify) {
          await notifySession(task.opencodeSessionID, message, task.status !== "running")
        }

        await saveTasks(directory, tasks)
      } catch (e) {
        console.error(`[tmux-task] check failed for ${task.id}:`, e)
      }
    }
  }

  /* ------ load persisted tasks & resume monitoring ------ */

  const tasks = await loadTasks(directory)

  let resumed = 0
  for (const task of Object.values(tasks)) {
    if (task.status === "running") {
      const alive = await tmuxHasSession(task.tmuxSession)
      if (alive) {
        resumed++
      } else {
        task.status = "completed"
        task.completedAt = new Date().toISOString()
      }
    }
  }
  await saveTasks(directory, tasks)

  if (resumed > 0 || Object.keys(tasks).length > 0) {
    const running = Object.values(tasks).filter(t => t.status === "running").length
    console.log(`[tmux-task] 已恢复 ${resumed}/${running} 个活跃任务，共 ${Object.keys(tasks).length} 个已注册`)
  }

  /* ------ start background timer ------ */

  const checkInterval = setInterval(() => {
    checkAllTasks(tasks).catch(e =>
      console.error("[tmux-task] checkAllTasks error:", e),
    )
  }, CHECK_INTERVAL_MS)

  /* ------ tools ------ */

  return {
    dispose: async () => {
      clearInterval(checkInterval)
      await saveTasks(directory, tasks)
      console.log("[tmux-task] 已停止")
    },
    tool: {
      "tmux-start": tool({
        description: `在后台 tmux session 中启动一个长期任务（如 ML 训练），
自动打开 kitty 窗口（如有 DISPLAY），
每 15 秒自动检查进度，进度变化超过 5% 时回调到当前会话。`,
        args: {
          command: tool.schema.string().describe("要在 tmux 中执行的命令，例如 'python train.py --epochs 100'"),
          description: tool.schema.string().describe("任务简短描述，用于显示和子会话标题"),
          server: tool.schema.string().optional().describe("SSH 目标主机（已配密钥），自动拼接为 ssh <server> '<command>'"),
        },
        async execute(args, ctx: ToolContext) {
          const { command, description, server } = args
          const id = taskID()
          const cmd = server
            ? `ssh ${server} '${command.replace(/'/g, "'\\''")}'`
            : command

          await $`tmux new-session -d -s ${id}`.quiet().nothrow()
          await tmuxSendKeysEnter(id, cmd)

          const display = process.env.DISPLAY
          if (display) {
            $`kitty --detach tmux attach -t ${id}`.quiet().nothrow().catch(() => {})
          }

          const task: Task = {
            id,
            tmuxSession: id,
            opencodeSessionID: ctx.sessionID,
            description,
            command: cmd,
            server,
            lastLine: "",
            lastPct: null,
            stalledCount: 0,
            createdAt: new Date().toISOString(),
            lastCheckAt: new Date().toISOString(),
            status: "running",
          }
          tasks[id] = task
          await saveTasks(directory, tasks)

          return {
            output: [
              `✅ 任务已启动`,
              `   tmux session: ${id}`,
              `   描述: ${description}`,
              `   命令: ${cmd}`,
              !display ? "   ⚠️ 无 DISPLAY，未开 kitty 窗口（后台运行）" : "",
            ].filter(Boolean).join("\n"),
            metadata: { taskID: id },
          }
        },
      }),

      "tmux-status": tool({
        description: "查看指定 tmux 任务的当前状态和进度",
        args: {
          taskID: tool.schema.string().describe("任务 ID"),
        },
        async execute(args) {
          const task = tasks[args.taskID]
          if (!task) throw new Error(`任务 ${args.taskID} 不存在`)

          const alive = await tmuxHasSession(task.tmuxSession)
          const output = alive ? await tmuxCapture(task.tmuxSession) : "(session ended)"
          const lines = output.split("\n").filter(Boolean)
          const lastLine = lines.length > 0 ? lines[lines.length - 1] : "(empty)"

          return {
            output: [
              `📋 任务: ${task.description}`,
              `   状态: ${task.status}`,
              `   tmux session: ${task.tmuxSession} (${alive ? "存活" : "已结束"})`,
              `   命令: ${task.command}`,
              `   最新行: ${lastLine.slice(0, 500)}`,
              task.lastPct !== null ? `   进度: ${task.lastPct}%` : "",
              task.stalledCount > 0 ? `   卡住次数: ${task.stalledCount}` : "",
            ].filter(Boolean).join("\n"),
          }
        },
      }),

      "tmux-list": tool({
        description: "列出所有已注册的 tmux 任务",
        args: {},
        async execute() {
          const entries = Object.values(tasks)
          if (entries.length === 0) return { output: "暂无任务" }

          const lines = entries.map(t =>
            `  ${t.id} | ${t.status.padEnd(10)} | ${t.description.slice(0, 40)}`,
          )

          return { output: `📋 共 ${entries.length} 个任务\n${lines.join("\n")}` }
        },
      }),

      "tmux-intervene": tool({
        description: `向 tmux pane 发送按键以干预运行中的任务。
支持 tmux 按键名（C-c = Ctrl+C, Enter, Escape, C-d 等）。
如 command 非空，会先发中断按键再发新命令。`,
        args: {
          taskID: tool.schema.string().describe("任务 ID"),
          keys: tool.schema.string().describe("tmux 按键名，如 C-c（Ctrl+C），可多个空格分隔"),
          command: tool.schema.string().optional().describe("中断后输入的新命令（可选）"),
        },
        async execute(args) {
          const task = tasks[args.taskID]
          if (!task) throw new Error(`任务 ${args.taskID} 不存在`)

          const alive = await tmuxHasSession(task.tmuxSession)
          if (!alive) throw new Error(`任务 ${args.taskID} 的 tmux session 已结束`)

          await tmuxSendKeys(task.tmuxSession, args.keys)

          if (args.command) {
            await new Promise(r => setTimeout(r, 300))
            await tmuxSendKeysEnter(task.tmuxSession, args.command)
          }

          await new Promise(r => setTimeout(r, 500))
          const output = await tmuxCapture(task.tmuxSession)
          const lines = output.split("\n").filter(Boolean)
          const lastLine = lines.length > 0 ? lines[lines.length - 1] : ""

          task.lastLine = lastLine
          task.stalledCount = 0
          if (args.command) task.status = "running"
          await saveTasks(directory, tasks)

          return {
            output: `已发送按键 ${args.keys}${args.command ? ` 并启动新命令` : ""}\n最新行: ${lastLine.slice(0, 300)}`,
          }
        },
      }),

      "tmux-stop": tool({
        description: "停止监控指定任务（不 kill tmux session，只清理注册表）",
        args: {
          taskID: tool.schema.string().describe("任务 ID"),
          kill: tool.schema.boolean().optional().describe("设为 true 同时 kill tmux session"),
        },
        async execute(args) {
          const task = tasks[args.taskID]
          if (!task) throw new Error(`任务 ${args.taskID} 不存在`)

          if (args.kill) {
            await $`tmux kill-session -t ${task.tmuxSession}`.quiet().nothrow()
          }

          task.status = "completed"
          task.completedAt = new Date().toISOString()
          await saveTasks(directory, tasks)

          return {
            output: `已停止监控 ${args.taskID}${args.kill ? "（已 kill tmux session）" : ""}`,
          }
        },
      }),

      "tmux-smoke": tool({
        description: "验证 tmux-task 插件是否正常工作：检查 tmux、kitty、持久化等",
        args: {},
        async execute() {
          const checks: string[] = []

          try {
            const v = await $`tmux -V`.quiet().text()
            checks.push(`✅ tmux: ${v.trim()}`)
          } catch {
            checks.push("❌ tmux: 未安装或不在 PATH")
          }

          try {
            const v = await $`kitty --version`.quiet().text()
            checks.push(`✅ kitty: ${v.trim()}`)
          } catch {
            checks.push("❌ kitty: 未安装")
          }

          checks.push(process.env.DISPLAY
            ? `✅ DISPLAY: ${process.env.DISPLAY}`
            : "⚠️ 无 DISPLAY（不会开窗口）")

          try {
            const out = await $`ssh-add -l 2>/dev/null || echo no keys`.quiet().text()
            checks.push(out.trim() !== "no keys"
              ? "✅ SSH agent: 有密钥"
              : "⚠️ SSH agent: 无密钥（SSH 需手动/免密配置）")
          } catch {
            checks.push("⚠️ SSH agent: 不可用")
          }

          const persisted = Object.keys(tasks).length
          checks.push(persisted > 0
            ? `📁 持久化: ${persisted} 个已注册任务`
            : "📁 持久化: 空")

          const running = Object.values(tasks).filter(t => t.status === "running").length
          checks.push(running > 0
            ? `🔄 活跃任务: ${running} 个`
            : "🔄 活跃任务: 无")

          return { output: checks.join("\n") }
        },
      }),
    },
  }
}
