# VibeHub Claude Code Hook
# 由 Claude Code 在事件触发时调用，stdin 收到事件 JSON。
# 将事件映射为 VibeHub event_type 并 POST 到本地服务。
# 失败时静默退出，绝不阻塞 Claude 本身。

$ErrorActionPreference = "SilentlyContinue"

$port = 51789
$url = "http://127.0.0.1:$port/event"

# 读取 stdin（Claude 传入的事件 JSON）
$raw = [Console]::In.ReadToEnd()

$hookEvent = ""
$task = ""
$message = ""
try {
    if ($raw) {
        $data = $raw | ConvertFrom-Json
        $hookEvent = "$($data.hook_event_name)"
        # prompt（UserPromptSubmit）作为任务摘要
        if ($data.prompt) { $task = "$($data.prompt)" }
        if ($data.message) { $message = "$($data.message)" }
    }
} catch {}

# Claude hook 事件名 -> VibeHub event_type
switch -Wildcard ($hookEvent) {
    "SessionStart"      { $eventType = "running" }
    "UserPromptSubmit"  { $eventType = "running" }
    "PreToolUse"        { $eventType = "running" }
    "Notification"      { $eventType = "needs_input" }
    "Stop"              { $eventType = "completed"; $message = "Done" }
    "SubagentStop"      { $eventType = "completed"; $message = "Subagent done" }
    "SessionEnd"        { $eventType = "idle" }
    default             { $eventType = "running" }
}

# 截断过长任务摘要
if ($task.Length -gt 60) { $task = $task.Substring(0, 57) + "..." }

$payload = @{
    agent_id   = "claude"
    agent_name = "Claude"
    event_type = $eventType
    task       = $task
    message    = $message
} | ConvertTo-Json -Compress

try {
    Invoke-RestMethod -Uri $url -Method Post -Body $payload -ContentType "application/json" -TimeoutSec 2 | Out-Null
} catch {}

# 总是成功退出，不影响 Claude
exit 0
