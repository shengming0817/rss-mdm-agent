// ref: wixtoolset/wix WixToolset.Sdk v4 schema; Microsoft desktop6:Service manifest.
import { createHash } from "node:crypto";
const xml = (value) =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
const key = (path) =>
  `f${createHash("sha256").update(path).digest("hex").slice(0, 24)}`;
export function msiSource({ version, target, id, payload, files }) {
  if (!/^\d+\.\d+\.\d+$/.test(version))
    throw Error("MSI requires a three-part product version");
  const service = key("rss-execution-service.exe");
  const directories = new Map([["", "INSTALLROOT"]]);
  for (const file of files) {
    const parts = file.split("/").slice(0, -1);
    for (let n = 1; n <= parts.length; n++) {
      const path = parts.slice(0, n).join("/");
      directories.set(path, key(path));
    }
  }
  const tree = (parent) =>
    [...directories]
      .filter(
        ([path]) => path && path.split("/").slice(0, -1).join("/") === parent,
      )
      .map(
        ([path, name]) =>
          `<Directory Id="${name}" Name="${xml(path.split("/").at(-1))}">${tree(path)}</Directory>`,
      )
      .join("");
  const components = files
    .map(
      (file) =>
        `<Component Id="c${key(file)}" Directory="${directories.get(file.split("/").slice(0, -1).join("/"))}" Guid="*"><File Id="${key(file)}" Source="${xml(`${payload}/${file}`)}" KeyPath="yes" /></Component>`,
    )
    .join("");
  return `<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs"><Package Name="RSS MDM Agent" Manufacturer="RSS" Version="${xml(version)}" UpgradeCode="${target === "windows-arm64" ? "E7A59295-D81E-4C06-99F4-B74A3B73C3DA" : "85C84C72-1836-42C3-8E51-9D14302B98AD"}" Scope="perMachine">
<MajorUpgrade DowngradeErrorMessage="A newer RSS MDM Agent is installed." Schedule="afterInstallInitialize"/><MediaTemplate EmbedCab="yes"/>
<StandardDirectory Id="ProgramFiles64Folder"><Directory Id="PRODUCTROOT" Name="RSS MDM Agent"><Directory Id="VERSIONS" Name="versions"><Directory Id="INSTALLROOT" Name="${xml(id)}">${tree("")}</Directory></Directory></Directory></StandardDirectory>
${components}<Feature Id="Agent" Level="1">${files.map((file) => `<ComponentRef Id="c${key(file)}"/>`).join("")}</Feature>
<CustomAction Id="ApplyAgent" FileRef="${service}" ExeCommand="--installation apply --candidate &quot;[INSTALLROOT]candidate.json&quot;" Execute="deferred" Impersonate="no" Return="check"/>
<CustomAction Id="RemoveAgent" FileRef="${service}" ExeCommand="--installation remove --binary &quot;[#${service}]&quot;" Execute="deferred" Impersonate="no" Return="check"/>
<InstallExecuteSequence><Custom Action="RemoveAgent" Before="RemoveFiles" Condition="REMOVE=&quot;ALL&quot;"/><Custom Action="ApplyAgent" After="InstallFiles" Condition="NOT REMOVE=&quot;ALL&quot;"/></InstallExecuteSequence>
</Package></Wix>\n`;
}
export function msixManifest({ version, architecture, publisher }) {
  if (
    !/^\d+\.\d+\.\d+$/.test(version) ||
    !["x64", "arm64"].includes(architecture) ||
    !publisher
  )
    throw Error("Invalid MSIX identity");
  return `<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" xmlns:desktop6="http://schemas.microsoft.com/appx/manifest/desktop/windows10/6" xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities" IgnorableNamespaces="uap desktop6 rescap">
<Identity Name="RSS.MDM.Agent" Publisher="${xml(publisher)}" Version="${xml(version)}.0" ProcessorArchitecture="${architecture}"/>
<Properties><DisplayName>RSS MDM Agent</DisplayName><PublisherDisplayName>RSS</PublisherDisplayName><Logo>Assets\\StoreLogo.png</Logo></Properties>
<Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0"/></Dependencies><Resources><Resource Language="zh-CN"/></Resources>
<Applications><Application Id="Agent" Executable="rss-mdm-desktop.exe" EntryPoint="Windows.FullTrustApplication"><uap:VisualElements DisplayName="RSS MDM Agent" Description="RSS MDM Agent" BackgroundColor="transparent" Square150x150Logo="Assets\\Logo.png" Square44x44Logo="Assets\\SmallLogo.png"/>
<Extensions><desktop6:Extension Category="windows.service" Executable="rss-execution-service.exe" EntryPoint="Windows.FullTrustApplication"><desktop6:Service Name="RssExecution" StartupType="auto" StartAccount="localSystem" Arguments="--packaged-service"/></desktop6:Extension></Extensions>
</Application></Applications><Capabilities><rescap:Capability Name="runFullTrust"/><rescap:Capability Name="packagedServices"/><rescap:Capability Name="localSystemServices"/></Capabilities></Package>\n`;
}
