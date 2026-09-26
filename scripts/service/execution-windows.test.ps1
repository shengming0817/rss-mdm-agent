# Behavior tests with mocked control-plane commands; no service/task registration is changed.
$ErrorActionPreference = 'Stop'
$Script = Join-Path $PSScriptRoot 'execution-windows.ps1'
# Installer requires the fixed product leaf; copy into a private test directory for User scope.
$Root = Join-Path ([IO.Path]::GetTempPath()) ('rss-execution-install-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $Root | Out-Null
$Candidate = Join-Path $Root 'rss-execution-service.exe'
[IO.File]::WriteAllBytes($Candidate, [byte[]](1,2,3))
$global:Calls = [Collections.Generic.List[string]]::new()
$global:StopFailed = $false
function global:New-ScheduledTaskPrincipal { param($UserId,$LogonType,$RunLevel) return @{} }
function global:New-ScheduledTaskTrigger { param([switch]$AtLogOn,$User) return @{} }
function global:New-ScheduledTaskAction { param($Execute,$Argument) return @{} }
function global:New-ScheduledTaskSettingsSet { param($ExecutionTimeLimit,$MultipleInstances) return @{} }
function global:Register-ScheduledTask { param($TaskName,$Principal,$Trigger,$Action,$Settings) $global:Calls.Add('register') }
function global:Start-ScheduledTask { param($TaskName) $global:Calls.Add('start'); throw 'start receipt lost; instance may be running' }
function global:Stop-ScheduledTask { param($TaskName,$ErrorAction) $global:Calls.Add('stop'); if ($global:StopFailed) { throw 'stop failed' } }
function global:Get-ScheduledTask { param($TaskName,$ErrorAction) if ($global:Calls.Contains('start')) { return [pscustomobject]@{State='Ready'} }; return $null }
function global:Unregister-ScheduledTask { param($TaskName,[switch]$Confirm) $global:Calls.Add('remove') }
try {
    foreach ($Fail in @($false,$true)) {
        $global:Calls.Clear(); $global:StopFailed=$Fail
        try { & $Script -Action Install -Scope User -Binary $Candidate; throw 'expected failure' } catch {
            if ($_.Exception.Message -notlike '*start receipt lost*') { throw }
        }
        $Expected = if ($Fail) { 'register,start,stop' } else { 'register,start,stop,remove' }
        if (($global:Calls -join ',') -ne $Expected) { throw "wrong rollback: $($global:Calls -join ',')" }
    }
    function global:Get-Item { param($LiteralPath,[switch]$Force) return [pscustomobject]@{FullName=$LiteralPath;Name='rss-execution-service.exe';PSIsContainer=$false;Attributes=0;Directory=$null} }
    function global:Get-Acl { param($LiteralPath)
        $Acl=[pscustomobject]@{}
        $Acl | Add-Member ScriptMethod GetOwner { param($Type) [pscustomobject]@{Value='S-1-5-18'} }
        $Acl | Add-Member ScriptMethod GetAccessRules { param($Explicit,$Inherited,$Type) @() }
        return $Acl
    }
    function global:Get-CimInstance { param($ClassName,$Filter) return $null }
    function global:New-Service { param($Name,$BinaryPathName,$StartupType,$DisplayName) $global:Calls.Add('register') }
    function global:Start-Service { param($Name) $global:Calls.Add('start'); throw 'start receipt lost; instance may be running' }
    function global:Stop-Service { param($Name,$ErrorAction) $global:Calls.Add('stop'); if ($global:StopFailed) { throw 'stop failed' } }
    function global:Get-Service { param($Name)
        $Service=[pscustomobject]@{}
        $Service | Add-Member ScriptMethod WaitForStatus { param($State,$Timeout) $global:Calls.Add('wait') }
        return $Service
    }
    function global:sc.exe { param($Action,$Name) $global:Calls.Add('remove'); $global:LASTEXITCODE=0 }
    foreach ($Fail in @($false,$true)) {
        $global:Calls.Clear(); $global:StopFailed=$Fail
        try { & $Script -Action Install -Scope System -Binary 'C:\trusted\rss-execution-service.exe'; throw 'expected failure' } catch {
            if ($_.Exception.Message -notlike '*start receipt lost*') { throw }
        }
        $Expected = if ($Fail) { 'register,start,stop' } else { 'register,start,stop,wait,remove' }
        if (($global:Calls -join ',') -ne $Expected) { throw "wrong SCM rollback: $($global:Calls -join ',')" }
    }
    Write-Output 'installer uncertain-start rollback passed'
} finally {
    Remove-Item -LiteralPath $Root -Recurse -Force
}
