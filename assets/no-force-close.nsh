!include "LogicLib.nsh"
!include "FileFunc.nsh"

!ifndef PALISADE_UPDATE_MUTEX_NAME
  !define PALISADE_UPDATE_MUTEX_NAME "Global\Palisade-Update-org.onyxops.palisade-v1"
!endif
!ifndef PALISADE_UPDATE_PERMIT_PREFIX
  !define PALISADE_UPDATE_PERMIT_PREFIX "Global\Palisade-Update-Installer-v1-"
!endif

!ifndef BUILD_UNINSTALLER
  Var PalisadeUpdateMutex
!endif

!macro palisadeInstallGate
  ${If} $PalisadeUpdateMutex == ""
    ClearErrors
    ${GetParameters} $R0
    ${GetOptions} $R0 "--palisade-update=" $R1
    ${IfNot} ${Errors}
      StrLen $R2 $R1
      ${If} $R2 != 64
        SetErrorLevel 170
        Quit
      ${EndIf}
      System::Call 'shlwapi::StrSpnW(w r11, w "0123456789abcdef") i.r12'
      ${If} $R2 != 64
        SetErrorLevel 170
        Quit
      ${EndIf}
      ; Retain the original global object BEFORE consuming its one-shot permit.
      ; If the helper crashes later, this observer keeps startup exclusion alive.
      System::Call 'kernel32::OpenMutexW(i 0x00100000, i 0, w "${PALISADE_UPDATE_MUTEX_NAME}") p.r12'
      ${If} $R2 = 0
        SetErrorLevel 170
        Quit
      ${EndIf}
      StrCpy $PalisadeUpdateMutex $R2
      System::Call 'kernel32::OpenEventW(i 0x00100000, i 0, w "${PALISADE_UPDATE_PERMIT_PREFIX}$R1") p.r12'
      ${If} $R2 = 0
        SetErrorLevel 170
        Quit
      ${EndIf}
      System::Call 'kernel32::WaitForSingleObject(p r12, i 0) i.r13'
      System::Call 'kernel32::CloseHandle(p r12)'
      ${If} $R3 <> 0
        SetErrorLevel 170
        Quit
      ${EndIf}
    ${Else}
      ; Manual installers serialize too. Existing objects, including abandoned
      ; operations retained by an observer, are never assumed safe to replace.
      System::Call 'kernel32::CreateMutexW(p 0, i 1, w "${PALISADE_UPDATE_MUTEX_NAME}") p.r12 ?e'
      Pop $R3
      ${If} $R2 = 0
        SetErrorLevel 170
        Quit
      ${EndIf}
      StrCpy $PalisadeUpdateMutex $R2
      ${If} $R3 == 183
        SetErrorLevel 170
        Quit
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend

!macro customCheckAppRunning
  ; The outer installer retains exclusion across its old-uninstaller child.
  !ifndef BUILD_UNINSTALLER
    !insertmacro palisadeInstallGate
  !endif
  nsProcess::_FindProcess /NOUNLOAD "${APP_EXECUTABLE_FILENAME}"
  Pop $R0
  nsProcess::_Unload
  ; 603 alone means absent. A found process or enumeration error blocks changes.
  ${If} $R0 != 603
    IfSilent +2 0
    MessageBox MB_OK|MB_ICONEXCLAMATION "Close Palisade normally in every Windows session before installing or removing it. No running client will be force-closed."
    SetErrorLevel 170
    Quit
  ${EndIf}
!macroend
