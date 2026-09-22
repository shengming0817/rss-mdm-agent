param([Parameter(Mandatory=$true)][int]$ParentPid)
$ErrorActionPreference = "Stop"
if ($ParentPid -le 1) { throw "Invalid owned parent PID." }
$scopes = @()
foreach ($process in Get-CimInstance Win32_Process -Filter "ParentProcessId=$ParentPid") {
    if ($process.Name -ne "rss-ai-worker-launcher.exe") { continue }
    if ($process.CommandLine -notmatch ' launch ([a-f0-9-]{36})$') { throw "Unexpected worker invocation." }
    $scopes += @{kind="jobObject";name="Local\rss-mdm-worker-$($Matches[1])";session=[int]$process.SessionId}
}
ConvertTo-Json -InputObject $scopes -Compress
