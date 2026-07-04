# 在 settings.json 注册 VibeHub hook 的辅助片段
# 把 hooks 路径替换为你的实际路径后，合并进 ~/.claude/settings.json 的 "hooks" 字段。
# 下面命令会自动用绝对路径生成可直接粘贴的片段。

$hookPath = Join-Path $PSScriptRoot "..\hooks\vibehub-hook.ps1"
$hookPath = (Resolve-Path $hookPath).Path
$cmd = "powershell -NoProfile -ExecutionPolicy Bypass -File `"$hookPath`""

$config = @{
    hooks = @{
        SessionStart     = @(@{ hooks = @(@{ type = "command"; command = $cmd }) })
        UserPromptSubmit = @(@{ hooks = @(@{ type = "command"; command = $cmd }) })
        PreToolUse       = @(@{ hooks = @(@{ type = "command"; command = $cmd }) })
        PostToolUse      = @(@{ hooks = @(@{ type = "command"; command = $cmd }) })
        Notification     = @(@{ hooks = @(@{ type = "command"; command = $cmd }) })
        Stop             = @(@{ hooks = @(@{ type = "command"; command = $cmd }) })
        SessionEnd       = @(@{ hooks = @(@{ type = "command"; command = $cmd }) })
    }
}

Write-Host "将以下内容合并进 ~/.claude/settings.json：" -ForegroundColor Cyan
$config | ConvertTo-Json -Depth 10
