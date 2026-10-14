# ref: osquery 390c3472 osquery/tables/system/windows/programs.cpp (source discovery only).
# ref: PowerShell 5a6ecbf8 engine/Modules/ModuleCmdletBase.cs (WinPSCompatSession).
function Get-SoftwareItem {
    param([string]$Origin, [string]$Owner, [string]$View, [string]$NativeId, [string]$Name, [string]$Version, [string]$Publisher, [string]$Location)
    $null = Get-RequiredText $NativeId
    foreach ($value in @($Name, $Version, $Publisher, $Location)) {
        if ($value.Length -gt 4096 -or $value.Contains([char]0)) { throw [IO.InvalidDataException]::new('invalid software value') }
    }
    return [ordered]@{
        id = Get-ItemId @($Origin, $Owner.ToLowerInvariant(), $View, $NativeId.ToLowerInvariant())
        native_id = $NativeId; name = $Name; version = $Version; publisher = $Publisher
        scope = $(if ($Owner -eq '') { 'device' } else { 'user' }); owner = $Owner
        origin = $Origin; location = $Location
    }
}
function Read-Profiles {
    param([int]$TimeoutSeconds)
    $profiles = @(Get-CimInstance -ClassName Win32_UserProfile -Property SID,Loaded -OperationTimeoutSec $TimeoutSeconds -ErrorAction Stop)
    if ($profiles.Count -gt 1000) { throw [IO.InvalidDataException]::new('profile budget exceeded') }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $out = foreach ($p in $profiles) {
        $sid = Get-RequiredText $p.SID
        if ($sid -notmatch '^S-1-(\d+-)*\d+$' -or -not $seen.Add($sid)) { throw [IO.InvalidDataException]::new('invalid profile identity') }
        @{ sid = $sid; loaded = [bool]$p.Loaded }
    }
    return @($out | Sort-Object sid)
}
function Get-ProfileFingerprint {
    param($Profiles)
    return ConvertTo-Json -InputObject @($Profiles | Sort-Object sid | ForEach-Object { [ordered]@{ sid=$_.sid; loaded=$_.loaded } }) -Compress
}
function Read-UninstallView {
    param($Scan, $Items, [string]$Owner, [string]$View)
    $origin = "win32.uninstall.$View"
    $count = 0; $status = 'complete'; $base = $null; $hive = $null; $uninstall = $null
    try {
        Assert-ScanBudget $Scan
        $registryView = if ($View -eq '32') { [Microsoft.Win32.RegistryView]::Registry32 } else { [Microsoft.Win32.RegistryView]::Registry64 }
        $root = if ($Owner -eq '') { [Microsoft.Win32.RegistryHive]::LocalMachine } else { [Microsoft.Win32.RegistryHive]::Users }
        $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey($root, $registryView)
        $hive = if ($Owner -eq '') { $base } else { $base.OpenSubKey($Owner, $false) }
        if ($null -eq $hive) { throw [UnauthorizedAccessException]::new('profile hive unavailable') }
        $uninstall = $hive.OpenSubKey('SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall', $false)
        if ($null -ne $uninstall) {
            foreach ($name in @($uninstall.GetSubKeyNames() | Sort-Object)) {
                Assert-ScanBudget $Scan
                $entry = $null
                try {
                    $entry = $uninstall.OpenSubKey($name, $false)
                    if ($null -eq $entry) { throw [IO.IOException]::new('entry disappeared') }
                    # Never read or invoke UninstallString/QuietUninstallString.
                    $item = Get-SoftwareItem $origin $Owner $View $name ([string]$entry.GetValue('DisplayName', '')) ([string]$entry.GetValue('DisplayVersion', '')) ([string]$entry.GetValue('Publisher', '')) ([string]$entry.GetValue('InstallLocation', ''))
                    if (-not (Add-ScanItem $Scan $Items $item)) { $status = 'truncated'; break }
                    $count++
                } finally { if ($null -ne $entry) { $entry.Dispose() } }
            }
        }
    } catch { $status = Get-SafeStatus $_ }
    finally {
        if ($null -ne $uninstall) { $uninstall.Dispose() }
        if ($null -ne $hive -and $Owner -ne '') { $hive.Dispose() }
        if ($null -ne $base) { $base.Dispose() }
    }
    Add-Coverage $Scan $origin $Owner $View $status $count
}
function Add-PackagedRows {
    param($Scan, $Items, $Rows, [bool]$Provisioned)
    $origin = if ($Provisioned) { 'msix.provisioned' } else { 'msix.registered' }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $count = 0; $status = 'complete'
    try {
        foreach ($r in $Rows) {
            Assert-ScanBudget $Scan
            if ($Provisioned) {
                $owner = ''; $native = Get-RequiredText $r.package_full_name
            } else {
                $owner = Get-RequiredText $r.sid
                if ($owner -notmatch '^S-1-(\d+-)*\d+$') { throw [IO.InvalidDataException]::new('invalid package owner') }
                $native = Get-RequiredText $r.package_full_name
                if ($r.state -ne 'Installed') {
                    # Staged/noninstalled registrations are evidence, never installed applications.
                    $status = 'partial'; continue
                }
            }
            $item = Get-SoftwareItem $origin $owner '' $native ([string]$r.name) ([string]$r.version) ([string]$r.publisher) ([string]$r.location)
            if (-not $seen.Add($item.id)) { throw [IO.InvalidDataException]::new('duplicate package identity') }
            if (-not (Add-ScanItem $Scan $Items $item)) { $status = 'truncated'; break }
            $count++
        }
    } catch { $status = Get-SafeStatus $_ }
    Add-Coverage $Scan $origin '' '' $status $count
}
function Read-PackagedSoftware {
    param($Scan, $Installations, $Packages)
    # Module path and invoked commands are fixed; caller data cannot select executable code.
    # Projection must occur in 5.1, before remoting serializes WinRT objects.
    foreach ($kind in @('registered', 'provisioned')) {
        $module = if ($kind -eq 'registered') { 'Appx' } else { 'Dism' }
        try {
            Assert-ScanBudget $Scan
            $system = [Environment]::GetFolderPath([Environment+SpecialFolder]::System)
            $path = Join-Path $system "WindowsPowerShell\v1.0\Modules\$module\$module.psd1"
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw [NotSupportedException]::new('module unavailable') }
            Import-Module -Name $path -UseWindowsPowerShell -WarningAction SilentlyContinue -ErrorAction Stop
            $session = Get-PSSession -Name WinPSCompatSession -ErrorAction Stop
            $limit = $Scan.limit
            $raw = Invoke-Command -Session $session -ArgumentList $kind,$limit,$Scan.bytes -WarningAction SilentlyContinue -ErrorAction Stop -ScriptBlock {
                param($kind, $limit, $maxBytes)
                $ErrorActionPreference = 'Stop'
                $ProgressPreference = 'SilentlyContinue'
                $WarningPreference = 'SilentlyContinue'
                $rows = [Collections.Generic.List[object]]::new()
                $overflow = $false; $rowBytes = 0
                if ($kind -eq 'registered') {
                    foreach ($p in @(Get-AppxPackage -AllUsers -PackageTypeFilter Main,Framework,Resource,Bundle,Optional -ErrorAction Stop | Select-Object -First ($limit + 1))) {
                        if ($overflow -or $rows.Count -gt $limit) { $overflow = $true; break }
                        $users = @($p.PackageUserInformation)
                        if ($users.Count -eq 0) { throw 'package registration unavailable' }
                        foreach ($u in $users) {
                            if ($overflow -or $rows.Count -gt $limit) { $overflow = $true; break }
                            $row = @{ package_full_name=[string]$p.PackageFullName; family=[string]$p.PackageFamilyName; architecture=[string]$p.Architecture; resource_id=[string]$p.ResourceId; name=[string]$p.Name; version=[string]$p.Version; publisher=[string]$p.Publisher; location=[string]$p.InstallLocation; sid=[string]$u.UserSecurityId; state=[string]$u.InstallState }
                            $rowBytes += [Text.Encoding]::UTF8.GetByteCount((ConvertTo-Json -InputObject $row -Compress))
                            if ($rowBytes -gt $maxBytes) { $overflow = $true; break }
                            $rows.Add($row)
                        }
                    }
                } else {
                    foreach ($p in @(Get-AppxProvisionedPackage -Online -ErrorAction Stop | Select-Object -First ($limit + 1))) {
                        if ($overflow -or $rows.Count -gt $limit) { $overflow = $true; break }
                        $row = @{ package_full_name=[string]$p.PackageName; family=''; architecture=[string]$p.Architecture; resource_id=[string]$p.ResourceId; name=[string]$p.DisplayName; version=[string]$p.Version; publisher=''; location='' }
                        $rowBytes += [Text.Encoding]::UTF8.GetByteCount((ConvertTo-Json -InputObject $row -Compress))
                        if ($rowBytes -gt $maxBytes) { $overflow = $true; break }
                        $rows.Add($row)
                    }
                }
                ConvertTo-Json -InputObject @{ rows=@($rows.ToArray()); overflow=$overflow } -Depth 8 -Compress
            }
            $decoded = ConvertFrom-Json -InputObject $raw -AsHashtable
            $Scan.data[$kind] = $decoded.rows
            if ($kind -eq 'registered') { Add-PackagedRows $Scan $Installations $decoded.rows $false }
            else { Add-PackagedRows $Scan $Packages $decoded.rows $true }
            if ($decoded.overflow) { Add-Coverage $Scan "msix.$kind" '' '' 'truncated' }
        } catch { Add-Coverage $Scan "msix.$kind" '' '' (Get-SafeStatus $_) }
    }
}
function Invoke-SoftwareScan {
    param([int]$MaxItems, [int]$MaxBytes, [int]$TimeoutSeconds)
    $scan = New-Scan 'windows.software' $MaxItems $MaxBytes $TimeoutSeconds
    $installations = [Collections.Generic.List[object]]::new()
    $packages = [Collections.Generic.List[object]]::new()
    try {
        if (-not $IsWindows) { throw [PlatformNotSupportedException]::new('windows required') }
        $profiles = @(Read-Profiles (Get-RemainingSeconds $scan))
        $scan.data.profiles = $profiles
        Add-Coverage $scan 'profiles' '' '' 'complete' $profiles.Count
        $views = if ([Environment]::Is64BitOperatingSystem) { @('32','64') } else { @('32') }
        foreach ($view in $views) { Read-UninstallView $scan $installations '' $view }
        foreach ($profile in $profiles) {
            foreach ($view in $views) {
                if ($profile.loaded) { Read-UninstallView $scan $installations $profile.sid $view }
                else { Add-Coverage $scan "win32.uninstall.$view" $profile.sid $view 'unsupported' }
            }
        }
        Read-PackagedSoftware $scan $installations $packages
        Assert-ScanBudget $scan
        $after = @(Read-Profiles (Get-RemainingSeconds $scan))
        if ((Get-ProfileFingerprint $profiles) -ne (Get-ProfileFingerprint $after)) { Add-Coverage $scan 'profiles.changed' '' '' 'partial' }
    } catch { Add-Coverage $scan 'scan' '' '' (Get-SafeStatus $_) }
    $scan.fields['device.software.installations'] = @($installations.ToArray() | Sort-Object id)
    $scan.fields['device.software.packages'] = @($packages.ToArray() | Sort-Object id)
    $scan.data.installations = $scan.fields['device.software.installations']
    $scan.data.packages = $scan.fields['device.software.packages']
    return $scan
}
if ($MyInvocation.InvocationName -ne '.') {
    Write-ScanResult (Invoke-SoftwareScan $MaxItems $MaxBytes $TimeoutSeconds)
}
