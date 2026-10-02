//! Bounded inspection of selected MSIX bytes; Windows still owns native signature trust.
use super::*;
use std::io::{Read, Seek};
use wire::{SoftwareTaskMsixArchitecture as Architecture, SoftwareTaskMsixIdentity as Identity};

pub(super) fn manifest<R: Read + Seek>(
    reader: R,
    expected: &Identity,
    dependencies: &[Identity],
) -> Result<(), Error> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|_| Error::Untrusted)?;
    if archive.len() > 65536 {
        return Err(Error::Capacity);
    }
    // A duplicate manifest cannot be selected by archive ordering.
    if archive
        .file_names()
        .filter(|p| *p == "AppxManifest.xml")
        .count()
        != 1
    {
        return Err(Error::Untrusted);
    }
    let entry = archive
        .by_name("AppxManifest.xml")
        .map_err(|_| Error::Untrusted)?;
    if entry.size() > 1024 * 1024 || entry.is_symlink() {
        return Err(Error::Untrusted);
    }
    let mut bytes = Vec::new();
    entry.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    check_manifest(&bytes, expected, dependencies)
}
fn check_manifest(
    bytes: &[u8],
    expected: &Identity,
    dependencies: &[Identity],
) -> Result<(), Error> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_reader(bytes);
    let mut depth = 0usize;
    let mut identity = None;
    let mut root_seen = false;
    let mut declared = std::collections::BTreeSet::new();
    loop {
        let event = reader.read_event().map_err(|_| Error::Untrusted)?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let name = element.local_name();
                if depth == 0 {
                    if root_seen || name.as_ref() != "Package" {
                        return Err(Error::Untrusted);
                    }
                    root_seen = true;
                }
                let attributes = element
                    .attributes()
                    .map(|a| {
                        let a = a.map_err(|_| Error::Untrusted)?;
                        let key = a.key.as_ref().to_owned();
                        let value = a
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .map_err(|_| Error::Untrusted)?
                            .into_owned();
                        Ok((key, value))
                    })
                    .collect::<Result<BTreeMap<_, _>, Error>>()?;
                if depth == 1 && name.as_ref() == "Identity" {
                    if identity.is_some() {
                        return Err(Error::Untrusted);
                    }
                    let field = |key| attributes.get(key).cloned().ok_or(Error::Untrusted);
                    identity = Some(Identity {
                        name: field("Name")?,
                        publisher: field("Publisher")?,
                        version: version(&field("Version")?)?,
                        architecture: match field("ProcessorArchitecture")?.as_str() {
                            "x64" => Architecture::X86_64,
                            "arm64" => Architecture::Aarch64,
                            "neutral" => Architecture::Neutral,
                            _ => return Err(Error::Untrusted),
                        },
                        resource_id: attributes.get("ResourceId").cloned().unwrap_or_default(),
                    });
                }
                if name.as_ref() == "PackageDependency" {
                    let name = attributes.get("Name").ok_or(Error::Untrusted)?;
                    let publisher = attributes.get("Publisher").ok_or(Error::Untrusted)?;
                    let min = version(attributes.get("MinVersion").ok_or(Error::Untrusted)?)?;
                    let candidates = dependencies
                        .iter()
                        .filter(|d| {
                            &d.name == name && &d.publisher == publisher && d.version >= min
                        })
                        .count();
                    if candidates != 1 || !declared.insert((name.clone(), publisher.clone())) {
                        return Err(Error::Untrusted);
                    }
                }
                if !empty {
                    depth += 1;
                    if depth > 48 {
                        return Err(Error::Capacity);
                    }
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).ok_or(Error::Untrusted)?;
            }
            Event::DocType(_) | Event::GeneralRef(_) => return Err(Error::Untrusted),
            Event::Eof => break,
            _ => (),
        }
    }
    if identity.as_ref() != Some(expected) || depth != 0 {
        return Err(Error::Untrusted);
    }
    Ok(())
}
pub(super) fn version(value: &str) -> Result<[u16; 4], Error> {
    let values = value
        .split('.')
        .map(|part| {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Error::Untrusted);
            }
            part.parse::<u16>().map_err(|_| Error::Untrusted)
        })
        .collect::<Result<Vec<_>, _>>()?;
    values.try_into().map_err(|_| Error::Untrusted)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> Identity {
        Identity {
            name: "org.rss.proof".into(),
            publisher: "CN=RSS".into(),
            version: [1, 2, 3, 4],
            architecture: Architecture::Aarch64,
            resource_id: String::new(),
        }
    }
    #[test]
    fn manifest_requires_exact_native_identity_and_declared_dependencies() {
        let xml = br#"<Package><Identity Name="org.rss.proof" Publisher="CN=RSS" Version="1.2.3.4" ProcessorArchitecture="arm64"/><Dependencies/></Package>"#;
        assert!(check_manifest(xml, &identity(), &[]).is_ok());
        for (from, to) in [
            ("CN=RSS", "CN=Other"),
            ("1.2.3.4", "1.2.3.5"),
            ("arm64", "neutral"),
        ] {
            assert!(check_manifest(
                String::from_utf8_lossy(xml).replace(from, to).as_bytes(),
                &identity(),
                &[]
            )
            .is_err());
        }
        let dependency = String::from_utf8_lossy(xml).replace("<Dependencies/>", "<Dependencies><PackageDependency Name=\"runtime\" Publisher=\"CN=Runtime\" MinVersion=\"1.0.0.0\"/></Dependencies>");
        assert!(check_manifest(dependency.as_bytes(), &identity(), &[]).is_err());
        let expected = Identity {
            name: "runtime".into(),
            publisher: "CN=Runtime".into(),
            version: [1, 0, 0, 0],
            architecture: Architecture::Aarch64,
            resource_id: String::new(),
        };
        assert!(check_manifest(dependency.as_bytes(), &identity(), &[expected]).is_ok());
        assert!(version("1.2.3").is_err());
        assert!(version("1.2.3.65536").is_err());
    }
}

#[cfg(test)]
#[test]
fn selected_package_manifest_is_inspected_through_the_archive_entry() {
    use std::io::Write;
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive
        .start_file("AppxManifest.xml", zip::write::SimpleFileOptions::default())
        .unwrap();
    archive.write_all(br#"<Package><Identity Name="proof" Publisher="CN=Proof" Version="1.0.0.0" ProcessorArchitecture="neutral"/></Package>"#).unwrap();
    let bytes = archive.finish().unwrap().into_inner();
    let expected = Identity {
        name: "proof".into(),
        publisher: "CN=Proof".into(),
        version: [1, 0, 0, 0],
        architecture: Architecture::Neutral,
        resource_id: String::new(),
    };
    assert!(manifest(std::io::Cursor::new(&bytes), &expected, &[]).is_ok());
    let mut wrong = expected;
    wrong.resource_id = "language-en".into();
    assert!(manifest(std::io::Cursor::new(bytes), &wrong, &[]).is_err());
}
