# Target Windows 11 amd64/arm64. Reuse the installation owner; never run from a macOS developer host.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('InstallSystem','VerifyUser','RefreshSystem','RemoveSystem','DeviceAccessHealth','PasskeyHealth','PasskeyRepairCycle')][string]$Phase,
    [Parameter(Mandatory)][string]$Candidate,
    [string]$Config,
    [Parameter(Mandatory)][string]$Output,
    [string]$CurrentBinary,
    [string]$CurrentConfig
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
if (![OperatingSystem]::IsWindows() -or [Runtime.InteropServices.RuntimeInformation]::OSArchitecture -notin @('X64','Arm64')) { throw 'Windows 11 amd64/arm64 target required' }
$Identity=[Security.Principal.WindowsIdentity]::GetCurrent()
$Elevated=([Security.Principal.WindowsPrincipal]$Identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (($Phase -in @('VerifyUser','PasskeyHealth','PasskeyRepairCycle')) -eq $Elevated) { throw 'VerifyUser requires the actual non-elevated login; system phases require an administrator' }
$Root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$Owner=Join-Path $PSScriptRoot 'execution-windows.ps1'
$Candidate=[IO.Path]::GetFullPath($Candidate)
if ($Phase -notin @('PasskeyHealth','PasskeyRepairCycle')) { if (!$Config) { throw 'Service configuration required' }; $Config=[IO.Path]::GetFullPath($Config) }
$Frozen=(& node (Join-Path $Root 'scripts/native-candidate.mjs') verify $Candidate | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw 'Fixed candidate verification failed' }
$Binary=if ($Phase -in @('PasskeyHealth','PasskeyRepairCycle')) { $Frozen.binaries.windowsPasskey.path } else { $Frozen.binaries.service.path }
$NodeVersion=(& node --version)
if ($LASTEXITCODE -ne 0) { throw 'Acceptance Node version unavailable' }
if ([IO.File]::Exists($Output) -or [IO.Directory]::Exists($Output)) { throw 'Receipt directory must be new' }
$null=New-Item -ItemType Directory -Path $Output
$Acl=[Security.AccessControl.DirectorySecurity]::new()
$Acl.SetOwner($Identity.User); $Acl.SetAccessRuleProtection($true,$false)
foreach ($Sid in @($Identity.User.Value,'S-1-5-18','S-1-5-32-544')) {
    $Acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($Sid),'FullControl','ContainerInherit,ObjectInherit','None','Allow'))
}
Set-Acl -LiteralPath $Output -AclObject $Acl
$Receipt=[ordered]@{status='running';phase=$Phase;utc=[DateTime]::UtcNow.ToString('o');osVersion=[Environment]::OSVersion.Version.ToString();architecture=[Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant();subject=$Identity.User.Value;session=(Get-Process -Id $PID).SessionId;candidate=$Frozen;acceptanceEnvironment=@{node=$NodeVersion;powershell=$PSVersionTable.PSVersion.ToString()};signature=(Get-AuthenticodeSignature -LiteralPath $Binary).Status.ToString();scenarios=@{};unexecuted=@('remote/same-name pipe','wrong SID/session','fake server','weak ACL/reparse','LocalSystem/user DPAPI separation','malformed/replay/expiry requests','old-connection revocation','real desktop/Codex','Host/worker Job Object recovery')}
try {
    switch ($Phase) {
        PasskeyHealth {
            if (!$CurrentBinary) { throw 'Installed Provider path required; a build directory is not an installation' }
            $Installed=[IO.Path]::GetFullPath($CurrentBinary)
            if ((Get-FileHash -LiteralPath $Installed -Algorithm SHA256).Hash -ine $Frozen.binaries.windowsPasskey.sha256) { throw 'Installed Provider differs from candidate' }
            $Receipt.signature=(Get-AuthenticodeSignature -LiteralPath $Installed).Status.ToString()
            $Receipt.scenarios.provider=(& $Installed inspect | ConvertFrom-Json)
            if ($LASTEXITCODE -ne 0 -or !$Receipt.scenarios.provider.registered) { throw 'Provider registration/identity unavailable' }
            $Receipt.scenarios.systemAccepted=$false
            $Receipt.unexecuted=@('real browser create/assert and RP acceptance','Hello cancellation and lock','cross-user requests','upgrade interruption and repair','uninstall/reinstall and deleted-credential rollback','Google/GitHub acceptance')
        }
        PasskeyRepairCycle {
            # Explicit destructive journey for a disposable test user/candidate; never a CI default.
            if (!$CurrentBinary) { throw 'Installed Provider path required' }
            $Installed=[IO.Path]::GetFullPath($CurrentBinary)
            if ((Get-FileHash -LiteralPath $Installed -Algorithm SHA256).Hash -ine $Frozen.binaries.windowsPasskey.sha256) { throw 'Installed Provider differs from candidate' }
            $InstallRoot=Split-Path -Parent $Installed
            $Setup=Join-Path $InstallRoot 'identity.ps1'
            $Uninstall='HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS.PersonalPasskeys'
            $VaultRoot=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'com.rss.mdmagent\personal-passkeys'
            $Vault=Join-Path $VaultRoot 'vault.json'
            if (!(Test-Path -LiteralPath $Vault)) { throw 'Initialize a disposable personal Vault before the recovery journey' }
            $Before=(Get-FileHash -LiteralPath $Vault -Algorithm SHA256).Hash
            $Receipt.scenarios.vaultBefore=$Before
            # Read actual system state independently of the provider executable being removed.
            Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RssPasskeyRecoveryProbe {
    [DllImport("webauthn.dll", ExactSpelling=true)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
    public static extern int WebAuthNPluginGetAuthenticatorState(ref Guid id, out int state);
}
'@
            function Assert-Removed {
                if (@(Get-AppxPackage -Name 'RSS.PersonalPasskeys').Count -ne 0) { throw 'Package remained after removal' }
                $Class=[Guid]'f1fd3a7c-27c1-4f88-bf89-6ac6525c2929'
                $State=0
                $Result=[RssPasskeyRecoveryProbe]::WebAuthNPluginGetAuthenticatorState([ref]$Class,[ref]$State)
                if ($Result -notin @(-2146893807,-2147023728,-2147024894)) { throw "Provider removal not confirmed: $Result" }
                if ((Get-FileHash -LiteralPath $Vault -Algorithm SHA256).Hash -ne $Before) { throw 'Personal Vault changed during installer recovery' }
            }
            function Invoke-Identity([string]$Step) {
                & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $Setup -Action $Step
                if ($LASTEXITCODE -ne 0) { throw "Identity $Step failed" }
            }
            # Stop after OS removal, intentionally leaving the program/repair payload in place.
            Invoke-Identity 'remove'
            Assert-Removed
            if ((Get-ItemPropertyValue -LiteralPath $Uninstall -Name PlatformRemoved) -ne 1) { throw 'Removal checkpoint missing' }
            $Receipt.scenarios.interruptedRemoval='package/provider absent; repair payload and Vault retained'
            Invoke-Identity 'install'
            if ((Get-ItemPropertyValue -LiteralPath $Uninstall -Name PlatformRemoved) -ne 0) { throw 'Repair retained a completed-removal checkpoint' }
            $State=(& $Installed inspect | ConvertFrom-Json)
            if ($LASTEXITCODE -ne 0 -or !$State.registered) { throw 'Repair did not register Provider' }
            $Receipt.scenarios.inPlaceRepair='Provider registered; checkpoint invalidated'
            $Process=Start-Process -FilePath (Join-Path $InstallRoot 'uninstall.exe') -ArgumentList '/S' -PassThru -Wait
            if ($Process.ExitCode -ne 0) { throw 'Second uninstall failed' }
            Assert-Removed
            if ((Test-Path -LiteralPath $Installed) -or (Test-Path -LiteralPath $Uninstall)) { throw 'Second uninstall left component files or uninstall registration' }
            $Receipt.scenarios.secondUninstall='package/provider and component removed; personal Vault unchanged'
            $Receipt.scenarios.systemAccepted=$false
            $Receipt.unexecuted=@('real browser/RP acceptance','Hello/cross-user request behavior','concurrent installer processes')
        }
        DeviceAccessHealth {
            if ($Identity.User.Value -ne 'S-1-5-18' -or [Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') { throw 'DeviceAccessHealth requires LocalSystem on x64' }
            if (!$Frozen.binaries.PSObject.Properties['windowsLogin']) { throw 'Frozen Windows login DLL required' }
            $ProviderDll=[IO.Path]::GetFullPath($Frozen.binaries.windowsLogin.path)
            $ProviderSha256=$Frozen.binaries.windowsLogin.sha256
            $ProviderSignature=Get-AuthenticodeSignature -LiteralPath $ProviderDll
            $ServiceSignature=Get-AuthenticodeSignature -LiteralPath $Binary
            if ($ProviderSignature.Status -ne 'Valid' -or $ServiceSignature.Status -ne 'Valid' -or $ProviderSignature.SignerCertificate.Thumbprint -ne $ServiceSignature.SignerCertificate.Thumbprint) { throw 'Signed candidate with matching publishers required' }
            $Actual=(Get-FileHash -LiteralPath $ProviderDll -Algorithm SHA256).Hash
            if ($Actual -ine $ProviderSha256) { throw 'Provider candidate hash mismatch' }
            $Class='Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Classes\CLSID\{C61C6B95-9162-42A2-B80B-61EBCA730510}\InprocServer32'
            if ((Get-Item -LiteralPath $Class).GetValue('') -ine $ProviderDll) { throw 'Registered provider differs from fixed candidate' }
            & $Binary --device-access-component health
            if ($LASTEXITCODE -ne 0) { throw 'Device Access component health failed' }
            $Receipt.scenarios.deviceAccess=@{dll=$ProviderDll;sha256=$Actual;signature=(Get-AuthenticodeSignature -LiteralPath $ProviderDll).Status.ToString();health='passed; DLL not loaded'}
            $Receipt.unexecuted+=@('LogonUI login/unlock and offline','wrong password consumes OTP','concurrent OTP replay','reboot throttling and clock rollback','recovery rotation','last administrator protection','provider disable and uninstall')
        }
        InstallSystem {
            & $Owner -Action Install -Scope System -Binary $Binary -Config $Config
            $Receipt.scenarios.install=& $Owner -Action Status -Scope System -Binary $Binary -Config $Config
        }
        VerifyUser {
            $TaskName='RssExecution-'+$Identity.User.Value
            if (!(Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue)) { throw 'Installation owner did not register the login helper' }
            & $Owner -Action Status -Scope User -Binary $Binary -Config $Config
            $Baseline=(& $Binary --config $Config --query | ConvertFrom-Json)
            if ($LASTEXITCODE -ne 0 -or $Baseline.reply.kind -ne 'tasks') { throw 'Authorized query baseline failed; security negatives not executed' }
            $Receipt.scenarios.authorized=$Baseline
            $Wrong=Join-Path $Output 'wrong-client.exe'; Copy-Item -LiteralPath $Binary -Destination $Wrong
            if ((Get-FileHash -LiteralPath $Wrong).Hash -cne (Get-FileHash -LiteralPath $Binary).Hash) { throw 'Wrong-image fixture bytes changed' }
            $Reply=(& $Wrong --config $Config --query | ConvertFrom-Json)
            if ($Reply.reply.kind -ne 'rejected') { throw 'Same-user wrong-image client was not explicitly rejected' }
            $Receipt.scenarios.wrongImage=$Reply
            $After=(& $Binary --config $Config --query | ConvertFrom-Json)
            if ($LASTEXITCODE -ne 0 -or $After.reply.kind -ne 'tasks') { throw 'Authorized channel failed after negative probe' }
            $Receipt.scenarios.after=$After
            Remove-Item -LiteralPath $Wrong
        }
        RefreshSystem {
            if (!$CurrentBinary -or !$CurrentConfig) { throw 'Original exact binary/config required for refresh' }
            & $Owner -Action Refresh -Scope System -Binary $CurrentBinary -Config $CurrentConfig -CandidateBinary $Binary -CandidateConfig $Config
            $Receipt.scenarios.refresh='existing owner verified persistent LocalSystem identity before publication'
        }
        RemoveSystem {
            & $Owner -Action RemoveLabRegistration -Scope System -Binary $Binary -Config $Config
            $Receipt.scenarios.remove='registration removed; persistent state retained by installation owner'
        }
    }
    $Receipt.status='passed'
} catch {
    $Receipt.status='failed'; $Receipt.error=$_.Exception.Message
    throw
} finally {
    try { $null=& node (Join-Path $Root 'scripts/native-candidate.mjs') verify $Candidate; if ($LASTEXITCODE -ne 0) { throw 'Candidate changed' } }
    catch { $Receipt.status='failed'; $Receipt.candidateError=$_.Exception.Message }
    $Receipt | ConvertTo-Json -Depth 50 | Set-Content -LiteralPath (Join-Path $Output 'receipt.json') -Encoding utf8
}
if ($Receipt.status -ne 'passed') { throw 'Target acceptance or cleanup failed; inspect receipt' }
