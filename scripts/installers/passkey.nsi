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
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS.PersonalPasskeys" "DisplayName" "RSS Personal Passkeys"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS.PersonalPasskeys" "UninstallString" '"$INSTDIR\uninstall.exe"'
SectionEnd
Section "Uninstall"
  ExecWait 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\identity.ps1" -Action remove' $Result
  ${If} $Result != 0
    Abort "Provider cleanup incomplete. Repair and retry; component and Vault retained."
  ${EndIf}
  Delete "$INSTDIR\rss-passkey-provider.exe"
  Delete "$INSTDIR\identity.msix"
  Delete "$INSTDIR\identity.ps1"
  Delete "$INSTDIR\deployment.json"
  Delete "$INSTDIR\Assets\StoreLogo.png"
  Delete "$INSTDIR\Assets\Logo.png"
  Delete "$INSTDIR\Assets\SmallLogo.png"
  RMDir "$INSTDIR\Assets"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS.PersonalPasskeys"
SectionEnd
