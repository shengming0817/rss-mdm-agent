# Install the sole production service and its per-login physical helper.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Install','Refresh','Remove','Status')][string]$Action,
    [Parameter(Mandatory)][ValidateSet('System','User')][string]$Scope,
    [string]$Binary,
    [string]$Config,
    [string]$CandidateBinary,
    [string]$CandidateConfig
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Name = 'RssExecution'
$Sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$TaskName = "RssExecution-$Sid"
$ConfigArgs = ''
if ($Config) {
    if (![IO.Path]::IsPathFullyQualified($Config) -or $Config.Contains('"')) { throw 'An absolute deployment path is required' }
    $ConfigArgs = ' --config "' + $Config + '"'
}
$HelperArgs = ($ConfigArgs + ' --user-helper').Trim()
function Assert-Binary {
    if (!$Binary -or ![IO.Path]::IsPathFullyQualified($Binary) -or $Binary.Contains('"')) { throw 'An absolute binary path is required' }
    $Item = Get-Item -LiteralPath $Binary -Force
    if ($Item.PSIsContainer -or $Item.Name -ne 'rss-execution-service.exe') { throw 'Expected rss-execution-service.exe' }
    for ($Part = $Item; $null -ne $Part; $Part = if ($Part.PSIsContainer) { $Part.Parent } else { $Part.Directory }) {
        if (($Part.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Reparse paths are refused' }
        if ($Scope -eq 'System') {
            $Acl = Get-Acl -LiteralPath $Part.FullName
            $Trusted = @('S-1-5-18', 'S-1-5-32-544', 'S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464')
            if ($Acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin $Trusted) { throw 'System binary path must be owned by System, Administrators or TrustedInstaller' }
            foreach ($Rule in $Acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])) {
                if ($Rule.AccessControlType -ne 'Allow' -or $Rule.IdentityReference.Value -in $Trusted -or ($Rule.PropagationFlags -band [Security.AccessControl.PropagationFlags]::InheritOnly)) { continue }
                $Mask = [Security.AccessControl.FileSystemRights]::Delete -bor [Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles -bor [Security.AccessControl.FileSystemRights]::ChangePermissions -bor [Security.AccessControl.FileSystemRights]::TakeOwnership
                if ($Part.FullName -eq $Item.FullName) { $Mask = $Mask -bor [Security.AccessControl.FileSystemRights]::Write }
                if (($Rule.FileSystemRights -band $Mask) -ne 0) { throw 'System binary path grants replacement rights to an untrusted principal' }
            }
        }
    }
    return $Item.FullName
}
function Assert-Refresh {
    $OriginalBinary=$Binary
    try { $null=Assert-Binary; $Binary=$CandidateBinary; $null=Assert-Binary } finally { $Binary=$OriginalBinary }
    if (!$Config -or !$CandidateConfig -or !$CandidateBinary) { throw 'Refresh requires current and candidate binary/config' }
    $Old = (& $Binary --config $Config --validate-installation | ConvertFrom-Json)
    if ($LASTEXITCODE -ne 0) { throw 'Current candidate failed native validation' }
    $New = (& $CandidateBinary --config $CandidateConfig --validate-installation | ConvertFrom-Json)
    if ($LASTEXITCODE -ne 0) { throw 'New candidate failed native validation' }
    if ($Old.version -ne 2 -or $New.version -ne 2 -or $Old.ipc_version -ne 6 -or $New.ipc_version -ne 6 -or $Old.service.path -cne $Binary -or $New.service.path -cne $CandidateBinary) { throw 'Current-format candidate/protocol required' }
    foreach ($Field in @('origin','tenant','enrollment','registration_operation','state_root','helper_work_roots')) {
        if (($Old.$Field | ConvertTo-Json -Depth 20 -Compress) -cne ($New.$Field | ConvertTo-Json -Depth 20 -Compress)) { throw 'Refresh cannot replace identity or persistent state' }
    }
    foreach ($Field in @('work_root','material_root')) { if ($Old.execution.$Field -cne $New.execution.$Field) { throw 'Refresh cannot replace unresolved task resources' } }
    $null = & $Binary --config $Config --validate-persistent
    if ($LASTEXITCODE -ne 0) { throw 'Current persistent identity/storage binding failed; service retained' }
    $null = & $CandidateBinary --config $CandidateConfig --validate-persistent
    if ($LASTEXITCODE -ne 0) { throw 'Candidate cannot access original identity/storage; service retained' }
    return $New
}
function Publish-Config($Path, $Document) {
    $Temporary=$Path+'.refresh'
    $Data=[Text.Encoding]::UTF8.GetBytes(($Document | ConvertTo-Json -Depth 30))
    $Created=$false
    try {
        $Stream=[IO.File]::Open($Temporary,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
        $Created=$true
        try { $Stream.Write($Data,0,$Data.Length); $Stream.Flush($true) } finally { $Stream.Dispose() }
        [IO.File]::Replace($Temporary,$Path,$null)
    } finally { if ($Created -and [IO.File]::Exists($Temporary)) { [IO.File]::Delete($Temporary) } }
}
if ($Scope -eq 'System') {
    $Service = Get-CimInstance Win32_Service -Filter "Name='$Name'"
    if ($Action -eq 'Status') { $Service | Select-Object Name,State,StartName,PathName; return }
    if ($Action -eq 'Remove') {
        if (!$Service) { return }
        if (!$Binary -or $Service.PathName -cne ('"' + $Binary + '"' + $ConfigArgs) -or $Service.StartName -ne 'LocalSystem') { throw 'Refusing to remove an unrelated service; supply its exact original binary path' }
        Stop-Service -Name $Name -ErrorAction Stop
        & sc.exe delete $Name
        if ($LASTEXITCODE -ne 0) { throw 'SCM deletion failed' }
        return
    }
    if ($Action -eq 'Refresh') {
        if (!$Service -or $Service.PathName -cne ('"' + $Binary + '"' + $ConfigArgs) -or $Service.StartName -ne 'LocalSystem') { throw 'Refusing to refresh an unrelated service' }
        $Next = Assert-Refresh
        foreach ($Subject in $Next.helper_work_roots.PSObject.Properties.Name) {
            if (Get-ScheduledTask -TaskName ("RssExecution-$Subject") -ErrorAction SilentlyContinue) { throw 'Remove matching helper from its actual login before system refresh' }
        }
        $Default=Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'RSS MDM Agent\execution.json'
        if ($Default -cne $Config -and [IO.File]::Exists($Default)) { throw 'Refresh requires the default production config; custom controlled deployments use their installation owner' }
        Stop-Service -Name $Name -ErrorAction Stop
        (Get-Service -Name $Name).WaitForStatus('Stopped', [TimeSpan]::FromSeconds(10))
        Publish-Config $Config $Next
        & sc.exe config $Name binPath= ('"' + $CandidateBinary + '" --config "' + $Config + '"')
        if ($LASTEXITCODE -ne 0) { throw 'SCM refresh failed; registration retained' }
        Start-Service -Name $Name
        Write-Output 'Registered current candidate; authenticated user readiness check required'
        return
    }
    if ($Service) { throw 'Service already exists; no overwrite or upgrade path' }
    $Path = Assert-Binary
    New-Service -Name $Name -BinaryPathName ('"' + $Path + '"' + $ConfigArgs) -StartupType Manual -DisplayName 'RSS Agent execution service' | Out-Null
    try { Start-Service -Name $Name } catch {
        $StartError = $_
        try {
            Stop-Service -Name $Name -ErrorAction Stop
            (Get-Service -Name $Name).WaitForStatus('Stopped', [TimeSpan]::FromSeconds(10))
            & sc.exe delete $Name | Out-Null
            if ($LASTEXITCODE -ne 0) { throw 'SCM rollback deletion failed' }
        } catch { Write-Warning "Rollback failed; registration retained for inspection: $_" }
        throw $StartError
    }
} else {
    $Task = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
    if ($Action -eq 'Status') { $Task | Select-Object TaskName,State,Actions,Principal; return }
    if ($Action -eq 'Remove') {
        if (!$Task) { return }
        if (!$Binary -or $Task.Actions.Count -ne 1 -or $Task.Actions[0].Execute -cne $Binary -or $Task.Actions[0].Arguments -cne $HelperArgs -or $Task.Principal.UserId -ne $Sid) { throw 'Refusing to remove an unrelated task; supply its exact original binary path' }
        Stop-ScheduledTask -TaskName $TaskName
        Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
        return
    }
    if ($Action -eq 'Refresh') { throw 'Helper refresh uses Remove/Install in the actual login after administrator publication' }
    if ($Task) { throw 'User helper already exists; no overwrite or upgrade path' }
    $Path = Assert-Binary
    $Principal = New-ScheduledTaskPrincipal -UserId $Sid -LogonType Interactive -RunLevel Limited
    $Trigger = New-ScheduledTaskTrigger -AtLogOn -User $Sid
    $TaskAction = New-ScheduledTaskAction -Execute $Path -Argument $HelperArgs
    $Settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances Parallel
    Register-ScheduledTask -TaskName $TaskName -Principal $Principal -Trigger $Trigger -Action $TaskAction -Settings $Settings | Out-Null
    try { Start-ScheduledTask -TaskName $TaskName } catch {
        $StartError = $_
        try {
            Stop-ScheduledTask -TaskName $TaskName -ErrorAction Stop
            $Deadline = [DateTime]::UtcNow.AddSeconds(10)
            while ((Get-ScheduledTask -TaskName $TaskName).State -eq 'Running') {
                if ([DateTime]::UtcNow -ge $Deadline) { throw 'User helper stop timed out' }
                Start-Sleep -Milliseconds 100
            }
            Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
        } catch { Write-Warning "Rollback failed; registration retained for inspection: $_" }
        throw $StartError
    }
}
