#requires -Version 5.1
#requires -PSEdition Desktop
param(
    [Parameter(Mandatory=$true)][string]$DesktopDirectory,
    [Parameter(Mandatory=$true)][string]$ServiceExecutable,
    [Parameter(Mandatory=$true)][string]$ProbeExecutable,
    [Parameter(Mandatory=$true)][string[]]$AllowedUserSid
)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if (-not ([Security.Principal.WindowsPrincipal]$identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Administrator required.' }
if ([Environment]::OSVersion.Version.Build -lt 22000 -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') { throw 'Windows 11 x64 required.' }
$name = 'RssMdmAgentStatus'
$product = Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'RSS MDM Agent'
$dataRoot = Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'RSS MDM Agent'
$root = Join-Path $dataRoot 'service'
$service = Join-Path $root 'rss-local-service.exe'
$probe = Join-Path $root 'rss-untrusted-service-probe.exe'
$desktop = Join-Path $product 'rss-mdm-desktop.exe'
foreach ($sid in $AllowedUserSid) { $null = [Security.Principal.SecurityIdentifier]::new($sid) }
if (Test-Path -LiteralPath $product) { throw 'Inspect and remove the previous laboratory app before reinstalling; existing service has not been stopped.' }
$manifest = Get-Content (Join-Path $DesktopDirectory 'desktop-manifest.json') -Raw | ConvertFrom-Json
if ($manifest.platform -ne 'win32' -or $manifest.arch -ne 'x64' -or
    (Get-FileHash (Join-Path $DesktopDirectory 'rss-mdm-desktop.exe') -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.executableSha256 -or
    -not (Test-Path (Join-Path $DesktopDirectory 'ai-host-runtime/worker-manifest.json'))) { throw 'Fixed expanded desktop candidate required.' }
if (-not (Test-Path -LiteralPath $ServiceExecutable -PathType Leaf) -or
    -not (Test-Path -LiteralPath $ProbeExecutable -PathType Leaf)) { throw 'Service and probe artifacts required.' }
function Restricted-Acl([string[]]$Readers) {
    $acl = [Security.AccessControl.DirectorySecurity]::new()
    $acl.SetAccessRuleProtection($true, $false)
    $acl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
    foreach ($sid in @('S-1-5-18', 'S-1-5-32-544')) {
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($sid), 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow'))
    }
    foreach ($sid in $Readers) {
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($sid), 'ReadAndExecute', 'ContainerInherit,ObjectInherit', 'None', 'Allow'))
    }
    return $acl
}
function Assert-ProtectedTree([string]$Path) {
    $pending = [Collections.Generic.Queue[string]]::new(); $pending.Enqueue($Path)
    while ($pending.Count -gt 0) {
        $item = Get-Item -Force -LiteralPath $pending.Dequeue()
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse installation path rejected.' }
        $acl = Get-Acl -LiteralPath $item.FullName
        $raw = [Security.AccessControl.RawSecurityDescriptor]::new($acl.GetSecurityDescriptorBinaryForm(), 0)
        if ($null -eq $raw.DiscretionaryAcl) { throw 'Null installation DACL rejected.' }
        $owner = $acl.GetOwner([Security.Principal.SecurityIdentifier]).Value
        if ($owner -notin @('S-1-5-18', 'S-1-5-32-544')) { throw 'Untrusted installation owner.' }
        foreach ($rule in $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
            $write = [Security.AccessControl.FileSystemRights]::Write -bor [Security.AccessControl.FileSystemRights]::Delete -bor
                [Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles -bor [Security.AccessControl.FileSystemRights]::ChangePermissions -bor [Security.AccessControl.FileSystemRights]::TakeOwnership -bor [long]0x40000000 -bor [long]0x10000000
            if ($rule.AccessControlType -eq 'Allow' -and ($rule.FileSystemRights -band $write) -and $rule.IdentityReference.Value -notin @('S-1-5-18','S-1-5-32-544')) { throw 'Untrusted installation writer.' }
        }
        if ($item.PSIsContainer) { foreach ($child in Get-ChildItem -Force -LiteralPath $item.FullName) { $pending.Enqueue($child.FullName) } }
    }
}
function New-ProtectedDirectory([string]$Path) {
    if (Test-Path -LiteralPath $Path) { Assert-ProtectedTree $Path; return }
    # DirectorySecurity is passed at creation; no open interval before tightening an inherited ACL.
    [IO.Directory]::CreateDirectory($Path, (Restricted-Acl @())) | Out-Null
    Assert-ProtectedTree $Path
}
function Copy-Locked([string]$Source, [string]$Destination) {
    $sourceStream = [IO.File]::Open($Source, 'Open', 'Read', 'Read')
    try {
        $targetStream = [IO.File]::Open($Destination, 'CreateNew', 'Write', 'None')
        try { $sourceStream.CopyTo($targetStream); $targetStream.Flush($true) } finally { $targetStream.Dispose() }
    } finally { $sourceStream.Dispose() }
}
# Check every ancestor for a reparse point before creating or reading product state.
foreach ($path in @($dataRoot, $product)) {
    $ancestor = [IO.DirectoryInfo]::new($path)
    while ($null -ne $ancestor) {
        if ($ancestor.Exists -and ($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Reparse ancestor rejected.' }
        $ancestor = $ancestor.Parent
    }
}
New-ProtectedDirectory $dataRoot
New-ProtectedDirectory $root
$stage = Join-Path (Split-Path $product) ('RSS MDM Agent.stage-' + [Guid]::NewGuid())
$stagedService = Join-Path $root ('service-' + [Guid]::NewGuid() + '.new')
$stagedProbe = Join-Path $root ('probe-' + [Guid]::NewGuid() + '.new')
$createdRegistration = $false; $published = $false; $stopped = $false
try {
    New-ProtectedDirectory $stage
    foreach ($item in Get-ChildItem -Force -LiteralPath $DesktopDirectory) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse source rejected.' }
        Copy-Item -LiteralPath $item.FullName -Destination $stage -Recurse
    }
    Copy-Locked $ServiceExecutable $stagedService
    Copy-Locked $ProbeExecutable $stagedProbe
    Assert-ProtectedTree $stage
    Assert-ProtectedTree $root
    if ((Get-FileHash (Join-Path $stage 'rss-mdm-desktop.exe') -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.executableSha256) { throw 'Desktop changed during staging.' }
    if (Get-Service $name -ErrorAction SilentlyContinue) {
        Stop-Service $name
        (Get-Service $name).WaitForStatus('Stopped', [TimeSpan]::FromSeconds(10))
    }
    $stopped = $true
    if (-not (Get-Service $name -ErrorAction SilentlyContinue)) {
        & "$env:SystemRoot\System32\sc.exe" create $name binPath= ([char]34 + $service + [char]34) start= auto obj= "NT SERVICE\$name"
        if ($LASTEXITCODE -ne 0) { throw 'SCM registration failed.' }
        $createdRegistration = $true
    }
    & "$env:SystemRoot\System32\sc.exe" sidtype $name restricted
    if ($LASTEXITCODE -ne 0) { throw 'Service SID setup failed.' }
    $serviceSid = ([Security.Principal.NTAccount]::new("NT SERVICE\$name")).Translate([Security.Principal.SecurityIdentifier]).Value
    $readers = @($serviceSid) + $AllowedUserSid
    Set-Acl $stage (Restricted-Acl $readers)
    Set-Acl $dataRoot (Restricted-Acl $readers)
    foreach ($path in @($stage, $dataRoot)) {
        & "$env:SystemRoot\System32\icacls.exe" "$path\*" /reset /T /Q | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Inherited read ACL setup failed.' }
        & "$env:SystemRoot\System32\icacls.exe" $path /setowner '*S-1-5-32-544' /T /Q | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Owner setup failed.' }
        Assert-ProtectedTree $path
    }
    $policyPath = Join-Path $root 'policy.json'
    $installation = [Guid]::NewGuid().ToString()
    if (Test-Path $policyPath) { $installation = (Get-Content $policyPath -Raw | ConvertFrom-Json).installation }
    $serviceHash = (Get-FileHash $stagedService -Algorithm SHA256).Hash.ToLowerInvariant()
    $probeHash = (Get-FileHash $stagedProbe -Algorithm SHA256).Hash.ToLowerInvariant()
    $policy = @{version=1;sourceSha=$manifest.sourceSha;installation=$installation;build=$serviceHash;platform='windows-x64';service_subject=$serviceSid;allowed_users=$AllowedUserSid;
        client=@{path=$desktop;sha256=$manifest.executableSha256;cdhash=$null};probe=@{path=$probe;sha256=$probeHash;cdhash=$null};service=@{path=$service;sha256=$serviceHash;cdhash=$null}}
    $temporary = Join-Path $root 'policy.new'
    [IO.File]::WriteAllText($temporary, ($policy | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
    [IO.Directory]::Move($stage, $product); $published = $true
    Move-Item -LiteralPath $stagedService -Destination $service -Force
    Move-Item -LiteralPath $stagedProbe -Destination $probe -Force
    Move-Item -LiteralPath $temporary -Destination $policyPath -Force
    Start-Service $name
    (Get-Service $name).WaitForStatus('Running', [TimeSpan]::FromSeconds(10))
    Write-Output 'Listener started. Run ordinary-user platform acceptance; this is not a completed security receipt.'
} catch {
    if ($stopped -and (Get-Service $name -ErrorAction SilentlyContinue)) { Stop-Service $name -ErrorAction SilentlyContinue }
    if ($createdRegistration) { & "$env:SystemRoot\System32\sc.exe" delete $name | Out-Null }
    if ($published) { Remove-Item -LiteralPath $product -Recurse -Force }
    # Do not restore an old authorization policy or restart the previous service.
    throw
} finally {
    if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
    if (Test-Path -LiteralPath $stagedService) { Remove-Item -LiteralPath $stagedService -Force }
    if (Test-Path -LiteralPath $stagedProbe) { Remove-Item -LiteralPath $stagedProbe -Force }
}
