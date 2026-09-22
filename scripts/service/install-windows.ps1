param(
    [Parameter(Mandatory=$true)][string]$DesktopDirectory,
    [Parameter(Mandatory=$true)][string]$ServiceExecutable,
    [Parameter(Mandatory=$true)][string[]]$AllowedUserSid
)
$ErrorActionPreference = "Stop"
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if (-not ([Security.Principal.WindowsPrincipal]$identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Run the laboratory installer as administrator."
}
if ([Environment]::OSVersion.Version.Build -lt 22000 -or -not [Environment]::Is64BitOperatingSystem) {
    throw "Windows 11 x64 is required."
}
$name = "RssMdmAgentStatus"
$product = Join-Path ([Environment]::GetFolderPath("ProgramFiles")) "RSS MDM Agent"
$root = Join-Path ([Environment]::GetFolderPath("CommonApplicationData")) "RSS MDM Agent\service"
foreach ($sid in $AllowedUserSid) { $null = [Security.Principal.SecurityIdentifier]::new($sid) }
if (Get-Service $name -ErrorAction SilentlyContinue) {
    Stop-Service $name
    (Get-Service $name).WaitForStatus("Stopped", [TimeSpan]::FromSeconds(10))
}
if (Test-Path $product) { throw "Remove the previous laboratory application explicitly before reinstalling." }
New-Item -ItemType Directory -Force $product, $root | Out-Null
Copy-Item (Join-Path $DesktopDirectory "*") $product -Recurse
$service = Join-Path $root "rss-local-service.exe"
Copy-Item $ServiceExecutable $service -Force
$desktop = Join-Path $product "rss-mdm-desktop.exe"
if (-not (Test-Path $desktop)) { throw "Complete desktop artifact is required." }
if (-not (Get-Service $name -ErrorAction SilentlyContinue)) {
    & "$env:SystemRoot\System32\sc.exe" create $name binPath= "`"$service`"" start= auto obj= "NT SERVICE\$name"
    if ($LASTEXITCODE -ne 0) { throw "SCM registration failed." }
}
& "$env:SystemRoot\System32\sc.exe" sidtype $name restricted
if ($LASTEXITCODE -ne 0) { throw "Service SID setup failed." }
$serviceSid = ([Security.Principal.NTAccount]::new("NT SERVICE\$name")).Translate([Security.Principal.SecurityIdentifier]).Value
function Protect-Tree([string]$path) {
    $acl = [Security.AccessControl.DirectorySecurity]::new()
    $acl.SetAccessRuleProtection($true, $false)
    $acl.SetOwner([Security.Principal.SecurityIdentifier]::new("S-1-5-32-544"))
    foreach ($sid in @("S-1-5-18", "S-1-5-32-544")) {
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($sid), "FullControl", "ContainerInherit,ObjectInherit", "None", "Allow"))
    }
    foreach ($sid in @($serviceSid) + $AllowedUserSid) {
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($sid), "ReadAndExecute", "ContainerInherit,ObjectInherit", "None", "Allow"))
    }
    Set-Acl $path $acl
    & "$env:SystemRoot\System32\icacls.exe" "$path\*" /reset /T /Q | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Protected installation ACL failed." }
    & "$env:SystemRoot\System32\icacls.exe" $path /setowner "*S-1-5-32-544" /T /Q | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Protected installation owner failed." }
}
$policyPath = Join-Path $root "policy.json"
$installation = [Guid]::NewGuid().ToString()
if (Test-Path $policyPath) { $installation = (Get-Content $policyPath -Raw | ConvertFrom-Json).installation }
function Artifact([string]$path) { return @{path=$path;sha256=(Get-FileHash $path -Algorithm SHA256).Hash.ToLowerInvariant();cdhash=$null} }
$policy = @{version=1;installation=$installation;build=(Get-FileHash $service -Algorithm SHA256).Hash.ToLowerInvariant();
    platform="windows-x64";service_subject=$serviceSid;allowed_users=$AllowedUserSid;client=(Artifact $desktop);service=(Artifact $service)}
$temporary = Join-Path $root "policy.new"
[IO.File]::WriteAllText($temporary, ($policy | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
Protect-Tree $product
Protect-Tree (Split-Path $root)
Move-Item $temporary $policyPath -Force
Start-Service $name
Write-Output "Installed laboratory status service; platform acceptance remains required."
