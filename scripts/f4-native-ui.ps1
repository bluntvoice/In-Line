param([Parameter(Mandatory)][string]$Exe,[Parameter(Mandatory)][string]$Output)
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows' -or $env:GITHUB_REPOSITORY -ne 'bluntvoice/In-Line' -or $env:GITHUB_REF -notin @('refs/heads/diagnostics/v050-f4-installed-20261127','refs/heads/diagnostics/v050-f4-medium-20261127')){throw 'Native GUI requires the pinned hosted Windows workflow'}
$root=[IO.Path]::GetFullPath((Join-Path $env:RUNNER_TEMP ("f4-desktop-"+[Guid]::NewGuid().ToString('N'))))
[void][IO.Directory]::CreateDirectory($root)
$probe=@(& "$PSScriptRoot/f4-hosted-admission.ps1" -Desktop -Installed)
$probe|Write-Output
if(!($probe -match 'F4_DESKTOP admission=True')){throw 'Interactive Windows desktop is not available'}
$env:PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD='1'
npm install --prefix $root --no-package-lock --no-audit --no-fund playwright@1.60.0
if ($LASTEXITCODE){exit $LASTEXITCODE}
$listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0)
$listener.Start();$port=$listener.LocalEndpoint.Port;$listener.Stop()
$psi=[Diagnostics.ProcessStartInfo]::new($Exe)
$psi.UseShellExecute=$false
$psi.RedirectStandardOutput=$true
$psi.RedirectStandardError=$true
foreach($key in @($psi.Environment.Keys)){if($key -match 'TOKEN|SECRET|PASSWORD|CREDENTIAL|^IN_LINE_MCP_') {[void]$psi.Environment.Remove($key)}}
$psi.Environment['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS']="--remote-debugging-address=127.0.0.1 --remote-debugging-port=$port"
$psi.Environment['WEBVIEW2_USER_DATA_FOLDER']="$root/webview"
$psi.Environment['TEMP']=$root
$psi.Environment['TMP']=$root
$authorization=Join-Path $env:USERPROFILE '.in-line/mcp-security/authorization.dpapi'
if(Test-Path -LiteralPath $authorization){throw 'Unexpected prior authorizations in disposable profile'}
$warm=[Diagnostics.Process]::Start($psi)
$warmOut=$warm.StandardOutput.ReadToEndAsync()
$warmErr=$warm.StandardError.ReadToEndAsync()
try {
  $until=[DateTime]::UtcNow.AddSeconds(30)
  while(!$warm.HasExited -and !(Test-Path -LiteralPath $authorization) -and [DateTime]::UtcNow -lt $until){Start-Sleep -Milliseconds 300}
  if($warm.HasExited -or !(Test-Path -LiteralPath $authorization)){throw 'Installed program did not initialize its isolated synthetic DPAPI store'}
} finally {
  if(!$warm.HasExited){$warm.Kill($true);[void]$warm.WaitForExit(10000)}
  $warmErr.GetAwaiter().GetResult() | Set-Content -LiteralPath "$Output/first-start-stderr.txt" -Encoding utf8
}
Add-Type -AssemblyName System.Security.Cryptography.ProtectedData
$scope=[Security.Cryptography.DataProtectionScope]::CurrentUser
$clear=[Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($authorization),$null,$scope)
$store=[Text.Encoding]::UTF8.GetString($clear)|ConvertFrom-Json
if($store.clients.Count -ne 0 -or $store.sid -ne [Security.Principal.WindowsIdentity]::GetCurrent().User.Value){throw 'Synthetic account storage did not originate in this lab'}
$fixtureToken=[Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32)).ToLowerInvariant()
$fixtureClient=[Guid]::NewGuid().ToString('N')
$permissions=@{regularRead=$true;fullRead=$false;write=$true}
$store.groups=$permissions
$store.clients=@(@{id=$fixtureClient;name='Synthetic F4 approval';kind='codex';tokenHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($fixtureToken))).ToLowerInvariant();revoked=$false;permissions=$permissions;scope=@{departments=@('F4 synthetic');taskTypes=$null}})
$store.revision++
$fixtureBytes=[Text.Encoding]::UTF8.GetBytes(($store | ConvertTo-Json -Depth 15 -Compress))
try { [IO.File]::WriteAllBytes($authorization,[Security.Cryptography.ProtectedData]::Protect($fixtureBytes,$null,$scope)) }
finally { [Array]::Clear($clear);[Array]::Clear($fixtureBytes);$store=$null }
"F4 synthetic identity staged in this hosted SID with restricted department scope; no credentials retained in evidence"
$app=[Diagnostics.Process]::Start($psi)
$started=$app.StartTime.ToUniversalTime().Ticks
$stdout=$app.StandardOutput.ReadToEndAsync()
$stderr=$app.StandardError.ReadToEndAsync()
try{
  $until=[DateTime]::UtcNow.AddSeconds(45)
  $ready=$false
  while(!$app.HasExited -and [DateTime]::UtcNow -lt $until){
    try{$http=Invoke-RestMethod "http://127.0.0.1:$port/json/list" -TimeoutSec 2; if($http){$ready=$true;break}}catch{}
    Start-Sleep -Milliseconds 400
  }
  if(!$ready){throw 'Native WebView2 runtime endpoint did not open'}
  $listeners=@(Get-NetTCPConnection -State Listen -LocalPort $port)
  if (!$listeners.Count -or @($listeners | Where-Object {$_.LocalAddress -notin @('127.0.0.1','::1')}).Count){throw 'Native WebView2 endpoint is not loopback only'}
  $app.Refresh()
  if($app.MainWindowHandle -eq [IntPtr]::Zero -or $app.SessionId -ne (Get-Process -Id $PID).SessionId){throw 'Installed GUI is not in the interactive runner session'}
  "F4_GUI appPid=$($app.Id) sha256=$((Get-FileHash -LiteralPath $Exe).Hash) port=$port session=$($app.SessionId)"
  $env:F4_NATIVE_APP_PID=[string]$app.Id
  $env:F4_NATIVE_CDP="http://127.0.0.1:$port"
  $env:F4_NATIVE_LAB=$root
  $env:F4_NATIVE_EVIDENCE=$Output
  $env:F4_NATIVE_PWSH=(Get-Process -Id $PID).Path
  $env:F4_NATIVE_DATABASE=Join-Path $env:APPDATA 'in-line/inline.db'
  $env:F4_NATIVE_EXE=$Exe
  $env:F4_SYNTHETIC_ID=$fixtureClient
  $env:F4_SYNTHETIC_TOKEN=$fixtureToken
  node "$PSScriptRoot/f4-native-ui.mjs"
  if($LASTEXITCODE){throw "Native GUI driver exited $LASTEXITCODE"}
}finally{
  Remove-Item Env:F4_SYNTHETIC_ID,Env:F4_SYNTHETIC_TOKEN -ErrorAction SilentlyContinue
  $fixtureToken=$null
  if(!$app.HasExited){
    $found=Get-Process -Id $app.Id
    if($found.StartTime.ToUniversalTime().Ticks -ne $started -or $found.Path -ne $Exe){throw 'Native cleanup process identity changed'}
    $app.Kill($true);[void]$app.WaitForExit(10000)
  }
  $stderr.GetAwaiter().GetResult() | Set-Content -LiteralPath "$Output/installed-gui-stderr.txt" -Encoding utf8
  "F4_GUI cleanup processExited=$($app.HasExited)"
}
exit 0
