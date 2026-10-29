# Windows installer safety

`installer.nsi` is the Tauri CLI **2.11.4** upstream template, from
https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi
(Tauri MIT/Apache-2.0). Retain its upstream structure and Chinese font hook.
The build check deliberately rejects a CLI version change until this template is reviewed.

P2-C changes are limited to the installation lifecycle:

- Remove calls to name-based GUI process termination and the MCP `taskkill` hook.
- Upgrade/repair in place; never execute an older uninstaller. MSI migration,
  unsupported legacy main-binary names and disallowed downgrades stop explicitly.
- Stage GUI, MCP, CLI, WebView2Loader.dll and the generated uninstaller in the
  private NSIS plugins directory before touching any target program.
- Run the finite `install-guard.ps1` / `InstallGuard.cs` transaction using built-in
  Windows PowerShell. No additional installed binary, service or resident proxy.
- Acquire every target with read/write/delete access and **no sharing** before
  changing bytes. File identity, non-reparse paths and single hard-link count
  are checked. A directory handle prevents replacing the selected root.
- Any file holder or access uncertainty blocks the operation. Restart Manager
  is used only to report exact registered-file holders, PID, creation time,
  executable path and user SID. There is no shutdown or process-termination API.
- Save and verify rollback bytes, then flush and verify the whole new group.
  Keep the transaction directory until NSIS completes. Failure restores the old
  group under the same locks; uninstall marks the whole group for deletion only
  after preflight/backup. Rollback preserves existing ACLs and file timestamps.
- Refuse rollback over a concurrent edit; retain backups and the launch barrier.
  Unknown/interrupted journals stop for inspection rather than blind replay.
- The three program entry points reject business startup while the transaction
  directory exists. Offline version/build-info queries remain compatible.
- Uninstall checks registration belongs to this installation, never removes
  user data, and leaves unknown installation-directory files alone.

The ordinary supported error paths have automated rollback tests. Power loss,
device failure, enterprise PowerShell restrictions, second-user sessions and real
NSIS registration/UI behavior still require their stated acceptance gates.
Never clear a pending transaction directory blindly: inspect `journal.xml`,
verify the fixed-name backup hashes and current files, then use the guard's
explicit rollback operation only when ownership and expected bytes are confirmed.

`verify-installer-guard.ps1` uses only a newly created isolated directory,
synthetic data and console fixture processes that exit via private signal files.
It never invokes the native installer or touches an installed application.
