; Independent current-user carrier. Never stages or restores the personal Vault.
Unicode true
RequestExecutionLevel user
Name "RSS Personal Passkeys"
OutFile "@OUTPUT@"
InstallDir "$LOCALAPPDATA\Programs\RSS Personal Passkeys"
ShowInstDetails show
!include "LogicLib.nsh"
Var Result
Section "Install"
  SetOutPath "$INSTDIR.incoming"
  File /r "@PAYLOAD@\*"
  WriteUninstaller "$INSTDIR.incoming\uninstall.exe"
  ExecWait 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR.incoming\identity.ps1" -Action preflight' $Result
  ${If} $Result != 0
    Abort "Package validation failed. Installed component unchanged."
  ${EndIf}
  IfFileExists "$INSTDIR\rss-passkey-provider.exe" 0 stopped
  ExecWait '"$INSTDIR\rss-passkey-provider.exe" stop' $Result
  ${If} $Result != 0
    Abort "Provider is busy; cancel its current operation and retry."
  ${EndIf}
stopped:
  SetOutPath "$TEMP"
  ; A previous interrupted attempt is kept for diagnosis, never restored over Vault data.
  IfFileExists "$INSTDIR.previous\*" 0 backup
  IfFileExists "$INSTDIR\*" 0 publish
  Abort "A previous interrupted installation exists. Complete repair before replacing another version."
backup:
  IfFileExists "$INSTDIR\*" 0 publish
  ClearErrors
  Rename "$INSTDIR" "$INSTDIR.previous"
  ${If} ${Errors}
    Abort "Cannot stage the installed component; no replacement performed."
  ${EndIf}
publish:
  ClearErrors
  Rename "$INSTDIR.incoming" "$INSTDIR"
  ${If} ${Errors}
    Abort "Publication interrupted. Rerun this installer to resume; previous files retained."
  ${EndIf}
  SetOutPath "$INSTDIR"
  ExecWait 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\identity.ps1" -Action install' $Result
  ${If} $Result != 0
    Abort "Identity registration incomplete. Run identity.ps1 -Action install from the installed directory to repair. Files and previous version retained."
  ${EndIf}
  CreateShortCut "$SMPROGRAMS\RSS Personal Passkeys.lnk" "$INSTDIR\rss-passkey-provider.exe" "manage"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS.PersonalPasskeys" "PlatformRemoved" 0
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS.PersonalPasskeys" "DisplayName" "RSS Personal Passkeys"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS.PersonalPasskeys" "UninstallString" '"$INSTDIR\uninstall.exe"'
SectionEnd
Section "Uninstall"
  ReadRegDWORD $Result HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS.PersonalPasskeys" "PlatformRemoved"
  ${If} $Result == 1
    Goto platform_removed
  ${EndIf}
  IfFileExists "$INSTDIR\platform-removed" record_removed 0
  ExecWait 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\identity.ps1" -Action remove' $Result
  ${If} $Result != 0
    Abort "Provider cleanup incomplete. Repair and retry; component and Vault retained."
  ${EndIf}
record_removed:
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS.PersonalPasskeys" "PlatformRemoved" 1
platform_removed:
  SetOutPath "$TEMP"
  ClearErrors
  Delete "$INSTDIR\rss-passkey-provider.exe"
  Delete "$INSTDIR\Assets\StoreLogo.png"
  Delete "$INSTDIR\Assets\Logo.png"
  Delete "$INSTDIR\Assets\SmallLogo.png"
  RMDir "$INSTDIR\Assets"
  ${If} ${Errors}
    Abort "Program cleanup incomplete. Uninstaller and repair payload retained; retry uninstall."
  ${EndIf}
  ClearErrors
  Delete "$INSTDIR\identity.msix"
  Delete "$INSTDIR\identity.ps1"
  Delete "$INSTDIR\deployment.json"
  ${If} ${Errors}
    Abort "Metadata cleanup incomplete. Uninstall entry retained."
  ${EndIf}
  Delete "$INSTDIR\platform-removed"
  Delete "$SMPROGRAMS\RSS Personal Passkeys.lnk"
  Delete "$INSTDIR\uninstall.exe"
  ${If} ${Errors}
    Abort "Final cleanup incomplete. Uninstall entry retained."
  ${EndIf}
  ; Leave an unrelated file, if any, in the otherwise empty program directory.
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS.PersonalPasskeys"
SectionEnd
