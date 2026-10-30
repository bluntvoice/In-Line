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

R1 adds `InstallMetadata.cs` to the same finite helper and journal. Its scope is
the fixed In-Line ARP key, the vendor key's remembered installation directory,
and the current-user desktop/start-menu `In Line.lnk` paths. It is not a general
registry transaction framework. The current x64/currentUser/publisher scope is
explicitly gated; changing that packaging scope requires review.

- Acquire and identify owned shortcuts before changing bytes. A required path
  occupied by a foreign shortcut stops installation; optional/foreign uninstall
  paths are preserved. No prior best-effort shortcut write/delete is trusted.
- Keep original link bytes, ACL and timestamps in the recovery journal. Link
  writes use exclusive handles, flush and hash readback; deletion is checked
  again during finalize. New links retain the application's AppUserModelID.
- On a registry failure, restore links under retained handles along with the
  program group. Explicit later recovery verifies current bytes before writing;
  a concurrent replacement causes refusal and retains the journal/startup barrier.
- Snapshot and read back the product registration. Remember the chosen directory
  on successful installation; restore the previous value (or absence) on failure.
  Successful uninstall retains directory memory and never changes user data.
- Finish-page desktop creation is an optional action after installation. Its
  failure has a distinct warning and cannot claim the link was created.
- Do not perform irreversible jump-list/unpin cleanup before a transaction can
  fail. The required link lifecycle is guarded; shell pin/recent-item UI cleanup
  is outside the R1 acceptance claim.

R1 hosted native evidence and remaining gates are recorded separately in
`docs/MCP_PHASE2C_R1_ACCEPTANCE.md`. Power loss, device failure, enterprise policy,
second-user sessions and visual wizard/tray behavior retain their acceptance gates.
This protocol is verified recovery under stated conditions, not cross-resource
atomicity across Windows registry and files. Helper waiting/cancellation is a
separate, unimplemented R2 proposal.
Never clear a pending transaction directory blindly: inspect `journal.xml`,
verify the fixed-name backup hashes and current files, then use the guard's
explicit rollback operation only when ownership and expected bytes are confirmed.

`verify-installer-guard.ps1` uses only a newly created isolated directory,
synthetic data and console fixture processes that exit via private signal files.
It never invokes the native installer or touches an installed application.
