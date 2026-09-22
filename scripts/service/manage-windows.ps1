param([string]$RevokeSid, [switch]$Uninstall)
$ErrorActionPreference = "Stop"
if (($Uninstall -and $RevokeSid) -or (-not $Uninstall -and -not $RevokeSid)) { throw "Select exactly one maintenance action." }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if (-not ([Security.Principal.WindowsPrincipal]$identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw "Administrator required." }
$name = "RssMdmAgentStatus"
Stop-Service $name
(Get-Service $name).WaitForStatus("Stopped", [TimeSpan]::FromSeconds(10))
if ($Uninstall) {
    & "$env:SystemRoot\System32\sc.exe" delete $name
    if ($LASTEXITCODE -ne 0) { throw "SCM removal failed." }
    Write-Output "Service registration removed; installation and AI data retained."
} else {
    $null = [Security.Principal.SecurityIdentifier]::new($RevokeSid)
    $path = Join-Path ([Environment]::GetFolderPath("CommonApplicationData")) "RSS MDM Agent\service\policy.json"
    $policy = Get-Content $path -Raw | ConvertFrom-Json
    $policy.allowed_users = @($policy.allowed_users | Where-Object { $_ -ne $RevokeSid })
    $temporary = "$path.new"
    [IO.File]::WriteAllText($temporary, ($policy | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
    Set-Acl $temporary (Get-Acl $path)
    [IO.File]::Replace($temporary, $path, $null)
    if ($policy.allowed_users.Count -gt 0) { Start-Service $name }
    Write-Output "Old connections invalidated; revoked SID cannot establish a new query."
}
