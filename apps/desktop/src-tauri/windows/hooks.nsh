; RGBStuff NSIS hooks

!macro NSIS_HOOK_PREINSTALL
  ; Free the files of a running copy (installed build or an old portable exe).
  nsExec::Exec 'taskkill /F /IM RGBStuff.exe'
  nsExec::Exec 'taskkill /F /IM rgbstuff-desktop.exe'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec 'taskkill /F /IM RGBStuff.exe'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; "Start with my PC" lives in the user's Run key; do not leave it dangling.
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "RGBStuff"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "RGBStuff"
!macroend
