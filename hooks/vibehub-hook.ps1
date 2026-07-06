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
$toolResponse = $null
$errorMsg = ""
$agentId = ""
$agentType = ""
$isInterrupt = $false
$lastAssistantMsg = ""

try {
    if ($raw) {
        $data = $raw | ConvertFrom-Json
        $hookEvent = "$($data.hook_event_name)"
        if ($data.prompt)  { $task    = "$($data.prompt)" }
        if ($data.message) { $message = "$($data.message)" }
        if ($data.tool_name)     { $toolName     = "$($data.tool_name)" }
        if ($data.tool_input)    { $toolInput    = $data.tool_input }
        if ($data.tool_response) { $toolResponse = $data.tool_response }
        if ($data.error)         { $errorMsg     = "$($data.error)" }
        if ($data.agent_id)      { $agentId      = "$($data.agent_id)" }
        if ($data.agent_type)    { $agentType    = "$($data.agent_type)" }
        if ($data.is_interrupt)  { $isInterrupt  = [bool]$data.is_interrupt }
        # 提取最后一条 assistant 消息（Stop 事件时有用）
        if ($data.last_assistant_message) { $lastAssistantMsg = "$($data.last_assistant_message)" }
    }
} catch {}

# ============ 工具输入预览 ============
# 参考 Open Island 的 toolInputPreview：按优先级提取最 relevant 的字段。
$toolPreview = ""
if ($toolInput) {
    try {
        if ($toolInput.command)       { $toolPreview = "$($toolInput.command)" }
        elseif ($toolInput.file_path) { $toolPreview = "$($toolInput.file_path)" }
        elseif ($toolInput.path)      { $toolPreview = "$($toolInput.path)" }
        elseif ($toolInput.pattern)   { $toolPreview = "$($toolInput.pattern)" }
        elseif ($toolInput.query)     { $toolPreview = "$($toolInput.query)" }
        elseif ($toolInput.prompt)    { $toolPreview = "$($toolInput.prompt)" }
        elseif ($toolInput.url)       { $toolPreview = "$($toolInput.url)" }
    } catch {}
}
# 截断过长预览
if ($toolPreview.Length -gt 120) { $toolPreview = $toolPreview.Substring(0, 117) + "..." }

# ============ 工具响应预览 ============
$responsePreview = ""
if ($toolResponse) {
    try {
        $respStr = ""
        if ($toolResponse.output)      { $respStr = "$($toolResponse.output)" }
        elseif ($toolResponse.content) { $respStr = "$($toolResponse.content)" }
        elseif ($toolResponse.stdout)  { $respStr = "$($toolResponse.stdout)" }
        if ($respStr.Length -gt 120) { $respStr = $respStr.Substring(0, 117) + "..." }
        $responsePreview = $respStr
    } catch {}
}

# 写文件类工具集合，这些操作需要用户审批。
$writeTools = @("Write", "Edit", "MultiEdit", "Bash", "Computer")
$needsApproval = ($hookEvent -eq "PreToolUse") -and ($writeTools -contains $toolName)

# PermissionRequest 也需要审批
if ($hookEvent -eq "PermissionRequest") {
    $needsApproval = $true
    if ($message) {
        # PermissionRequest 自带 message，直接用
    } elseif ($toolName) {
        $message = "Allow $toolName ?"
    }
}

# 生成 decision_id（仅审批场景）。
$decisionId = ""
if ($needsApproval) {
    $decisionId = [System.Guid]::NewGuid().ToString("N").Substring(0, 12)
}

# Claude hook 事件名 -> VibeHub event_type
# 保持四种前端状态：running / needs_input / completed / idle
# 通过附加字段区分具体语义。
$eventType = "running"
switch -Wildcard ($hookEvent) {
    "SessionStart"       { $eventType = "running" }
    "UserPromptSubmit"   { $eventType = "running" }
    "PreToolUse" {
        if ($needsApproval) {
            $eventType = "needs_input"
            # 构造审批提示
            $filePath = ""
            if ($toolInput -and $toolInput.path)         { $filePath = $toolInput.path }
            elseif ($toolInput -and $toolInput.file_path){ $filePath = $toolInput.file_path }
            if ($filePath) {
                $message = "Allow $toolName -> $filePath ?"
            } elseif (-not $message) {
                $message = "Allow $toolName ?"
            }
        } else {
            $eventType = "running"
        }
    }
    "PostToolUse"        { $eventType = "running" }
    "PostToolUseFailure" { $eventType = "error"; if ($errorMsg) { $message = $errorMsg } }
    "PermissionRequest"  {
        $eventType = "needs_input"
    }
    "PermissionDenied"   { $eventType = "running" }
    "Notification" {
        if ($message -and $message.Length -gt 0) {
            $eventType = "needs_input"
        } else {
            $eventType = "running"
        }
    }
    "Stop" {
        $eventType = "completed"
        if ($lastAssistantMsg) {
            $message = $lastAssistantMsg
        } else {
            $message = "Done"
        }
    }
    "StopFailure"  { $eventType = "error"; if ($errorMsg) { $message = $errorMsg } else { $message = "Stop failed" } }
    "SubagentStart" { $eventType = "running" }
    "SubagentStop"  { $eventType = "running" }
    "PreCompact"    { $eventType = "running" }
    "SessionEnd"    { $eventType = "idle" }
    default         { $eventType = "running" }
}

# 截断过长任务摘要。
if ($task.Length -gt 60) { $task = $task.Substring(0, 57) + "..." }
# 截断过长消息。
if ($message.Length -gt 200) { $message = $message.Substring(0, 197) + "..." }

$payload = @{
    agent_id   = "claude"
    agent_name = "Claude"
    event_type = $eventType
}
# 仅在非空时添加可选字段，避免覆盖 UI 已显示的内容。
if ($task)        { $payload["task"]        = $task }
if ($message)     { $payload["message"]     = $message }
if ($decisionId)  { $payload["decision_id"] = $decisionId }

# 新增字段：全活动监控上下文
if ($hookEvent)       { $payload["hook_event_name"]   = $hookEvent }
if ($toolName)        { $payload["tool_name"]         = $toolName }
if ($toolPreview)     { $payload["tool_preview"]      = $toolPreview }
if ($responsePreview) { $payload["response_preview"]  = $responsePreview }
if ($errorMsg)        { $payload["error"]             = $errorMsg }
if ($lastAssistantMsg -and $hookEvent -ne "Stop") {
    # Stop 事件的 lastAssistantMsg 已放入 message，此处只转发非 Stop 场景
    $payload["last_message"] = $lastAssistantMsg
}
if ($agentType)       { $payload["agent_type"]        = $agentType }
if ($isInterrupt)     { $payload["is_interrupt"]      = $true }

try {
    Invoke-RestMethod -Uri "$baseUrl/event" -Method Post `
        -Body ($payload | ConvertTo-Json -Compress) `
        -ContentType "application/json" -TimeoutSec 2 | Out-Null
} catch {
    # VibeHub 未运行，直接允许
    exit 0
}

# 审批场景：轮询 GET /decision/{id}，最多等 120 秒。
if ($needsApproval -and $decisionId) {
    $pollUrl = "$baseUrl/decision/$decisionId"
    $deadline = (Get-Date).AddSeconds(120)

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
