!macro customInstall
  ; Refresh the application entry even on updates that retain existing shortcuts.
  ; Pinning remains a user action; this creates only the normal app shortcut.
  !insertmacro createMenuDirectory
  CreateShortCut "$newStartMenuLink" "$appExe" "" "$appExe" 0 "" "" "${APP_DESCRIPTION}"
  WinShell::SetLnkAUMI "$newStartMenuLink" "${APP_ID}"
  WriteRegStr SHELL_CONTEXT "Software\Microsoft\Windows\CurrentVersion\App Paths\Palisade.exe" "" "$appExe"
  WriteRegStr SHELL_CONTEXT "Software\Microsoft\Windows\CurrentVersion\App Paths\Palisade.exe" "Path" "$INSTDIR"
  WriteRegStr SHELL_CONTEXT "${UNINSTALL_REGISTRY_KEY}" "InstallLocation" "$INSTDIR"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
!macroend

!macro customUnInstall
  ; Remove only registrations pointing to this installation, not a successor.
  ReadRegStr $R0 SHELL_CONTEXT "Software\Microsoft\Windows\CurrentVersion\App Paths\Palisade.exe" ""
  ${If} $R0 == "$INSTDIR\${APP_EXECUTABLE_FILENAME}"
    DeleteRegKey SHELL_CONTEXT "Software\Microsoft\Windows\CurrentVersion\App Paths\Palisade.exe"
  ${EndIf}
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\Palisade.exe" ""
  ${If} $R0 == "$INSTDIR\${APP_EXECUTABLE_FILENAME}"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\Palisade.exe"
  ${EndIf}
!macroend
