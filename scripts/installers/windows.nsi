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
Function .onInit
  SetRegView 64
  SetShellVarContext all
  System::Call 'kernel32::GetCurrentProcess() p.r0'
  System::Call 'kernel32::IsWow64Process2(p r0, *i .r1, *i .r2) i.r3'
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
  SetOutPath "$INSTDIR\versions\@ID@"
  File /r "@PAYLOAD@\*"
  StrCpy $0 '"$INSTDIR\versions\@ID@\rss-execution-service.exe" --installation apply --candidate "$INSTDIR\versions\@ID@\candidate.json"'
  ${If} $Profile != ""
    StrCpy $0 '$0 --profile "$Profile"'
  ${EndIf}
  ExecWait $0 $Result
  ${If} $Result != 0
    SetErrorLevel $Result
    Abort "Installation validation or registration failed; retained device state was preserved."
  ${EndIf}
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS MDM Agent" "DisplayName" "RSS MDM Agent"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS MDM Agent" "DisplayVersion" "@VERSION@"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS MDM Agent" "UninstallString" '$"$INSTDIR\uninstall.exe$"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS MDM Agent" "QuietUninstallString" '$"$INSTDIR\uninstall.exe$" /S'
  WriteRegStr HKLM "Software\RSS MDM Agent" "ServiceImage" "$INSTDIR\versions\@ID@\rss-execution-service.exe"
  CreateShortcut "$SMPROGRAMS\RSS MDM Agent.lnk" "$INSTDIR\versions\@ID@\rss-mdm-desktop.exe"
SectionEnd
Section "Uninstall"
  SetRegView 64
  SetShellVarContext all
  ReadRegStr $0 HKLM "Software\RSS MDM Agent" "ServiceImage"
  ExecWait '"$0" --installation remove --binary "$0"' $Result
  ${If} $Result != 0
    SetErrorLevel $Result
    Abort "Service removal was not confirmed. Close RSS MDM Agent and retry."
  ${EndIf}
  Delete "$SMPROGRAMS\RSS MDM Agent.lnk"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS MDM Agent"
  DeleteRegKey HKLM "Software\RSS MDM Agent"
  RMDir /r "$INSTDIR\versions"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
