# VibeHub Approval Flow End-to-End Test
# Metrics: M1 (event receive), M2 (decision register), M5 (auto-approve), M7 (state transitions)
# Usage: Start VibeHub (npm run tauri dev), then run this script.

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
        Write-Host "  PASS: $passMsg" -ForegroundColor Green
        $script:pass++
    } else {
        Write-Host "  FAIL: $failMsg" -ForegroundColor Red
        $script:fail++
    }
}

Write-Host "=== VibeHub Approval Flow Test ===" -ForegroundColor Cyan
Write-Host ""

# --- Step 1: Connectivity ---
Write-Host "[Step 1] Connectivity..." -ForegroundColor Yellow
$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="running"; task="Connectivity test" }
Assert $ok "POST /event returns ok" "Cannot connect to VibeHub - is it running?"

if (-not $ok) {
    Write-Host "`nVibeHub not running, aborting test." -ForegroundColor Red
    exit 1
}

# --- Step 2: Decision registration ---
Write-Host "`n[Step 2] Decision registration..." -ForegroundColor Yellow
$decId = "test_$(Get-Random)"
$ok = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Approval test"; message="Allow Write -> test.txt?"
    decision_id=$decId; hook_event_name="PreToolUse"; tool_name="Write"; tool_preview="test.txt"
}
Assert $ok "POST needs_input + decision_id returns ok" "POST needs_input failed"

Start-Sleep -Milliseconds 200
$result = Check-Decision $decId
Assert (($result -eq "pending") -or ($result -eq "allowed")) "GET /decision/$decId returns '$result' (pending or allowed)" "GET /decision/$decId returned unexpected: $result"

# --- Step 3: Regression - needs_input without decision_id ---
Write-Host "`n[Step 3] Regression: needs_input without decision_id..." -ForegroundColor Yellow
$ok = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Regression test"; message="Notification without approval"
}
Assert $ok "needs_input without decision_id handled correctly" "POST failed"

# --- Step 4: Full lifecycle ---
Write-Host "`n[Step 4] Full lifecycle..." -ForegroundColor Yellow

$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="running"; task="Lifecycle test" }
Assert $ok "running event sent" "running event failed"
Start-Sleep -Milliseconds 300

$decId2 = "lifecycle_$(Get-Random)"
$ok = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Lifecycle test"; message="Allow Edit -> main.rs?"
    decision_id=$decId2; hook_event_name="PreToolUse"; tool_name="Edit"; tool_preview="main.rs"
}
Assert $ok "needs_input event sent" "needs_input event failed"
Start-Sleep -Milliseconds 300

$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="completed"; task="Lifecycle test"; message="Done" }
Assert $ok "completed event sent" "completed event failed"
Start-Sleep -Milliseconds 300

$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="idle" }
Assert $ok "idle event sent" "idle event failed"

# --- Step 5: Auto-approve verification ---
Write-Host "`n[Step 5] Auto-approve verification..." -ForegroundColor Yellow
$decId3 = "auto_$(Get-Random)"
$null = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Auto-approve test"; message="Allow Bash -> npm test?"
    decision_id=$decId3; hook_event_name="PreToolUse"; tool_name="Bash"; tool_preview="npm test"
}

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
    Write-Host "  PASS: auto-approve active, decision changed to allowed within 2s" -ForegroundColor Green
    $script:pass++
} elseif ($finalDecision -eq "pending") {
    Write-Host "  SKIP: auto-approve not enabled (enable in VibeHub and re-run)" -ForegroundColor DarkYellow
    $script:skip++
} else {
    Write-Host "  FAIL: unexpected decision: $finalDecision" -ForegroundColor Red
    $script:fail++
}

# --- Cleanup ---
Write-Host "`n[Cleanup] Sending idle event to reset state..." -ForegroundColor DarkGray
$null = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="idle" }

# --- Results ---
Write-Host ""
$total = $pass + $fail + $skip
$color = if ($fail -eq 0) { "Green" } else { "Red" }
Write-Host "=== Results: $pass PASS / $fail FAIL / $skip SKIP (total $total) ===" -ForegroundColor $color

if ($fail -gt 0) { exit 1 }
exit 0
