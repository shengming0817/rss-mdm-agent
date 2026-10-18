# Lab adapter. The installed Rust service owns SCM, LocalSystem validation and login tasks.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Install','Refresh','Remove','Status','Apply','Import')][string]$Action,
    [Parameter(Mandatory)][ValidateSet('System','User')][string]$Scope,
    [Parameter(Mandatory)][string]$Binary,
    [string]$Config,
    [string]$CandidateBinary,
    [string]$CandidateConfig,
    [string]$Candidate,
    [string]$Profile,
    [string]$Configuration,
    [string]$PackagePath
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (![IO.Path]::IsPathFullyQualified($Binary)) { throw 'Absolute service path required' }
$Arguments = @('--installation', $Action.ToLowerInvariant(), '--scope', $Scope.ToLowerInvariant(), '--binary', $Binary)
foreach ($Pair in @(@('config',$Config),@('candidate-binary',$CandidateBinary),@('candidate-config',$CandidateConfig),@('candidate',$Candidate),@('profile',$Profile),@('configuration',$Configuration),@('package-path',$PackagePath))) {
    if ($Pair[1]) { $Arguments += @('--' + $Pair[0], $Pair[1]) }
}
& $Binary @Arguments
if ($LASTEXITCODE -ne 0) { throw 'Native installation operation failed; retained state must be inspected' }
