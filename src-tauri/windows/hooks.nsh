; Installer hooks for the Windows build (tauri.conf.json: bundle.windows.nsis.installerHooks).
;
; The app is a GUI program, so terminals use the console `keyhandoff` command
; (src/bin/keyhandoff-cli.rs) instead. It is installed as bin\keyhandoff.exe,
; and that folder goes on the user's PATH, so `keyhandoff run -- python report.py`
; works in any new terminal. Uninstalling takes both away again.

!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$INSTDIR\bin"
  Delete "$INSTDIR\bin\keyhandoff.exe"
  CopyFiles /SILENT "$INSTDIR\keyhandoff-cli.exe" "$INSTDIR\bin\keyhandoff.exe"
  nsExec::Exec `powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$d = '$INSTDIR\bin'; $$p = [Environment]::GetEnvironmentVariable('Path', 'User'); if (-not $$p) { $$p = '' }; if (($$p -split ';') -notcontains $$d) { [Environment]::SetEnvironmentVariable('Path', (($$p.TrimEnd(';') + ';' + $$d).TrimStart(';')), 'User') }"`
  Pop $0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::Exec `powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$d = '$INSTDIR\bin'; $$p = [Environment]::GetEnvironmentVariable('Path', 'User'); if ($$p) { [Environment]::SetEnvironmentVariable('Path', ((($$p -split ';') | Where-Object { $$_ -and $$_ -ne $$d }) -join ';'), 'User') }"`
  Pop $0
  RMDir /r "$INSTDIR\bin"
!macroend
