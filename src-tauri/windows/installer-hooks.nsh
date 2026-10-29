; Match the Windows Chinese UI font and give Tauri's 8u maintenance radios
; enough height for full glyphs. Keep all original styles and other controls.
SetFont "Microsoft YaHei UI" 9
!include nsDialogs.nsh
!define InLineNativeCreateRadioButton "${NSD_CreateRadioButton}"
!undef NSD_CreateRadioButton
!define NSD_CreateRadioButton "!insertmacro InLineCreateRadioButton"
!macro InLineCreateRadioButton X Y WIDTH HEIGHT TEXT
  !if "${HEIGHT}" == "8u"
    ${InLineNativeCreateRadioButton} ${X} ${Y} ${WIDTH} 14u "${TEXT}"
  !else
    ${InLineNativeCreateRadioButton} ${X} ${Y} ${WIDTH} ${HEIGHT} "${TEXT}"
  !endif
!macroend

; Files are managed as one verified group by the custom Tauri template.
; No name-based process actions, Restart Manager shutdown, or forced fallback.
Var InLineTransactionReady
!define InLineGuardSource "${__FILEDIR__}"
!macro InLineExtractGuard
  InitPluginsDir
  File /oname=$PLUGINSDIR\InstallGuard.cs "${InLineGuardSource}\InstallGuard.cs"
  File /oname=$PLUGINSDIR\install-guard.ps1 "${InLineGuardSource}\install-guard.ps1"
!macroend
!macro InLineGuard OPERATION
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\install-guard.ps1" -InstallDirectory "$INSTDIR" -Operation "${OPERATION}" -PayloadDirectory "$PLUGINSDIR\inline-payload"'
  Pop $0
  Pop $1
  DetailPrint "$1"
!macroend
!macro InLineRequireGuard OPERATION
  !insertmacro InLineGuard "${OPERATION}"
  ${If} $0 != 0
    SetErrorLevel 1
    IfSilent +2
    MessageBox MB_ICONSTOP "安装/卸载已停止。目标文件被占用、无法安全确认或事务校验失败；未结束任何进程。请安全退出占用程序后重试。$\r$\n$1"
    Abort
  ${EndIf}
!macroend
!macro NSIS_HOOK_PREINSTALL
  !insertmacro InLineExtractGuard
!macroend
!macro NSIS_HOOK_POSTINSTALL
  !insertmacro InLineRequireGuard "finalize"
  StrCpy $InLineTransactionReady 0
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro InLineExtractGuard
  !insertmacro InLineRequireGuard "uninstall"
  StrCpy $InLineTransactionReady 1
!macroend
!macro NSIS_HOOK_POSTUNINSTALL
  !insertmacro InLineRequireGuard "finalize"
  StrCpy $InLineTransactionReady 0
!macroend
