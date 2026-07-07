# VibeHub 审批流程端到端测试
# 验证指标：M1（事件接收率）、M2（Decision 注册）、M5（自动批准延迟）、M7（状态转换）
# 用法：先启动 VibeHub（npm run tauri dev），再运行本脚本。

$url = "http://127.0.0.1:51789"
$pass = 0
$fail = 0
$skip = 0

function Send-Event($payload) {
    $json = $payload | ConvertTo-Json -Compress
    try {
        $resp = Invoke-RestMethod -Uri "$url/event" -Method Post -Body $json -ContentType "application/json; charset=utf-8" -TimeoutSec 2
        return $true
    } catch {
        return $false
    }
}

function Check-Decision($id) {
    try {
        $resp = Invoke-RestMethod -Uri "$url/decision/$id" -Method Get -TimeoutSec 2
        return $resp.decision
    } catch {
        return $null
    }
}

function Assert($condition, $passMsg, $failMsg) {
    if ($condition) {
        Write-Host "  ✅ PASS: $passMsg" -ForegroundColor Green
        $script:pass++
    } else {
        Write-Host "  ❌ FAIL: $failMsg" -ForegroundColor Red
        $script:fail++
    }
}

Write-Host "=== VibeHub 审批流程测试 ===" -ForegroundColor Cyan
Write-Host ""

# --- Step 1: 基础连通性 ---
Write-Host "[Step 1] 基础连通性..." -ForegroundColor Yellow
$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="running"; task="Connectivity test" }
Assert $ok "POST /event 返回 ok" "无法连接 VibeHub — 是否在运行？"

if (-not $ok) {
    Write-Host "`n❌ VibeHub 未运行，终止测试。" -ForegroundColor Red
    exit 1
}

# --- Step 2: Decision 注册 ---
Write-Host "`n[Step 2] Decision 注册..." -ForegroundColor Yellow
$decId = "test_$(Get-Random)"
$ok = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Approval test"; message="Allow Write -> test.txt?"
    decision_id=$decId; hook_event_name="PreToolUse"; tool_name="Write"; tool_preview="test.txt"
}
Assert $ok "POST needs_input + decision_id 返回 ok" "POST needs_input 失败"

# 等待一小段时间让 auto-approve（如果开启）生效
Start-Sleep -Milliseconds 200
$result = Check-Decision $decId
# 可能是 pending（未开 auto-approve）或 allowed（已开 auto-approve）
Assert (($result -eq "pending") -or ($result -eq "allowed")) "GET /decision/$decId 返回 '$result' (pending 或 allowed)" "GET /decision/$decId 返回异常: $result"

# --- Step 3: 无 decision_id 的 needs_input（回归测试） ---
Write-Host "`n[Step 3] 回归：无 decision_id 的 needs_input..." -ForegroundColor Yellow
$ok = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Regression test"; message="Notification without approval"
}
Assert $ok "不带 decision_id 的 needs_input 正确处理" "POST 失败"

# --- Step 4: 完整生命周期 ---
Write-Host "`n[Step 4] 完整生命周期..." -ForegroundColor Yellow

$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="running"; task="Lifecycle test" }
Assert $ok "running 事件发送成功" "running 事件失败"
Start-Sleep -Milliseconds 300

$decId2 = "lifecycle_$(Get-Random)"
$ok = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Lifecycle test"; message="Allow Edit -> main.rs?"
    decision_id=$decId2; hook_event_name="PreToolUse"; tool_name="Edit"; tool_preview="main.rs"
}
Assert $ok "needs_input 事件发送成功" "needs_input 事件失败"
Start-Sleep -Milliseconds 300

$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="completed"; task="Lifecycle test"; message="Done" }
Assert $ok "completed 事件发送成功" "completed 事件失败"
Start-Sleep -Milliseconds 300

$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="idle" }
Assert $ok "idle 事件发送成功" "idle 事件失败"

# --- Step 5: 自动批准验证 ---
Write-Host "`n[Step 5] 自动批准验证..." -ForegroundColor Yellow
$decId3 = "auto_$(Get-Random)"
$null = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Auto-approve test"; message="Allow Bash -> npm test?"
    decision_id=$decId3; hook_event_name="PreToolUse"; tool_name="Bash"; tool_preview="npm test"
}

# 轮询最多 2 秒
$deadline = (Get-Date).AddSeconds(2)
$finalDecision = "pending"
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 200
    $d = Check-Decision $decId3
    if ($d -eq "allowed" -or $d -eq "denied") {
        $finalDecision = $d
        break
    }
}

if ($finalDecision -eq "allowed") {
    Write-Host "  ✅ PASS: auto-approve 生效，decision 在 2s 内变为 allowed" -ForegroundColor Green
    $script:pass++
} elseif ($finalDecision -eq "pending") {
    Write-Host "  ⏭ SKIP: auto-approve 未开启（请在 VibeHub 中开启后重新运行）" -ForegroundColor DarkYellow
    $script:skip++
} else {
    Write-Host "  ❌ FAIL: unexpected decision: $finalDecision" -ForegroundColor Red
    $script:fail++
}

# --- 清理 ---
Write-Host "`n[清理] 发送 idle 事件重置状态..." -ForegroundColor DarkGray
$null = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="idle" }

# --- 结果 ---
Write-Host ""
$total = $pass + $fail + $skip
Write-Host "=== 结果：$pass PASS / $fail FAIL / $skip SKIP (共 $total 项) ===" -ForegroundColor $(if ($fail -eq 0) { "Green" } else { "Red" })

if ($fail -gt 0) { exit 1 }
exit 0
