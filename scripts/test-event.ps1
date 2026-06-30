# VibeHub 手动联调脚本
# 不依赖真跑 Claude，直接向本地服务 POST 模拟事件，验证胶囊四态切换。
# 用法：先启动 VibeHub（npm run tauri dev），再运行本脚本。

$url = "http://127.0.0.1:51789/event"

function Send-Event($type, $task, $message) {
    $payload = @{
        agent_id   = "claude"
        agent_name = "Claude"
        event_type = $type
        task       = $task
        message    = $message
    } | ConvertTo-Json -Compress
    try {
        Invoke-RestMethod -Uri $url -Method Post -Body $payload -ContentType "application/json" -TimeoutSec 2 | Out-Null
        Write-Host "[OK] $type  $task  $message"
    } catch {
        Write-Host "[FAIL] 无法连接 $url —— VibeHub 是否在运行？" -ForegroundColor Red
    }
}

Write-Host "=== VibeHub 联调：将依次模拟 running -> needs_input -> completed -> idle ===" -ForegroundColor Cyan

Send-Event "running" "Fix auth bug in login flow" ""
Start-Sleep -Seconds 3

Send-Event "needs_input" "Fix auth bug in login flow" "Allow edit to auth.ts?"
Start-Sleep -Seconds 3

Send-Event "completed" "Fix auth bug in login flow" "Tests Passed"
Start-Sleep -Seconds 3

Send-Event "idle" "" ""

Write-Host "=== 完成。观察顶部胶囊是否依次切换了四种状态。===" -ForegroundColor Cyan
