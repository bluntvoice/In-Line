; Codex 等客户端可能仍在运行 stdio MCP，Windows 会锁定旧的伴随程序。
; 安装和卸载写文件前仅结束 In Line MCP，不关闭 Codex 或其他客户端。
!macro StopInLineMcp
  DetailPrint "正在关闭 In Line MCP 伴随程序..."
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM "in-line-mcp.exe"'
  Sleep 800
  ; AI 客户端可能在伴随进程异常退出后立即重启它；只做一次有界复查，不关闭客户端本体。
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM "in-line-mcp.exe"'
  Sleep 350
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro StopInLineMcp
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro StopInLineMcp
!macroend
