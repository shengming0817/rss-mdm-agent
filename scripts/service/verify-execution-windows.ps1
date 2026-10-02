# Target Windows 11 x64 only. Reuse the installation owner; never run from a macOS developer host.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('InstallSystem','VerifyUser','RefreshSystem','RemoveSystem')][string]$Phase,
    [Parameter(Mandatory)][string]$Candidate,
    [Parameter(Mandatory)][string]$Config,
    [Parameter(Mandatory)][string]$Output,
    [string]$CurrentBinary,
    [string]$CurrentConfig
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
if (![OperatingSystem]::IsWindows() -or [Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') { throw 'Windows 11 x64 target required' }
$Identity=[Security.Principal.WindowsIdentity]::GetCurrent()
$Elevated=([Security.Principal.WindowsPrincipal]$Identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (($Phase -eq 'VerifyUser') -eq $Elevated) { throw 'VerifyUser requires the actual non-elevated login; system phases require an administrator' }
$Root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$Owner=Join-Path $PSScriptRoot 'execution-windows.ps1'
$Candidate=[IO.Path]::GetFullPath($Candidate)
$Config=[IO.Path]::GetFullPath($Config)
$Frozen=(& node (Join-Path $Root 'scripts/native-candidate.mjs') verify $Candidate | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw 'Fixed candidate verification failed' }
$Binary=$Frozen.binaries.service.path
if ([IO.File]::Exists($Output) -or [IO.Directory]::Exists($Output)) { throw 'Receipt directory must be new' }
$null=New-Item -ItemType Directory -Path $Output
$Acl=[Security.AccessControl.DirectorySecurity]::new()
$Acl.SetOwner($Identity.User); $Acl.SetAccessRuleProtection($true,$false)
foreach ($Sid in @($Identity.User.Value,'S-1-5-18','S-1-5-32-544')) {
    $Acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($Sid),'FullControl','ContainerInherit,ObjectInherit','None','Allow'))
}
Set-Acl -LiteralPath $Output -AclObject $Acl
$Receipt=[ordered]@{status='running';phase=$Phase;utc=[DateTime]::UtcNow.ToString('o');osVersion=[Environment]::OSVersion.Version.ToString();architecture='x64';subject=$Identity.User.Value;session=(Get-Process -Id $PID).SessionId;candidate=$Frozen;signature=(Get-AuthenticodeSignature -LiteralPath $Binary).Status.ToString();scenarios=@{};unexecuted=@('remote/same-name pipe','wrong SID/session','fake server','weak ACL/reparse','LocalSystem/user DPAPI separation','malformed/replay/expiry requests','old-connection revocation','real desktop/Codex','Host/worker Job Object recovery')}
$HelperCreated=$false
try {
    switch ($Phase) {
        InstallSystem {
            & $Owner -Action Install -Scope System -Binary $Binary -Config $Config
            $Receipt.scenarios.install=& $Owner -Action Status -Scope System -Binary $Binary -Config $Config
        }
        VerifyUser {
            $TaskName='RssExecution-'+$Identity.User.Value
            if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) { throw 'Existing helper registration; refusing replacement' }
            & $Owner -Action Install -Scope User -Binary $Binary -Config $Config
            $HelperCreated=$true
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
            & $Owner -Action Remove -Scope System -Binary $Binary -Config $Config
            $Receipt.scenarios.remove='registration removed; persistent state retained by installation owner'
        }
    }
    $Receipt.status='passed'
} catch {
    $Receipt.status='failed'; $Receipt.error=$_.Exception.Message
    throw
} finally {
    if ($HelperCreated) {
        try { & $Owner -Action Remove -Scope User -Binary $Binary -Config $Config; $Receipt.scenarios.helperCleanup='complete' }
        catch { $Receipt.status='failed'; $Receipt.cleanupError=$_.Exception.Message }
    }
    try { $null=& node (Join-Path $Root 'scripts/native-candidate.mjs') verify $Candidate; if ($LASTEXITCODE -ne 0) { throw 'Candidate changed' } }
    catch { $Receipt.status='failed'; $Receipt.candidateError=$_.Exception.Message }
    $Receipt | ConvertTo-Json -Depth 50 | Set-Content -LiteralPath (Join-Path $Output 'receipt.json') -Encoding utf8
}
if ($Receipt.status -ne 'passed') { throw 'Target acceptance or cleanup failed; inspect receipt' }
