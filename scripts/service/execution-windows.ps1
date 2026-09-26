# Local candidate host only. Production identity/approval/bootstrap remains #2564.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Install','Remove','Status')][string]$Action,
    [Parameter(Mandatory)][ValidateSet('System','User')][string]$Scope,
    [string]$Binary
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Name = 'RssExecution'
$Sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$TaskName = "RssExecution-$Sid"
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
if ($Scope -eq 'System') {
    $Service = Get-CimInstance Win32_Service -Filter "Name='$Name'"
    if ($Action -eq 'Status') { $Service | Select-Object Name,State,StartName,PathName; return }
    if ($Action -eq 'Remove') {
        if (!$Service) { return }
        if (!$Binary -or $Service.PathName -cne ('"' + $Binary + '"') -or $Service.StartName -ne 'LocalSystem') { throw 'Refusing to remove an unrelated service; supply its exact original binary path' }
        Stop-Service -Name $Name -ErrorAction Stop
        & sc.exe delete $Name
        if ($LASTEXITCODE -ne 0) { throw 'SCM deletion failed' }
        return
    }
    if ($Service) { throw 'Service already exists; no overwrite or upgrade path' }
    $Path = Assert-Binary
    New-Service -Name $Name -BinaryPathName ('"' + $Path + '"') -StartupType Manual -DisplayName 'RSS execution candidate' | Out-Null
    try { Start-Service -Name $Name } catch {
        & sc.exe delete $Name | Out-Null
        if ($LASTEXITCODE -ne 0) { Write-Warning 'Rollback failed; inspect the candidate service' }
        throw
    }
} else {
    $Task = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
    if ($Action -eq 'Status') { $Task | Select-Object TaskName,State,Actions,Principal; return }
    if ($Action -eq 'Remove') {
        if (!$Task) { return }
        if (!$Binary -or $Task.Actions.Count -ne 1 -or $Task.Actions[0].Execute -cne $Binary -or $Task.Actions[0].Arguments -cne '--user' -or $Task.Principal.UserId -ne $Sid) { throw 'Refusing to remove an unrelated task; supply its exact original binary path' }
        Stop-ScheduledTask -TaskName $TaskName
        Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
        return
    }
    if ($Task) { throw 'User helper already exists; no overwrite or upgrade path' }
    $Path = Assert-Binary
    $Principal = New-ScheduledTaskPrincipal -UserId $Sid -LogonType Interactive -RunLevel Limited
    $Trigger = New-ScheduledTaskTrigger -AtLogOn -User $Sid
    $TaskAction = New-ScheduledTaskAction -Execute $Path -Argument '--user'
    $Settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances Parallel
    Register-ScheduledTask -TaskName $TaskName -Principal $Principal -Trigger $Trigger -Action $TaskAction -Settings $Settings | Out-Null
    try { Start-ScheduledTask -TaskName $TaskName } catch { Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false; throw }
}
