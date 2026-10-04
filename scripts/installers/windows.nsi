; ref: NSIS 3.11 Docs/Chapter4.html; native payload is owned by the Rust installation boundary.
Unicode true
RequestExecutionLevel admin
Name "RSS MDM Agent"
OutFile "@OUTPUT@"
InstallDir "$PROGRAMFILES64\RSS MDM Agent"
ShowInstDetails show
!include "LogicLib.nsh"
!include "FileFunc.nsh"
Var Profile
Var Result
; Establish the same protected public directory ACL as the native installation owner.
Function ProtectedDirectory
  Exch $R0
  System::Call 'advapi32::ConvertStringSecurityDescriptorToSecurityDescriptorW(w "O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FRFX;;;BU)", i 1, *p .r4, p 0) i.r5'
  ${If} $5 == 0
    SetErrorLevel 1
    Abort "Protected installation directory descriptor could not be created."
  ${EndIf}
  System::Call '*(i 12, p r4, i 0) p.r6'
  ; SECURITY_ATTRIBUTES is 12 bytes on the 32-bit NSIS carrier.
  System::Call 'kernel32::CreateDirectoryW(w "$R0", p r6) i.r5 ?e'
  Pop $7
  System::Free $6
  System::Call 'kernel32::LocalFree(p r4) p'
  ${If} $5 == 0
  ${AndIf} $7 != 183
    SetErrorLevel 1
    Abort "Protected installation directory could not be created."
  ${EndIf}
  System::Call 'kernel32::GetFileAttributesW(w "$R0") i.r5'
  IntOp $5 $5 & 0x400
  ${If} $5 != 0
    SetErrorLevel 1
    Abort "Installation directories cannot be reparse points."
  ${EndIf}
  Pop $R0
FunctionEnd
Function .onInit
  SetRegView 64
  SetShellVarContext all
  System::Call 'kernel32::GetCurrentProcess() p.r0'
  ; The plug-in supports *i pointers; zero the upper half before WORD outputs are written.
  System::Call 'kernel32::IsWow64Process2(p r0, *i 0.r1, *i 0.r2) i.r3'
  ${If} $3 == 0
  ${OrIf} $2 != @MACHINE@
    MessageBox MB_ICONSTOP "This package requires its native Windows architecture."
    SetErrorLevel 1
    Abort
  ${EndIf}
  ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    MessageBox MB_ICONSTOP "Install the machine-wide Microsoft Edge WebView2 Runtime before installing RSS MDM Agent."
    SetErrorLevel 1
    Abort
  ${EndIf}
  ${GetParameters} $0
  ${GetOptions} $0 "/PROFILE=" $Profile
FunctionEnd
Section "Install"
  Push "$INSTDIR"
  Call ProtectedDirectory
  Push "$INSTDIR\versions"
  Call ProtectedDirectory
  ${IfNot} ${FileExists} "$INSTDIR\versions\@ID@\candidate.json"
    ${If} ${FileExists} "$INSTDIR\versions\@ID@\*"
      SetErrorLevel 1
      Abort "An incomplete immutable payload is retained. Use a fresh package version."
    ${EndIf}
    Push "$INSTDIR\versions\@ID@"
    Call ProtectedDirectory
    SetOutPath "$INSTDIR\versions\@ID@"
    File /r "@PAYLOAD@\*"
    WriteUninstaller "$INSTDIR\versions\@ID@\uninstall.exe"
  ${EndIf}
  StrCpy $0 '"$INSTDIR\versions\@ID@\rss-execution-service.exe" --installation apply --candidate "$INSTDIR\versions\@ID@\candidate.json"'
  ${If} $Profile != ""
    StrCpy $0 '$0 --profile "$Profile"'
  ${EndIf}
  ExecWait $0 $Result
  ${If} $Result != 0
    SetErrorLevel $Result
    Abort "Installation validation or registration failed; retained device state was preserved."
  ${EndIf}
SectionEnd
Section "Uninstall"
  SetRegView 64
  SetShellVarContext all
  StrCpy $0 "$INSTDIR\versions\@ID@\rss-execution-service.exe"
  StrCpy $1 $0
  ${IfNot} ${FileExists} "$1"
    StrCpy $1 "$INSTDIR\retired\@ID@\rss-execution-service.exe"
  ${EndIf}
  ExecWait '"$1" --installation uninstall --binary "$0"' $Result
  ${If} $Result != 0
    SetErrorLevel $Result
    Abort "Service removal was not confirmed. Close RSS MDM Agent and retry."
  ${EndIf}
  ; Only the version detached by the Rust owner can be deleted after its worker exits.
  ClearErrors
  RMDir /r "$INSTDIR\retired\@ID@"
  ${If} ${Errors}
    SetErrorLevel 1
    Abort "The service was retired. Retry removal of this version's retired payload."
  ${EndIf}
SectionEnd
