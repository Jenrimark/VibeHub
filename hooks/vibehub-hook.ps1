# VibeHub Claude Code Hook
# 由 Claude Code 在事件触发时调用，stdin 收到事件 JSON。
# 将事件映射为 VibeHub event_type 并 POST 到本地服务。
# PreToolUse 写操作时阻塞等待用户审批，exit 0=允许 exit 2=拒绝。
# 失败时静默退出，绝不阻塞 Claude 本身。

$ErrorActionPreference = "SilentlyContinue"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$port = 51789
$baseUrl = "http://127.0.0.1:$port"

# 读取 stdin（Claude 传入的事件 JSON）
$raw = [Console]::In.ReadToEnd()

$hookEvent = ""
$task = ""
$message = ""
$toolName = ""
$toolInput = $null

try {
    if ($raw) {
        $data = $raw | ConvertFrom-Json
        $hookEvent = "$($data.hook_event_name)"
        if ($data.prompt)  { $task    = "$($data.prompt)" }
        if ($data.message) { $message = "$($data.message)" }
        if ($data.tool_name)  { $toolName  = "$($data.tool_name)" }
        if ($data.tool_input) { $toolInput = $data.tool_input }
    }
} catch {}

# 写文件类工具集合，这些操作需要用户审批。
$writeTools = @("Write", "Edit", "MultiEdit", "Bash", "Computer")
$needsApproval = ($hookEvent -eq "PreToolUse") -and ($writeTools -contains $toolName)

# 生成 decision_id（仅审批场景）。
$decisionId = ""
if ($needsApproval) {
    $decisionId = [System.Guid]::NewGuid().ToString("N").Substring(0, 12)
}

# Claude hook 事件名 -> VibeHub event_type
$eventType = "running"
switch -Wildcard ($hookEvent) {
    "SessionStart"     { $eventType = "running" }
    "UserPromptSubmit" { $eventType = "running" }
    "PreToolUse" {
        if ($needsApproval) {
            $eventType = "needs_input"
            # 构造审批提示
            $filePath = ""
            if ($toolInput -and $toolInput.path)         { $filePath = $toolInput.path }
            elseif ($toolInput -and $toolInput.file_path){ $filePath = $toolInput.file_path }
            if ($filePath) {
                $message = "Allow $toolName -> $filePath ?"
            } else {
                $message = "Allow $toolName ?"
            }
        } else {
            $eventType = "running"
        }
    }
    "PostToolUse"  { $eventType = "running" }
    "Notification" {
        if ($message -and $message.Length -gt 0) {
            $eventType = "needs_input"
        } else {
            $eventType = "running"
        }
    }
    "Stop"         { $eventType = "completed"; $message = "Done" }
    "SubagentStop" { $eventType = "completed"; $message = "Subagent done" }
    "SessionEnd"   { $eventType = "idle" }
    default        { $eventType = "running" }
}

# 截断过长任务摘要。
if ($task.Length -gt 60) { $task = $task.Substring(0, 57) + "..." }

$payload = @{
    agent_id   = "claude"
    agent_name = "Claude"
    event_type = $eventType
    task       = $task
    message    = $message
}
if ($decisionId) { $payload["decision_id"] = $decisionId }

try {
    Invoke-RestMethod -Uri "$baseUrl/event" -Method Post `
        -Body ($payload | ConvertTo-Json -Compress) `
        -ContentType "application/json" -TimeoutSec 2 | Out-Null
} catch {
    # VibeHub 未运行，直接允许
    exit 0
}

# 审批场景：轮询 GET /decision/{id}，最多等 60 秒。
if ($needsApproval -and $decisionId) {
    $pollUrl = "$baseUrl/decision/$decisionId"
    $deadline = (Get-Date).AddSeconds(60)

    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 500
        try {
            $resp = Invoke-RestMethod -Uri $pollUrl -Method Get -TimeoutSec 2
            $d = $resp.decision
            if ($d -eq "allowed") { exit 0 }
            if ($d -eq "denied")  { exit 2 }
            # pending -> 继续等待
        } catch {
            # 网络错误默认允许，不阻塞 Claude
            exit 0
        }
    }

    # 超时默认允许
    exit 0
}

exit 0
