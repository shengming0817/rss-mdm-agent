param([Parameter(Mandatory)][string]$Resources)
$ErrorActionPreference = 'Stop'
function Assert($Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
foreach ($name in @('hardware','software')) {
    $file = Join-Path $Resources "windows-$name-v1.ps1"
    $errors = $null
    $null = [Management.Automation.Language.Parser]::ParseFile($file, [ref]$null, [ref]$errors)
    Assert ($errors.Count -eq 0) "invalid $name syntax: $errors"
}
. (Join-Path $Resources 'windows-hardware-v1.ps1') -Section cpu
$cpus = @(@{Name='CPU';NumberOfCores=4;NumberOfLogicalProcessors=8}, @{Name='CPU';NumberOfCores=6;NumberOfLogicalProcessors=12})
$hardware = Convert-Hardware cpu $cpus
Assert ($hardware['device.cpu.physical_cores'] -eq 10) 'physical cores must sum sockets'
Assert ($hardware['device.cpu.logical_cores'] -eq 20) 'logical cores must sum sockets'
Assert ((Convert-Hardware memory @(@{TotalPhysicalMemory='8589934592'}))['device.memory.bytes'] -eq 8589934592L) 'memory bytes must not overflow 32 bits'
$volume = @{DeviceID='\\?\Volume{11111111-1111-1111-1111-111111111111}\';Label='old';FileSystem='NTFS';Capacity=4096;FreeSpace=2048;BootVolume=$true}
$a = (Convert-Hardware volumes @($volume))['device.storage.volumes'][0]
$volume.Label = 'renamed'
$b = (Convert-Hardware volumes @($volume))['device.storage.volumes'][0]
Assert ($a.id -eq $b.id) 'volume identity must survive a label change'
foreach ($serial in @('To Be Filled By O.E.M.','00000000','System Serial Number')) {
    $rejected = $false
    try { $null = Convert-Hardware identity @(@{Vendor='VM';Name='VM';IdentifyingNumber=$serial;UUID='11111111-1111-1111-1111-111111111111'}) } catch { $rejected = $true }
    Assert $rejected 'placeholder identity accepted'
}
$rejected = $false
try { $null = Convert-Hardware battery @() } catch { $rejected = (Get-SafeStatus $_) -eq 'unsupported' }
Assert $rejected 'no battery must be unsupported, not zero percent'
. (Join-Path $Resources 'windows-software-v1.ps1')
$first = Get-SoftwareItem 'win32.uninstall.64' 'S-1-5-21-1' '64' '{product}' 'App' '1' 'Publisher' 'C:\App'
$upgrade = Get-SoftwareItem 'win32.uninstall.64' 'S-1-5-21-1' '64' '{product}' 'Renamed' '2' 'Publisher' 'D:\App'
$other = Get-SoftwareItem 'win32.uninstall.64' 'S-1-5-21-2' '64' '{product}' 'App' '1' 'Publisher' ''
$view32 = Get-SoftwareItem 'win32.uninstall.32' 'S-1-5-21-1' '32' '{product}' 'App' '1' 'Publisher' ''
Assert ($first.id -eq $upgrade.id) 'upgrade/name/path must not change ARP identity'
Assert ($first.id -ne $other.id -and $first.id -ne $view32.id) 'SID and registry view must remain distinct'
Assert ((Get-ItemId @('a|b','c')) -ne (Get-ItemId @('a','b|c'))) 'identity framing collision'
$rows = @(
    @{package_full_name='App_1.0_x64__publisher';name='App';version='1.0';publisher='Publisher';location='';sid='S-1-5-21-1';state='Installed'},
    @{package_full_name='App_2.0_x64__publisher';name='App';version='2.0';publisher='Publisher';location='';sid='S-1-5-21-1';state='Installed'},
    @{package_full_name='App_2.0_x64__publisher';name='App';version='2.0';publisher='Publisher';location='';sid='S-1-5-21-2';state='Installed'}
)
$scan = New-Scan software 100 65536 120
$items = [Collections.Generic.List[object]]::new()
Add-PackagedRows $scan $items $rows $false
Assert ($items.Count -eq 3 -and @($items | ForEach-Object {$_.id} | Select-Object -Unique).Count -eq 3) 'MSIX parallel versions/users were collapsed'
$packages = [Collections.Generic.List[object]]::new()
Add-PackagedRows $scan $packages @($rows[1]) $true
Assert ($packages.Count -eq 1 -and $packages[0].scope -eq 'device' -and $packages[0].id -ne $items[1].id) 'provisioning is not user registration'
$scan.fields['device.software.installations'] = @($items.ToArray())
$scan.data.installations = @($items.ToArray())
Assert ((Get-ScanResult $scan).exit -eq 0) 'full scope should be complete'
Add-Coverage $scan 'win32.uninstall.64' 'S-1-5-21-3' '64' unsupported
$partial = Get-ScanResult $scan
$body = $partial.json | ConvertFrom-Json -AsHashtable
Assert ($partial.exit -ne 0 -and $body.fields.Count -eq 0 -and $body.evidence.installations.Count -eq 3) 'partial scope must retain evidence without replacement fields'
$scan = New-Scan software 100 65536 120
Add-Coverage $scan 'win32.uninstall.64' '' '64' complete 0
$scan.fields['device.software.installations'] = @()
$empty = (Get-ScanResult $scan).json | ConvertFrom-Json -AsHashtable
Assert ($empty.status -eq 'complete' -and $empty.fields['device.software.installations'].Count -eq 0) 'legitimate zero items must remain complete'
foreach ($state in @('unsupported','denied','failed','partial','truncated')) {
    $scan = New-Scan software 100 65536 120
    Add-Coverage $scan 'source' '' '' $state
    $result = Get-ScanResult $scan
    Assert ($result.exit -ne 0 -and ($result.json | ConvertFrom-Json).status -eq $state) "lost $state classification"
}
$scan = New-Scan software 1 65536 120
$items = [Collections.Generic.List[object]]::new()
Add-PackagedRows $scan $items $rows $false
Assert ($items.Count -eq 1 -and ((Get-ScanResult $scan).json | ConvertFrom-Json).status -eq 'truncated') 'item budget not enforced'
$scan = New-Scan software 100 1024 120
Add-Coverage $scan source '' '' complete
$scan.fields['device.software.installations'] = @(@{name=('x'*2000)})
$scan.data.items = $scan.fields['device.software.installations']
$result = Get-ScanResult $scan
$body = $result.json | ConvertFrom-Json -AsHashtable
Assert ($result.exit -ne 0 -and $body.status -eq 'truncated' -and $body.fields.Count -eq 0 -and [Text.Encoding]::UTF8.GetByteCount($result.json) -le 1024) 'byte budget cannot turn truncated data into a full list'
$before = @(@{sid='S-1-5-21-2';loaded=$true}, @{sid='S-1-5-21-1';loaded=$false})
$after = @($before[1], $before[0])
Assert ((Get-ProfileFingerprint $before) -eq (Get-ProfileFingerprint $after)) 'profile ordering is not a coverage change'
$after = @(@{sid='S-1-5-21-1';loaded=$true}, $before[0])
Assert ((Get-ProfileFingerprint $before) -ne (Get-ProfileFingerprint $after)) 'profile availability change must invalidate complete scan'
$scan = New-Scan software 100 65536 120
$items = [Collections.Generic.List[object]]::new()
Add-PackagedRows $scan $items @($rows[0], $rows[0]) $false
Assert ((Get-ScanResult $scan).exit -ne 0) 'duplicate item identity must not be complete'
$scan = New-Scan software 100 65536 120
Add-Coverage $scan source '' '' complete
$scan.fields['device.software.installations'] = @()
# Deterministic frozen clock: no sleep or Windows provider needed to reproduce late completion.
$scan.clock = @{Elapsed = [TimeSpan]::FromSeconds(121)}
$result = Get-ScanResult $scan
$late = $result.json | ConvertFrom-Json -AsHashtable
Assert ($result.exit -ne 0 -and $late.status -eq 'partial' -and $late.fields.Count -eq 0) 'late completion must never replace inventory'
# Exercise the production compatibility path with host-only provider substitutes.
& {
    function Join-Path { param($Path, $ChildPath) return $ChildPath }
    function Test-Path { param($LiteralPath, $PathType) return $true }
    function Import-Module {
        [CmdletBinding()] param($Name, [switch]$UseWindowsPowerShell)
        Write-Warning 'Compatibility proxy warning'
    }
    function Get-PSSession { [CmdletBinding()] param($Name) return 'fake-session' }
    function Invoke-Command {
        [CmdletBinding()] param($Session, $ArgumentList, $ScriptBlock)
        Write-Warning 'Remote provider warning'
        return '{"rows":[],"overflow":false}'
    }
    $scan = New-Scan software 100 65536 120
    $items = [Collections.Generic.List[object]]::new()
    $packages = [Collections.Generic.List[object]]::new()
    $output = @(Read-PackagedSoftware $scan $items $packages 3>&1)
    Assert ($output.Count -eq 0) 'compatibility warnings must not contaminate JSON stdout'
    Assert ($scan.coverage.Count -eq 2 -and (Get-ScanResult $scan).exit -eq 0) 'fake compatibility path must complete both sources'
}
Write-Output 'Windows collector pure logic and production script syntax passed (Windows APIs not executed)'
