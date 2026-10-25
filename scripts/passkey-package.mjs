// ref: Microsoft external-location package manifest; official PasskeyManager COM extension.
export const component = "RSS.PersonalPasskeys";
export const clsid = "f1fd3a7c-27c1-4f88-bf89-6ac6525c2929";
export const sdk = "10.0.26100.7175";
export const unsignedPublisher =
  "CN=RSS Development, OID.2.25.311729368913984317654407730594956997722=1";
const xml = (s) =>
  String(s)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
export function identityManifest({ version, publisher }) {
  if (
    !/^\d+\.\d+\.\d+\.\d+$/.test(version) ||
    version.split(".").some((n) => Number(n) > 65535) ||
    !publisher ||
    /[\r\n]/.test(publisher)
  )
    throw Error("Invalid Passkey package identity");
  return `<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10" xmlns:com="http://schemas.microsoft.com/appx/manifest/com/windows10" xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities" IgnorableNamespaces="uap uap10 com rescap">
<Identity Name="${component}" Publisher="${xml(publisher)}" Version="${version}" ProcessorArchitecture="x64"/>
<Properties><DisplayName>RSS Personal Passkeys</DisplayName><PublisherDisplayName>RSS</PublisherDisplayName><Logo>Assets\\StoreLogo.png</Logo><uap10:AllowExternalContent>true</uap10:AllowExternalContent></Properties>
<Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.26100.7171" MaxVersionTested="10.0.26100.7171"/></Dependencies><Resources><Resource Language="zh-CN"/></Resources>
<Applications><Application Id="Provider" Executable="rss-passkey-provider.exe" uap10:TrustLevel="mediumIL" uap10:RuntimeBehavior="win32App"><uap:VisualElements DisplayName="RSS Personal Passkeys" Description="个人 Passkey Provider" BackgroundColor="transparent" Square150x150Logo="Assets\\Logo.png" Square44x44Logo="Assets\\SmallLogo.png" AppListEntry="none"/>
<Extensions><com:Extension Category="windows.comServer"><com:ComServer><com:ExeServer Executable="rss-passkey-provider.exe" Arguments="-Embedding" DisplayName="RSS Personal Passkeys"><com:Class Id="${clsid}" DisplayName="RSS Personal Passkeys"/></com:ExeServer></com:ComServer></com:Extension></Extensions>
</Application></Applications><Capabilities><rescap:Capability Name="runFullTrust"/><rescap:Capability Name="unvirtualizedResources"/></Capabilities></Package>\n`;
}
