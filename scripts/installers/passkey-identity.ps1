param([Parameter(Mandatory)][ValidateSet('preflight','install','remove')][string]$Action)
$ErrorActionPreference = 'Stop'
$Root = $PSScriptRoot
$Provider = Join-Path $Root 'rss-passkey-provider.exe'
$Identity = Join-Path $Root 'identity.msix'
$Deployment = Get-Content -LiteralPath (Join-Path $Root 'deployment.json') -Raw | ConvertFrom-Json
function Run-Provider([string]$Operation) {
    & $Provider $Operation
    if ($LASTEXITCODE -ne 0) { throw "Provider $Operation failed ($LASTEXITCODE)" }
}
if ((Get-FileHash -LiteralPath $Provider -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Deployment.providerSha256 -or
    (Get-FileHash -LiteralPath $Identity -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Deployment.identitySha256) { throw 'Component digest mismatch' }
if ($Deployment.release) {
    foreach ($File in @($Provider,$Identity)) {
        $Signature = Get-AuthenticodeSignature -LiteralPath $File
        if ($Signature.Status -ne 'Valid' -or $Signature.SignerCertificate.Subject -ne $Deployment.publisher) { throw 'Component signer mismatch' }
    }
}
$Packages = @(Get-AppxPackage -Name 'RSS.PersonalPasskeys')
if ($Packages.Count -gt 1 -or ($Packages.Count -eq 1 -and $Packages[0].Publisher -ne $Deployment.publisher)) { throw 'Another host owns the Passkey package identity' }
if ($Packages.Count -eq 1 -and $Packages[0].Version -gt [version]$Deployment.version) { throw 'Program downgrade refused; Vault data is preserved' }
if ($Action -eq 'preflight') { exit 0 }
if ($Action -eq 'remove') {
    Run-Provider 'stop'
    Run-Provider 'unregister'
    foreach ($Package in $Packages) { Remove-AppxPackage -Package $Package.PackageFullName }
    if (@(Get-AppxPackage -Name 'RSS.PersonalPasskeys').Count -ne 0) { throw 'Identity removal incomplete' }
    [IO.File]::WriteAllText((Join-Path $Root 'platform-removed'), 'RSS.PersonalPasskeys')
    exit 0
}
if ($Packages.Count -eq 1 -and $Packages[0].Version -gt [version]$Deployment.version) { throw 'Program downgrade refused; Vault data is preserved' }
$Arguments = @{ Path=$Identity; ExternalLocation=$Root; ErrorAction='Stop' }
if (-not $Deployment.release) { $Arguments.AllowUnsigned=$true }
if ($Packages.Count -eq 0 -or $Packages[0].Version -lt [version]$Deployment.version) { Add-AppxPackage @Arguments }
Run-Provider 'register'
# Successful repair also completes interrupted installer bookkeeping.
$Uninstall = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RSS.PersonalPasskeys'
$null = New-Item -Path $Uninstall -Force
New-ItemProperty -LiteralPath $Uninstall -Name PlatformRemoved -Value 0 -PropertyType DWord -Force | Out-Null
Set-ItemProperty -LiteralPath $Uninstall -Name DisplayName -Value 'RSS Personal Passkeys'
Set-ItemProperty -LiteralPath $Uninstall -Name UninstallString -Value ('"' + (Join-Path $Root 'uninstall.exe') + '"')
$Previous = $Root + '.previous'
if (Test-Path -LiteralPath $Previous) {
    if ((Get-Item -LiteralPath $Previous).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Previous installation is a reparse point; refusing cleanup' }
    foreach ($Name in @('rss-passkey-provider.exe','identity.msix','identity.ps1','deployment.json','uninstall.exe','Assets\StoreLogo.png','Assets\Logo.png','Assets\SmallLogo.png')) {
        $File = Join-Path $Previous $Name
        if (Test-Path -LiteralPath $File) { Remove-Item -LiteralPath $File }
    }
    $Assets = Join-Path $Previous 'Assets'
    if ((Test-Path -LiteralPath $Assets) -and @(Get-ChildItem -LiteralPath $Assets -Force).Count -eq 0) { Remove-Item -LiteralPath $Assets }
    if (@(Get-ChildItem -LiteralPath $Previous -Force).Count -eq 0) { Remove-Item -LiteralPath $Previous }
}
Run-Provider 'enable' 
