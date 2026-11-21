//! Windows Installer's Application log records the opening/closing transaction and the
//! product outcome. The protected ticket binds one installation UUID to its original
//! opening record. A later transaction or a rolled-over log cannot confirm that UUID.
//! ref: Microsoft Windows Installer Event Logging (1033/1034/1035), MsiGetProductInfoExW.
use super::*;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Ticket {
    product: Uuid,
    version: String,
    first_record: u64,
    opening: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Event {
    system: System,
    event_data: Data,
}
#[derive(Deserialize)]
struct System {
    #[serde(rename = "EventID")]
    kind: u32,
    #[serde(rename = "EventRecordID")]
    record: u64,
    #[serde(rename = "TimeCreated")]
    time: Time,
    #[serde(rename = "Provider")]
    provider: Provider,
}
#[derive(Deserialize)]
struct Provider {
    #[serde(rename = "@Name")]
    name: String,
}
#[derive(Deserialize)]
struct Time {
    #[serde(rename = "@SystemTime")]
    value: String,
}
#[derive(Deserialize)]
struct Data {
    #[serde(rename = "Data", default)]
    fields: Vec<String>,
    #[serde(rename = "Binary", default)]
    binary: String,
}
#[derive(Deserialize)]
struct Events {
    #[serde(rename = "Event", default)]
    events: Vec<Event>,
}
impl Event {
    fn transaction(&self) -> Option<(String, u32)> {
        let target = self.event_data.fields.first()?.to_lowercase();
        let pid = self.event_data.fields.get(1)?.parse().ok()?;
        if target.is_empty() || pid == 0 {
            return None;
        }
        Some((target, pid))
    }
    fn opening(&self) -> Option<String> {
        let tx = self.transaction()?;
        Some(hash(
            &serde_json::to_vec(&(self.system.record, &self.system.time.value, tx)).ok()?,
        ))
    }
}
impl Ticket {
    fn capture(product: Uuid, version: &str, package: &Path, event: &Event) -> Result<Self, Error> {
        let (target, _) = event.transaction().ok_or(Error::Unavailable)?;
        if event.system.kind != 1040
            || event.system.provider.name != "MsiInstaller"
            || version.is_empty()
            || version.len() > 64
            || event.system.record == 0
            || event.system.time.value.is_empty()
            || (Uuid::parse_str(&target).ok() != Some(product)
                && package.to_str().is_none_or(|p| p.to_lowercase() != target))
        {
            return Err(Error::Unavailable);
        }
        Ok(Self {
            product,
            version: version.to_owned(),
            first_record: event.system.record,
            opening: event.opening().ok_or(Error::Unavailable)?,
        })
    }

    fn successful(&self, events: &[Event], removing: bool) -> bool {
        let Some((begin, rest)) = events.split_first() else {
            return false;
        };
        if begin.system.provider.name != "MsiInstaller"
            || begin.system.kind != 1040
            || begin.system.record != self.first_record
            || begin.opening().as_ref() != Some(&self.opening)
        {
            return false;
        }
        let Some(root) = begin.transaction() else {
            return false;
        };
        let mut stack = vec![root.clone()];
        let mut outcome = None;
        let mut last_record = self.first_record;
        for e in rest {
            if e.system.provider.name != "MsiInstaller" || e.system.record <= last_record {
                return false;
            }
            last_record = e.system.record;
            match e.system.kind {
                1040 => {
                    if stack.is_empty() {
                        break;
                    }
                    let Some(nested) = e.transaction() else {
                        return false;
                    };
                    if nested == root {
                        return false;
                    }
                    stack.push(nested);
                }
                1042 => {
                    let Some(ending) = e.transaction() else {
                        return false;
                    };
                    let Some(expected) = stack.pop() else {
                        return false;
                    };
                    if expected != ending
                        && !(expected == root
                            && ending.1 == root.1
                            && Uuid::parse_str(&ending.0).ok() == Some(self.product))
                    {
                        return false;
                    }
                }
                1033..=1035 if stack.len() <= 1 => {
                    let fields = &e.event_data.fields;
                    if fields.first().map(String::as_str) != Some("RSS MDM Agent")
                        || fields.get(1) != Some(&self.version)
                        || fields.get(4).map(String::as_str) != Some("RSS")
                        || (e.system.kind == 1034) != removing
                    {
                        continue;
                    }
                    // Native providers that include a ProductCode discriminator must match it.
                    // After the closing bracket require that discriminator for attribution.
                    let discriminator = binary_product(&e.event_data.binary);
                    if discriminator.is_some_and(|id| id != self.product)
                        || (stack.is_empty() && discriminator.is_none())
                    {
                        continue;
                    }
                    if outcome.is_some() {
                        return false;
                    }
                    outcome = Some(fields.get(3).map(String::as_str) == Some("0"));
                }
                _ => (),
            }
            if stack.is_empty() && outcome.is_some() {
                break;
            }
        }
        stack.is_empty() && outcome == Some(true)
    }
}
fn binary_product(binary: &str) -> Option<Uuid> {
    if !binary.is_ascii() || !binary.len().is_multiple_of(2) || binary.len() > 160 {
        return None;
    }
    let bytes = (0..binary.len())
        .step_by(2)
        .map(|n| u8::from_str_radix(&binary[n..n + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if bytes.len() == 16 {
        return Some(Uuid::from_bytes_le(bytes.try_into().ok()?));
    }
    let ascii = if bytes.get(1) == Some(&0) {
        if !bytes.len().is_multiple_of(2) || bytes.chunks_exact(2).any(|pair| pair[1] != 0) {
            return None;
        }
        bytes.chunks_exact(2).map(|pair| pair[0]).collect()
    } else {
        bytes
    };
    Uuid::parse_str(
        std::str::from_utf8(&ascii)
            .ok()?
            .trim_end_matches(char::from(0)),
    )
    .ok()
}

fn parse(xml: &str) -> Result<Events, Error> {
    quick_xml::de::from_str(xml).map_err(|_| Error::Unavailable)
}

#[cfg(windows)]
fn query(predicate: &str, newest: bool) -> Result<Events, Error> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut wide = vec![0u16; 32768];
    let used = unsafe { GetSystemDirectoryW(wide.as_mut_ptr(), wide.len() as u32) } as usize;
    if used == 0 || used >= wide.len() {
        return Err(Error::Unavailable);
    }
    let tool = PathBuf::from(OsString::from_wide(&wide[..used])).join("wevtutil.exe");
    let filter = format!("/q:*[System[Provider[@Name='MsiInstaller'] and ({predicate})]]");
    let data = command(
        &tool,
        &[
            "qe".into(),
            "Application".into(),
            filter.into(),
            "/f:xml".into(),
            "/e:Events".into(),
            "/uni:true".into(),
            if newest { "/rd:true" } else { "/rd:false" }.into(),
            if newest { "/c:1" } else { "/c:128" }.into(),
        ],
    )?;
    if !data.len().is_multiple_of(2) {
        return Err(Error::Unavailable);
    }
    let wide = data
        .chunks_exact(2)
        .map(|v| u16::from_le_bytes([v[0], v[1]]))
        .collect::<Vec<_>>();
    let xml = String::from_utf16(&wide).map_err(|_| Error::Unavailable)?;
    parse(xml.trim_start_matches('\u{feff}'))
}
#[cfg(windows)]
impl Ticket {
    pub(super) fn at_save(product: Uuid, version: &str, package: &Path) -> Result<Self, Error> {
        // A prior already-closed transaction with the same ProductCode cannot seed a repair.
        let events = query(
            "EventID=1040 or EventID=1042 or EventID=1033 or EventID=1034 or EventID=1035",
            true,
        )?;
        let event = events.events.first().ok_or(Error::Unavailable)?;
        Self::capture(product, version, package, event)
    }
    pub(super) fn confirmed(&self, removing: bool) -> Result<bool, Error> {
        let predicate = format!("EventRecordID >= {} and (EventID=1040 or EventID=1042 or EventID=1033 or EventID=1034 or EventID=1035)", self.first_record);
        if !self.successful(&query(&predicate, false)?.events, removing) {
            return Ok(false);
        }
        let version = crate::local_software::native::machine_msi_version(self.product)?;
        Ok(if removing {
            version.is_none()
        } else {
            version.as_deref() == Some(self.version.as_str())
        })
    }
}

#[cfg(all(test, not(windows)))]
impl Ticket {
    pub(super) fn confirmed(&self, _: bool) -> Result<bool, Error> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    const PRODUCT: Uuid = Uuid::from_u128(37);
    fn event(id: u32, record: u64, fields: &[&str]) -> String {
        let data = fields
            .iter()
            .map(|value| format!("<Data>{value}</Data>"))
            .collect::<String>();
        format!("<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'><System><Provider Name='MsiInstaller'/><EventID>{id}</EventID><TimeCreated SystemTime='2026-01-01T00:00:00Z'/><EventRecordID>{record}</EventRecordID></System><EventData>{data}</EventData></Event>")
    }
    pub(in crate::installation) fn ticket() -> Ticket {
        let open = event(1040, 7, &["fixed.msi", "2300"]);
        let events = parse(&format!("<Events>{open}</Events>")).unwrap();
        Ticket::capture(PRODUCT, "1.2.3", Path::new("fixed.msi"), &events.events[0]).unwrap()
    }
    pub(in crate::installation) fn observed(
        ticket: &Ticket,
        removing: bool,
        complete: bool,
    ) -> Result<bool, Error> {
        let start = event(1040, 7, &["fixed.msi", "2300"]);
        let success = event(
            if removing { 1034 } else { 1033 },
            9,
            &["RSS MDM Agent", "1.2.3", "1033", "0", "RSS"],
        );
        let end = if complete {
            event(1042, 10, &["fixed.msi", "2300"])
        } else {
            String::new()
        };
        Ok(ticket.successful(
            &parse(&format!("<Events>{start}{success}{end}</Events>"))?.events,
            removing,
        ))
    }
    #[test]
    fn capture_rejects_a_previous_completed_run_and_a_different_package() {
        let closed = event(1042, 10, &["fixed.msi", "2300"]);
        let opened = event(1040, 11, &["other.msi", "2300"]);
        for xml in [closed, opened] {
            let events = parse(&format!("<Events>{xml}</Events>")).unwrap();
            assert!(
                Ticket::capture(PRODUCT, "1.2.3", Path::new("fixed.msi"), &events.events[0])
                    .is_err()
            );
        }
        let value = PRODUCT.braced().to_string();
        let hex = value
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .map(|b| format!("{b:02X}"))
            .collect::<String>();
        assert_eq!(binary_product(&hex), Some(PRODUCT));
    }
    #[test]
    fn only_the_original_closed_successful_native_transaction_can_settle() {
        let ticket = ticket();
        let opening = event(1040, 7, &["fixed.msi", "2300"]);
        let success = event(1033, 9, &["RSS MDM Agent", "1.2.3", "1033", "0", "RSS"]);
        let failed = event(1033, 9, &["RSS MDM Agent", "1.2.3", "1033", "1603", "RSS"]);
        let end = event(1042, 10, &["fixed.msi", "2300"]);
        let later = event(1040, 8, &["fixed.msi", "2300"]);
        for (sequence, expected) in [
            (vec![&opening, &success, &end], true),
            (vec![&opening, &success], false),
            (vec![&opening, &failed, &end], false),
            (vec![&opening, &end], false),
            (vec![&opening, &later, &success, &end], false),
            (vec![&success, &end], false),
        ] {
            let events = parse(&format!(
                "<Events>{}</Events>",
                sequence.into_iter().map(String::as_str).collect::<String>()
            ))
            .unwrap();
            assert_eq!(ticket.successful(&events.events, false), expected);
        }
        let removed = event(1034, 9, &["RSS MDM Agent", "1.2.3", "1033", "0", "RSS"]);
        let events = parse(&format!("<Events>{opening}{removed}{end}</Events>")).unwrap();
        assert!(ticket.successful(&events.events, true));
        assert!(!ticket.successful(&events.events, false));
    }
}
