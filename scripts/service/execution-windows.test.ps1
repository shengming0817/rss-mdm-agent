# Behavior tests with mocked control-plane commands; no service/task registration is changed.
$ErrorActionPreference = 'Stop'
$Script = Join-Path $PSScriptRoot 'execution-windows.ps1'
function Test-SystemPersistent {
    $Image = if ($IsWindows) { 'C:\trusted\rss-execution-service.exe' } else { '/trusted/rss-execution-service.exe' }
    $Deployment = if ($IsWindows) { 'C:\trusted\execution.json' } else { '/trusted/execution.json' }
    $global:ExpectedValidationImage=$Image; $global:ExpectedValidationDeployment=$Deployment
    $global:Calls = [Collections.Generic.List[string]]::new()
    function global:Register-ScheduledTask { param($TaskName,$Principal,$Action,$Settings,$ErrorAction) $global:Calls.Add('register') }
    function global:Unregister-ScheduledTask { param($TaskName,[switch]$Confirm,$ErrorAction) $global:Calls.Add('remove') }
    function global:New-ScheduledTaskSettingsSet { param($ExecutionTimeLimit,$MultipleInstances)
        if ($ExecutionTimeLimit.TotalSeconds -ne 15 -or $MultipleInstances -ne 'IgnoreNew') { throw 'unbounded diagnostic task' }
        return @{}
    }
    # The validation routine must use the actual service identity, not the administrator SID.
    $global:DiagnosticResult = 0
    $global:DiagnosticInfoCalls = 0
    $global:DiagnosticQueryFailure=$false; $global:DiagnosticStopFailed=$false
    function global:New-ScheduledTaskPrincipal { param($UserId,$LogonType,$RunLevel)
        if ($UserId -ne 'S-1-5-18' -or $LogonType -ne 'ServiceAccount') { throw 'wrong diagnostic OS identity' }
        return @{}
    }
    function global:New-ScheduledTaskAction { param($Execute,$Argument)
        if ($Execute -ne $global:ExpectedValidationImage -or $Argument -ne ('--config "' + $global:ExpectedValidationDeployment + '" --validate-persistent')) { throw 'diagnostic command was not fixed' }
        return @{}
    }
    function global:Start-ScheduledTask { param($TaskName,$ErrorAction) $global:Calls.Add('validation-start') }
    function global:Get-ScheduledTaskInfo { param($TaskName,$ErrorAction)
        $global:DiagnosticInfoCalls++
        if ($global:DiagnosticQueryFailure -and $global:DiagnosticInfoCalls -gt 1) { throw 'diagnostic lookup failed' }
        # The first post-start poll still looks unused; that must not count as a pass.
        return [pscustomobject]@{LastRunTime=if($global:DiagnosticInfoCalls -le 2){[datetime]::MinValue}else{[datetime]::UtcNow};LastTaskResult=$global:DiagnosticResult}
    }
    function global:Get-ScheduledTask { param($TaskName,$ErrorAction) return [pscustomobject]@{State='Ready'} }
    function global:Stop-ScheduledTask { param($TaskName,$ErrorAction)
        $global:Calls.Add('validation-stop')
        if ($global:DiagnosticStopFailed) { throw 'diagnostic stop unconfirmed' }
    }
    foreach ($Result in @(0,1)) {
        $global:Calls.Clear(); $global:DiagnosticResult=$Result; $global:DiagnosticInfoCalls=0
        try {
            Assert-SystemPersistent $Image $Deployment
            if ($Result -ne 0) { throw 'expected validation failure' }
        } catch {
            if ($Result -eq 0 -or $_.Exception.Message -notlike '*validation failed*') { throw }
        }
        if (($global:Calls -join ',') -ne 'register,validation-start,remove' -or $global:DiagnosticInfoCalls -ne 3) { throw 'diagnostic did not await its actual run or clean only its own task' }
    }
    foreach ($StopFailed in @($false,$true)) {
        $global:Calls.Clear(); $global:DiagnosticInfoCalls=0
        $global:DiagnosticQueryFailure=$true; $global:DiagnosticStopFailed=$StopFailed
        try { Assert-SystemPersistent $Image $Deployment; throw 'expected lookup failure' } catch {
            if ($_.Exception.Message -notlike '*diagnostic lookup failed*' -and $_.Exception.Message -notlike '*diagnostic stop unconfirmed*') { throw }
        }
        $Expected = if ($StopFailed) { 'register,validation-start,validation-stop' } else { 'register,validation-start,validation-stop,remove' }
        if (($global:Calls -join ',') -ne $Expected) { throw 'unconfirmed diagnostic cleanup removed its evidence' }
    }
}
if (!$IsWindows) {
    $Tokens=$null; $ParseErrors=$null
    $Ast=[Management.Automation.Language.Parser]::ParseFile($Script,[ref]$Tokens,[ref]$ParseErrors)
    if ($ParseErrors.Count) { throw ($ParseErrors | Out-String) }
    foreach ($Definition in $Ast.FindAll({param($Node) $Node -is [Management.Automation.Language.FunctionDefinitionAst]}, $false)) {
        . ([scriptblock]::Create($Definition.Extent.Text))
    }
    Test-SystemPersistent
    Write-Output 'Windows installer syntax and mocked LocalSystem validation passed; no Windows OS proof'
    return
}
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
    $global:Calls.Clear()
    try { & $Script -Action Refresh -Scope User; throw 'expected helper refresh refusal' } catch {
        if ($_.Exception.Message -notlike '*Remove/Install*') { throw }
    }
    if ($global:Calls.Count -ne 0) { throw 'helper refresh performed a mutation' }
    . $Script -Action Status -Scope User
    $ConfigPath=Join-Path $Root 'execution.json'
    [IO.File]::WriteAllText($ConfigPath,'original')
    [IO.File]::WriteAllText(($ConfigPath+'.refresh'),'existing evidence')
    try { Publish-Config $ConfigPath @{}; throw 'expected exclusive publication failure' } catch {
        if ($_.Exception.Message -eq 'expected exclusive publication failure') { throw }
    }
    if ([IO.File]::ReadAllText($ConfigPath+'.refresh') -ne 'existing evidence' -or [IO.File]::ReadAllText($ConfigPath) -ne 'original') { throw 'publication deleted another owner evidence' }
    Test-SystemPersistent
    Write-Output 'installer rollback and refresh ownership passed'
} finally {
    Remove-Item -LiteralPath $Root -Recurse -Force
}
