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
Var RootPin
Var VersionsPin
Var VersionPin
Var ImagePin
Var Invocation
Var ImagePath
; The carrier establishes trust before executing any retained program. Hold directory
; handles without delete sharing, and verify the packaged service hash under a read handle.
; ref: Microsoft GetSecurityInfo, GetAclInformation, BCryptCreateHash; NSIS System plug-in.
!macro Guards Prefix Create
Function ${Prefix}ProtectedDirectory
  Exch $R0
  StrCpy $0 0
  StrCpy $2 -1
  StrCpy $3 0
  StrCpy $8 0
  StrCpy $9 0
  System::Call 'advapi32::ConvertStringSecurityDescriptorToSecurityDescriptorW(w "O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FRFX;;;BU)", i 1, *p .r0, p 0) i.R1'
  ${If} $R1 == 0
    Goto directory_rejected
  ${EndIf}
!if ${Create} == 1
  System::Call '*(i 12, p r0, i 0) p.r1'
  ; SECURITY_ATTRIBUTES is 12 bytes on the 32-bit NSIS carrier.
  System::Call 'kernel32::CreateDirectoryW(w "$R0", p r1) i.R1 ?e'
  Pop $R2
  System::Free $1
  ${If} $R1 == 0
  ${AndIf} $R2 != 183
    Goto directory_rejected
  ${EndIf}
!endif
  System::Call 'kernel32::CreateFileW(w "$R0", i 0x20080, i 1, p 0, i 3, i 0x02200000, p 0) p.r2'
  ${If} $2 == -1
    Goto directory_rejected
  ${EndIf}
  System::Alloc 52
  Pop $8
  System::Call 'kernel32::GetFileInformationByHandle(p r2, p r8) i.R1'
  ${If} $R1 == 0
    Goto directory_rejected
  ${EndIf}
  System::Call '*$8(i .R1)'
  IntOp $R1 $R1 & 0x410
  ${If} $R1 != 0x10
    Goto directory_rejected
  ${EndIf}
  System::Free $8
  StrCpy $8 0
  System::Call 'advapi32::GetSecurityInfo(p r2, i 1, i 5, *p .r5, p 0, *p .r6, p 0, *p .r3) i.R1'
  ${If} $R1 != 0
  ${OrIf} $5 == 0
  ${OrIf} $6 == 0
    Goto directory_rejected
  ${EndIf}
  System::Call 'advapi32::GetSecurityDescriptorOwner(p r0, *p .r4, *i .R2) i.R1'
  ${If} $R1 == 0
    Goto directory_rejected
  ${EndIf}
  System::Call 'advapi32::EqualSid(p r4, p r5) i.R1'
  System::Call 'advapi32::IsWellKnownSid(p r5, i 22) i.R2'
  ${If} $R1 == 0
  ${AndIf} $R2 == 0
    Goto directory_rejected
  ${EndIf}
  System::Call 'advapi32::GetSecurityDescriptorControl(p r3, *i 0.R1, *i .R2) i.R3'
  ${If} $R3 == 0
    Goto directory_rejected
  ${EndIf}
  IntOp $R1 $R1 & 0x1000
  ${If} $R1 == 0
    Goto directory_rejected
  ${EndIf}
  System::Call 'advapi32::GetSecurityDescriptorDacl(p r0, *i .R1, *p .r7, *i .R2) i.R3'
  ${If} $R3 == 0
  ${OrIf} $R1 == 0
  ${OrIf} $7 == 0
    Goto directory_rejected
  ${EndIf}
  System::Alloc 12
  Pop $8
  System::Alloc 12
  Pop $9
  System::Call 'advapi32::GetAclInformation(p r6, p r8, i 12, i 2) i.R1'
  System::Call 'advapi32::GetAclInformation(p r7, p r9, i 12, i 2) i.R2'
  ${If} $R1 == 0
  ${OrIf} $R2 == 0
    Goto directory_rejected
  ${EndIf}
  System::Call '*$8(i .R1, i .R2, i)'
  System::Call '*$9(i .R3, i .R4, i)'
  ${If} $R1 != $R3
  ${OrIf} $R2 != $R4
  ${OrIf} $R2 < 8
    Goto directory_rejected
  ${EndIf}
  ; Compare ACE contents, independent of the ACL header's unused allocation capacity.
  IntOp $6 $6 + 8
  IntOp $7 $7 + 8
  IntOp $R2 $R2 - 8
  System::Call 'msvcrt::memcmp(p r6, p r7, i R2) i.R1'
  ${If} $R1 != 0
    Goto directory_rejected
  ${EndIf}
  System::Free $8
  System::Free $9
  System::Call 'kernel32::LocalFree(p r3) p'
  System::Call 'kernel32::LocalFree(p r0) p'
  Pop $R0
  Push $2
  Return
 directory_rejected:
  System::Free $8
  System::Free $9
  System::Call 'kernel32::LocalFree(p r3) p'
  System::Call 'kernel32::LocalFree(p r0) p'
  ${If} $2 != -1
    System::Call 'kernel32::CloseHandle(p r2)'
  ${EndIf}
  Pop $R0
  SetErrorLevel 1
  Abort "Installation directories must have the fixed protected owner and ACL."
FunctionEnd
Function ${Prefix}PinnedImage
  Exch $R0
  StrCpy $0 -1
  StrCpy $1 0
  StrCpy $2 0
  StrCpy $3 0
  StrCpy $4 0
  ; Delete sharing permits the Rust owner to retire its version directory. The
  ; checked parent ACL denies user namespace changes; write sharing stays denied.
  System::Call 'kernel32::CreateFileW(w "$R0", i 0x80000000, i 5, p 0, i 3, i 0x00200000, p 0) p.r0'
  ${If} $0 == -1
    Goto image_rejected
  ${EndIf}
  System::Alloc 52
  Pop $3
  System::Call 'kernel32::GetFileInformationByHandle(p r0, p r3) i.R1'
  ${If} $R1 == 0
    Goto image_rejected
  ${EndIf}
  System::Call '*$3(i .R1)'
  IntOp $R1 $R1 & 0x410
  ${If} $R1 != 0
    Goto image_rejected
  ${EndIf}
  System::Call '*$3(i, i, i, i, i, i, i, i, i .R1, i .R2)'
  ${If} $R1 != 0
  ${OrIf} $R2 != @SERVICE_SIZE@
    Goto image_rejected
  ${EndIf}
  System::Free $3
  StrCpy $3 0
  System::Call 'bcrypt::BCryptOpenAlgorithmProvider(*p .r1, w "SHA256", p 0, i 0) i.R1'
  ${If} $R1 != 0
    Goto image_rejected
  ${EndIf}
  ; Windows 7+ lets the provider own the hash object's allocation.
  System::Call 'bcrypt::BCryptCreateHash(p r1, *p .r2, p 0, i 0, p 0, i 0, i 0) i.R1'
  ${If} $R1 != 0
    Goto image_rejected
  ${EndIf}
  System::Alloc 65536
  Pop $3
  System::Alloc 32
  Pop $4
 image_read:
  System::Call 'kernel32::ReadFile(p r0, p r3, i 65536, *i .R2, p 0) i.R1'
  ${If} $R1 == 0
    Goto image_rejected
  ${EndIf}
  ${If} $R2 == 0
    Goto image_finish
  ${EndIf}
  System::Call 'bcrypt::BCryptHashData(p r2, p r3, i R2, i 0) i.R1'
  ${If} $R1 != 0
    Goto image_rejected
  ${EndIf}
  Goto image_read
 image_finish:
  System::Call 'bcrypt::BCryptFinishHash(p r2, p r4, i 32, i 0) i.R1'
  ${If} $R1 != 0
    Goto image_rejected
  ${EndIf}
  System::Call 'crypt32::CryptBinaryToStringW(p r4, i 32, i 0x4000000c, w .R2, *i 65.R3) i.R1'
  ${If} $R1 == 0
    Goto image_rejected
  ${EndIf}
  StrCmp $R2 "@SERVICE_SHA256@" image_verified image_rejected
 image_verified:
  System::Free $3
  System::Free $4
  System::Call 'bcrypt::BCryptDestroyHash(p r2)'
  System::Call 'bcrypt::BCryptCloseAlgorithmProvider(p r1, i 0)'
  Pop $R0
  Push $0
  Return
 image_rejected:
  System::Free $3
  System::Free $4
  ${If} $2 != 0
    System::Call 'bcrypt::BCryptDestroyHash(p r2)'
  ${EndIf}
  ${If} $1 != 0
    System::Call 'bcrypt::BCryptCloseAlgorithmProvider(p r1, i 0)'
  ${EndIf}
  ${If} $0 != -1
    System::Call 'kernel32::CloseHandle(p r0)'
  ${EndIf}
  Pop $R0
  SetErrorLevel 1
  Abort "The executable does not match this fixed installation package."
FunctionEnd
!macroend
!insertmacro Guards "" 1
!insertmacro Guards "un." 0
Function un.onInit
  SetRegView 64
  SetShellVarContext all
  ; NSIS initializes uninstall $INSTDIR to the version containing uninstall.exe.
  StrCpy $INSTDIR "$PROGRAMFILES64\RSS MDM Agent"
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
  ${If} $INSTDIR != "$PROGRAMFILES64\RSS MDM Agent"
    SetErrorLevel 1
    Abort "This fixed package does not support an alternate installation directory."
  ${EndIf}
  Push "$INSTDIR"
  Call ProtectedDirectory
  Pop $RootPin
  Push "$INSTDIR\versions"
  Call ProtectedDirectory
  Pop $VersionsPin
  ${IfNot} ${FileExists} "$INSTDIR\versions\@ID@\candidate.json"
    ${If} ${FileExists} "$INSTDIR\versions\@ID@\*"
      SetErrorLevel 1
      Abort "An incomplete immutable payload is retained. Use a fresh package version."
    ${EndIf}
    Push "$INSTDIR\versions\@ID@"
    Call ProtectedDirectory
    Pop $VersionPin
    SetOutPath "$INSTDIR\versions\@ID@"
    File /r "@PAYLOAD@\*"
    WriteUninstaller "$INSTDIR\versions\@ID@\uninstall.exe"
  ${Else}
    Push "$INSTDIR\versions\@ID@"
    Call ProtectedDirectory
    Pop $VersionPin
  ${EndIf}
  Push "$INSTDIR\versions\@ID@\rss-execution-service.exe"
  Call PinnedImage
  Pop $ImagePin
  StrCpy $Invocation '"$INSTDIR\versions\@ID@\rss-execution-service.exe" --installation apply --package-path "$EXEPATH" --candidate "$INSTDIR\versions\@ID@\candidate.json"'
  ${If} $Profile != ""
    StrCpy $Invocation '$Invocation --profile "$Profile"'
  ${EndIf}
  ExecWait $Invocation $Result
  System::Call 'kernel32::CloseHandle(p $ImagePin)'
  System::Call 'kernel32::CloseHandle(p $VersionPin)'
  System::Call 'kernel32::CloseHandle(p $VersionsPin)'
  System::Call 'kernel32::CloseHandle(p $RootPin)'
  ${If} $Result != 0
    SetErrorLevel $Result
    Abort "Installation validation or registration failed; retained device state was preserved."
  ${EndIf}
SectionEnd
Section "Uninstall"
  SetRegView 64
  SetShellVarContext all
  StrCpy $INSTDIR "$PROGRAMFILES64\RSS MDM Agent"
  Push "$INSTDIR"
  Call un.ProtectedDirectory
  Pop $RootPin
  Push "$INSTDIR\versions"
  Call un.ProtectedDirectory
  Pop $VersionsPin
  StrCpy $0 "$INSTDIR\versions\@ID@\rss-execution-service.exe"
  StrCpy $ImagePath $0
  ${IfNot} ${FileExists} "$ImagePath"
    StrCpy $ImagePath "$INSTDIR\retired\@ID@\rss-execution-service.exe"
    System::Call 'kernel32::CloseHandle(p $VersionsPin)'
    Push "$INSTDIR\retired"
    Call un.ProtectedDirectory
    Pop $VersionsPin
  ${EndIf}
  StrCpy $Invocation '"$ImagePath" --installation uninstall --binary "$INSTDIR\versions\@ID@\rss-execution-service.exe"'
  ${GetParent} "$ImagePath" $R0
  Push $R0
  Call un.ProtectedDirectory
  Pop $VersionPin
  ; Releasing this handle permits retirement; its checked parent remains protected.
  System::Call 'kernel32::CloseHandle(p $VersionPin)'
  Push $ImagePath
  Call un.PinnedImage
  Pop $ImagePin
  ExecWait $Invocation $Result
  System::Call 'kernel32::CloseHandle(p $ImagePin)'
  System::Call 'kernel32::CloseHandle(p $VersionsPin)'
  System::Call 'kernel32::CloseHandle(p $RootPin)'
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
