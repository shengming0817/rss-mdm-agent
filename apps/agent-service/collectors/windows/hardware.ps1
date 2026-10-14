# ref: osquery 390c3472 osquery/tables/system/windows/{system_info,cpu_info,disk_info}.cpp
function Convert-Hardware {
    param([string]$Section, $Rows)
    $fields = @{}
    switch ($Section) {
        'identity' {
            if ($Rows.Count -ne 1) { throw [IO.InvalidDataException]::new('system identity count') }
            $r = $Rows[0]
            $fields['device.manufacturer'] = Get-RequiredText $r.Vendor
            $fields['device.model'] = Get-RequiredText $r.Name
            $serial = Get-RequiredText $r.IdentifyingNumber
            if ($serial -match '^(?i:0+|unknown|none|default string|system serial number|to be filled by o\.e\.m\.)$') { throw [IO.InvalidDataException]::new('placeholder identity') }
            $uuid = [guid]::Parse([string]$r.UUID)
            if ($uuid -eq [guid]::Empty -or $uuid -eq [guid]'ffffffff-ffff-ffff-ffff-ffffffffffff') { throw [IO.InvalidDataException]::new('placeholder uuid') }
            $fields['device.serial_number'] = $serial
            $fields['device.hardware_uuid'] = $uuid.ToString()
        }
        'os' {
            if ($Rows.Count -ne 1) { throw [IO.InvalidDataException]::new('os count') }
            $r = $Rows[0]
            $fields['device.os.name'] = Get-RequiredText $r.Caption
            $fields['device.os.version'] = Get-RequiredText $r.Version
            $fields['device.os.build'] = Get-RequiredText $r.BuildNumber
            $fields['device.os.edition'] = Get-RequiredText $r.Edition
            $fields['device.architecture'] = Get-RequiredText $r.Architecture
        }
        'cpu' {
            if ($Rows.Count -eq 0) { throw [IO.InvalidDataException]::new('missing cpu') }
            $physical = 0L; $logical = 0L
            foreach ($r in $Rows) { $physical += Get-BytesValue $r.NumberOfCores; $logical += Get-BytesValue $r.NumberOfLogicalProcessors }
            if ($physical -eq 0 -or $logical -eq 0) { throw [IO.InvalidDataException]::new('invalid cores') }
            $fields['device.cpu.model'] = (@($Rows | ForEach-Object { Get-RequiredText $_.Name } | Sort-Object -Unique) -join '; ')
            $fields['device.cpu.physical_cores'] = $physical; $fields['device.cpu.logical_cores'] = $logical
        }
        'memory' {
            if ($Rows.Count -ne 1) { throw [IO.InvalidDataException]::new('memory count') }
            # Matches system_info.physical_memory: OS-visible physical memory, not DIMM installed capacity.
            $fields['device.memory.bytes'] = Get-BytesValue $Rows[0].TotalPhysicalMemory
        }
        'firmware' {
            if ($Rows.Count -ne 1) { throw [IO.InvalidDataException]::new('firmware count') }
            $fields['device.firmware.version'] = Get-RequiredText $Rows[0].SMBIOSBIOSVersion
        }
        'battery' {
            if ($Rows.Count -eq 0) { throw [NotSupportedException]::new('no battery') }
            if ($Rows.Count -ne 1) { throw [NotSupportedException]::new('multiple battery aggregation undefined') }
            if ($null -eq $Rows[0].EstimatedChargeRemaining) { throw [IO.InvalidDataException]::new('missing charge') }
            $charge = [double]$Rows[0].EstimatedChargeRemaining
            if ($charge -lt 0 -or $charge -gt 100) { throw [IO.InvalidDataException]::new('invalid charge') }
            $fields['device.battery.charge_percent'] = $charge
        }
        { $_ -in 'disks','volumes','network' } {
            $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
            $items = [Collections.Generic.List[object]]::new()
            foreach ($r in $Rows) {
                $native = switch ($Section) { 'disks' { Get-RequiredText $r.PNPDeviceID } 'volumes' { Get-RequiredText $r.DeviceID } 'network' { ([guid]::Parse([string]$r.InterfaceGuid)).ToString() } }
                if (-not $seen.Add($native)) { throw [IO.InvalidDataException]::new('duplicate native identity') }
                $id = Get-ItemId @($Section, $native.ToLowerInvariant())
                $item = switch ($Section) {
                    'disks' { @{ id = $id; name = (Get-RequiredText $r.Model); size_bytes = (Get-BytesValue $r.Size) } }
                    'volumes' { @{ id = $id; name = [string]$r.Label; filesystem = (Get-RequiredText $r.FileSystem); size_bytes = (Get-BytesValue $r.Capacity); free_bytes = (Get-BytesValue $r.FreeSpace); system_volume = [bool]$r.BootVolume } }
                    'network' { @{ id = $id; name = (Get-RequiredText $r.Name); addresses = @($r.Addresses | ForEach-Object { Get-RequiredText $_ } | Sort-Object -Unique) } }
                }
                $items.Add($item)
            }
            $key = switch ($Section) { 'disks' { 'device.storage.devices' } 'volumes' { 'device.storage.volumes' } 'network' { 'device.network.interfaces' } }
            $fields[$key] = @($items.ToArray() | Sort-Object id)
        }
    }
    return $fields
}
function Read-Hardware {
    param([string]$Section, [int]$TimeoutSeconds)
    $class = switch ($Section) {
        'identity' { 'Win32_ComputerSystemProduct' } 'os' { 'Win32_OperatingSystem' }
        'cpu' { 'Win32_Processor' } 'memory' { 'Win32_ComputerSystem' }
        'firmware' { 'Win32_BIOS' } 'disks' { 'Win32_DiskDrive' }
        'volumes' { 'Win32_Volume' } 'battery' { 'Win32_Battery' }
        'network' { 'MSFT_NetAdapter' }
    }
    $namespace = if ($Section -eq 'network') { 'root/StandardCimv2' } else { 'root/cimv2' }
    $rows = @(Get-CimInstance -Namespace $namespace -ClassName $class -OperationTimeoutSec $TimeoutSeconds -ErrorAction Stop)
    if ($Section -eq 'os') {
        $key = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine, [Microsoft.Win32.RegistryView]::Registry64)
        try {
            $current = $key.OpenSubKey('SOFTWARE\Microsoft\Windows NT\CurrentVersion', $false)
            try { $edition = $current.GetValue('EditionID') } finally { if ($null -ne $current) { $current.Dispose() } }
        } finally { $key.Dispose() }
        $rows = @($rows | ForEach-Object { @{ Caption=$_.Caption; Version=$_.Version; BuildNumber=$_.BuildNumber; Edition=$edition; Architecture=[Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant() } })
    }
    if ($Section -eq 'network') {
        $addresses = @(Get-CimInstance -Namespace root/StandardCimv2 -ClassName MSFT_NetIPAddress -OperationTimeoutSec $TimeoutSeconds -ErrorAction Stop)
        $rows = @($rows | ForEach-Object { $index = $_.InterfaceIndex; @{ InterfaceGuid=$_.InterfaceGuid; Name=$_.Name; Addresses=@($addresses | Where-Object { $_.InterfaceIndex -eq $index } | ForEach-Object { $_.IPAddress }) } })
    }
    return $rows
}
function Invoke-HardwareScan {
    param([string]$Section, [int]$MaxItems, [int]$MaxBytes, [int]$TimeoutSeconds)
    $scan = New-Scan "windows.hardware.$Section" $MaxItems $MaxBytes $TimeoutSeconds
    try {
        if (-not $IsWindows) { throw [PlatformNotSupportedException]::new('windows required') }
        $rows = @(Read-Hardware $Section (Get-RemainingSeconds $scan))
        Assert-ScanBudget $scan
        if ($rows.Count -gt $MaxItems) { Add-Coverage $scan $Section '' '' 'truncated' $MaxItems }
        else {
            $scan.fields = Convert-Hardware $Section $rows
            $scan.count = $rows.Count
            Add-Coverage $scan $Section '' '' 'complete' $rows.Count
        }
    } catch { Add-Coverage $scan $Section '' '' (Get-SafeStatus $_) }
    return $scan
}
if ($MyInvocation.InvocationName -ne '.') {
    Write-ScanResult (Invoke-HardwareScan $Section $MaxItems $MaxBytes $TimeoutSeconds)
}
