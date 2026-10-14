# Included in each single-file resource by export.py. No remote modules or dynamic commands.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function New-Scan {
    param([string]$Collector, [int]$MaxItems, [int]$MaxBytes, [int]$TimeoutSeconds)
    return @{
        format = 1; collector = $Collector; version = 1; scope = 'device'
        fields = @{}; data = @{}; coverage = [Collections.Generic.List[object]]::new()
        limit = $MaxItems; bytes = $MaxBytes; count = 0; item_bytes = 0
        clock = [Diagnostics.Stopwatch]::StartNew(); deadline = $TimeoutSeconds
    }
}
function Assert-ScanBudget {
    param($Scan)
    if ($Scan.clock.Elapsed.TotalSeconds -ge $Scan.deadline) { throw [TimeoutException]::new('deadline') }
}
function Get-SafeStatus {
    param($ErrorRecord)
    if ($ErrorRecord.CategoryInfo.Category -in @('PermissionDenied','SecurityError')) { return 'denied' }
    $e = $ErrorRecord.Exception
    while ($null -ne $e.InnerException) { $e = $e.InnerException }
    if ($e -is [UnauthorizedAccessException] -or $e -is [Security.SecurityException]) { return 'denied' }
    if ($e -is [PlatformNotSupportedException] -or $e -is [NotSupportedException]) { return 'unsupported' }
    if ($e -is [TimeoutException]) { return 'partial' }
    return 'failed'
}
function Add-Coverage {
    param($Scan, [string]$Source, [string]$Owner, [string]$View, [string]$Status, [int]$Count = 0)
    if ($Scan.coverage.Count -ge 4096) { throw [InvalidOperationException]::new('coverage limit') }
    $Scan.coverage.Add([ordered]@{ source = $Source; owner = $Owner; view = $View; status = $Status; count = $Count })
}
function Get-ItemId {
    param([string[]]$Parts)
    # Length-delimited canonical coordinates; display names and enumeration order are not identities.
    $text = ($Parts | ForEach-Object { '{0}:{1}' -f ([Text.Encoding]::UTF8.GetByteCount($_)), $_ }) -join '|'
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($text))).ToLowerInvariant()
}
function Add-ScanItem {
    param($Scan, $Items, $Item)
    Assert-ScanBudget $Scan
    if ($Scan.count -ge $Scan.limit) { return $false }
    $cost = [Text.Encoding]::UTF8.GetByteCount((ConvertTo-Json -InputObject $Item -Depth 16 -Compress))
    if ($Scan.item_bytes + $cost -gt $Scan.bytes) { return $false }
    $Scan.item_bytes += $cost
    $Scan.count++
    $Items.Add($Item)
    return $true
}
function Get-ScanResult {
    param($Scan)
    $states = @($Scan.coverage | ForEach-Object { $_.status })
    $complete = $states.Count -gt 0 -and @($states | Where-Object { $_ -ne 'complete' }).Count -eq 0
    $status = if ($complete) { 'complete' } elseif ($states -contains 'truncated') { 'truncated' } elseif ($states -contains 'complete') { 'partial' } elseif ($states.Count -gt 0 -and @($states | Select-Object -Unique).Count -eq 1) { $states[0] } else { 'failed' }
    $result = [ordered]@{
        format = $Scan.format; collector = $Scan.collector; version = $Scan.version
        scope = $Scan.scope; status = $status; coverage = @($Scan.coverage.ToArray())
        fields = $(if ($complete) { $Scan.fields } else { @{} }); evidence = $Scan.data
    }
    $json = ConvertTo-Json -InputObject $result -Depth 32 -Compress
    if ([Text.Encoding]::UTF8.GetByteCount($json) -gt $Scan.bytes) {
        $result.status = 'truncated'; $result.fields = @{}
        # Never trim a list into an apparently complete replacement. Keep only bounded scan metadata.
        $result.evidence = @{ reason = 'output_limit'; collected_items = $Scan.count }
        $json = ConvertTo-Json -InputObject $result -Depth 32 -Compress
        if ([Text.Encoding]::UTF8.GetByteCount($json) -gt $Scan.bytes) {
            $result.coverage = @(); $result.evidence.coverage_omitted = $true
            $json = ConvertTo-Json -InputObject $result -Depth 32 -Compress
        }
    }
    return @{ json = $json; exit = $(if ($result.status -eq 'complete') { 0 } else { 2 }) }
}
function Write-ScanResult {
    param($Scan)
    $result = Get-ScanResult $Scan
    [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
    [Console]::WriteLine($result.json)
    exit $result.exit
}
function Get-RequiredText {
    param($Value)
    $text = [string]$Value
    if ([string]::IsNullOrWhiteSpace($text) -or $text.Length -gt 4096 -or $text.Contains([char]0)) { throw [IO.InvalidDataException]::new('invalid text') }
    return $text
}
function Get-BytesValue {
    param($Value)
    if ($null -eq $Value) { throw [IO.InvalidDataException]::new('missing capacity') }
    $number = [long]::Parse([string]$Value, [Globalization.CultureInfo]::InvariantCulture)
    if ($number -lt 0) { throw [IO.InvalidDataException]::new('negative capacity') }
    return $number
}
