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
Var InLineCreateStartMenu
Var InLineCreateDesktop
Var InLineRemoveShortcuts
Var InLineRegistryHive
!define InLineGuardSource "${__FILEDIR__}"
!macro InLineExtractGuard
  InitPluginsDir
  File /oname=$PLUGINSDIR\InstallGuard.cs "${InLineGuardSource}\InstallGuard.cs"
  File /oname=$PLUGINSDIR\InstallMetadata.cs "${InLineGuardSource}\InstallMetadata.cs"
  File /oname=$PLUGINSDIR\install-guard.ps1 "${InLineGuardSource}\install-guard.ps1"
!macroend
!macro InLineGuard OPERATION
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\install-guard.ps1" -InstallDirectory "$INSTDIR" -Operation "${OPERATION}" -PayloadDirectory "$PLUGINSDIR\inline-payload" -ProductVersion "${VERSION}" -Publisher "${MANUFACTURER}" -BundleId "${BUNDLEID}" -DesktopDirectory "$DESKTOP" -ProgramsDirectory "$SMPROGRAMS" -StartMenuFolder ".$AppStartMenuFolder" -RegistryHive "$InLineRegistryHive" -CreateStartMenu $InLineCreateStartMenu -CreateDesktop $InLineCreateDesktop -RemoveShortcuts $InLineRemoveShortcuts -EstimatedSize ${ESTIMATEDSIZE}'
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
  !if "${INSTALLMODE}" != "currentUser"
    !error "Review In-Line metadata scope before changing the currentUser install mode."
  !endif
  !if "${ARCH}" != "x64"
    !error "Review In-Line metadata registry view before changing the x64 target."
  !endif
  StrCpy $InLineRegistryHive "CurrentUser"
  StrCpy $InLineCreateStartMenu 1
  StrCpy $InLineCreateDesktop 0
  StrCpy $InLineRemoveShortcuts 1
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    StrCpy $InLineCreateDesktop 1
  ${EndIf}
  ${If} $WixMode = 0
    ${If} $UpdateMode = 1
    ${OrIf} $NoShortcutMode = 1
      StrCpy $InLineCreateStartMenu 0
      StrCpy $InLineCreateDesktop 0
    ${EndIf}
  ${EndIf}
  !insertmacro InLineExtractGuard
!macroend
!macro NSIS_HOOK_POSTINSTALL
  !insertmacro InLineRequireGuard "finalize"
  StrCpy $InLineTransactionReady 0
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  StrCpy $InLineRegistryHive "CurrentUser"
  StrCpy $InLineCreateStartMenu 0
  StrCpy $InLineCreateDesktop 0
  StrCpy $InLineRemoveShortcuts 1
  !insertmacro MUI_STARTMENU_GETFOLDER Application $AppStartMenuFolder
  ${If} $UpdateMode = 1
    StrCpy $InLineRemoveShortcuts 0
  ${EndIf}
  !insertmacro InLineExtractGuard
  !insertmacro InLineRequireGuard "uninstall"
  StrCpy $InLineTransactionReady 1
!macroend
!macro NSIS_HOOK_POSTUNINSTALL
  !insertmacro InLineRequireGuard "finalize"
  StrCpy $InLineTransactionReady 0
  RMDir "$INSTDIR" ; empty directories only; unknown/user files are preserved
!macroend
