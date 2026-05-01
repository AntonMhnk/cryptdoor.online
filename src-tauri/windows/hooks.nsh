; CryptDoor NSIS installer hooks.
;
; The CryptDoor helper runs as a Windows service (CryptDoorHelper) and holds
; cryptdoor-helper.exe open. To allow updates and uninstalls we must stop the
; service before touching files and (re)start it afterwards.
;
; All sc commands are best-effort: on the very first install nothing exists
; to stop, and that's fine — nsExec just logs a non-zero exit code.

!define NSIS_HOOK_PREINSTALL "CRYPTDOOR_PREINSTALL"
!macro CRYPTDOOR_PREINSTALL
  DetailPrint "Stopping CryptDoor helper service (if running)..."
  nsExec::ExecToLog 'sc stop CryptDoorHelper'
  Sleep 1000
!macroend

!define NSIS_HOOK_POSTINSTALL "CRYPTDOOR_POSTINSTALL"
!macro CRYPTDOOR_POSTINSTALL
  DetailPrint "Starting CryptDoor helper service (if registered)..."
  nsExec::ExecToLog 'sc start CryptDoorHelper'
!macroend

!define NSIS_HOOK_PREUNINSTALL "CRYPTDOOR_PREUNINSTALL"
!macro CRYPTDOOR_PREUNINSTALL
  DetailPrint "Removing CryptDoor helper service..."
  nsExec::ExecToLog 'sc stop CryptDoorHelper'
  Sleep 1000
  nsExec::ExecToLog 'sc delete CryptDoorHelper'
!macroend
