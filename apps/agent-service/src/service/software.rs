//! Software facts and local intents are owned by the same authenticated system service.
use super::*;
use execution_contract::*;
use execution_ipc::host::{ClientOrigin, Reply};
use serde_json::Value;

fn text<'a>(row: &'a Value, key: &str) -> Option<&'a str> {
    row.get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
}
impl Core {
    pub(super) fn read_software(
        &mut self,
        command: &Command,
        context: &NativeContextRef,
        origin: &ClientOrigin,
        input: &SoftwareRead,
    ) -> Result<Reply, Error> {
        self.data_context(command, context)?;
        if matches!(origin, ClientOrigin::Ai {context: ai, ..} if ai != context) {
            return Err(Error::Identity);
        }
        let binding = command.binding.as_ref().ok_or(Error::Denied)?;
        self.data
            .authorize_read(&input.read, &command.subject, binding)?;
        let (data, snapshot) = self.data.read_page(
            &input.read,
            &command.subject,
            binding,
            self.host.clock.millis()? / 1000,
        )?;
        let descriptor = self
            .local_directory(command)?
            .into_iter()
            .find(|d| d.reference == data.query)
            .ok_or(Error::Unsupported)?;
        if !descriptor.domains.iter().any(|d| {
            matches!(
                d.as_str(),
                "software.installations" | "software.receipts" | "software.packages"
            )
        }) {
            return Err(Error::Unsupported);
        }
        let offset = input.read.after.as_ref().map_or(0, |c| c.offset);
        let mut items = Vec::new();
        for (index, row) in data.rows.iter().enumerate() {
            if let Some(item) = software_item(
                row,
                &data.query,
                &snapshot,
                offset + index as u32,
                &command.subject,
            )? {
                items.push(item);
            }
        }
        Ok(Reply::SoftwarePage {
            value: SoftwarePage {
                query: data.query,
                collected_at: data.collected_at,
                quality: data.quality,
                enumeration_complete: data.enumeration_complete,
                stale: data.stale,
                items,
                next: data.next,
            },
        })
    }
}

pub(super) fn software_item(
    row: &Value,
    query: &QueryRef,
    snapshot: &Digest,
    offset: u32,
    subject: &str,
) -> Result<Option<InstalledSoftware>, Error> {
    let scope = match (text(row, "scope"), text(row, "owner")) {
        (Some("device"), None) => DataScope::Device,
        (Some("user"), Some(owner)) if owner == subject => DataScope::User {
            subject: plan::id(subject)?,
        },
        (Some("user"), _) => return Ok(None),
        _ => return Err(Error::Untrusted),
    };
    let origin = text(row, "origin").ok_or(Error::Untrusted)?;
    let kind = match origin {
        "application" => InstalledSoftwareKind::Application,
        "pkg_receipt" => InstalledSoftwareKind::Pkg,
        "homebrew" => InstalledSoftwareKind::Homebrew,
        value if value.starts_with("win32.uninstall.") => InstalledSoftwareKind::WindowsProgram,
        value if value.starts_with("msix") => InstalledSoftwareKind::Msix,
        _ => return Err(Error::Unsupported),
    };
    let native_id = text(row, "native_id").ok_or(Error::Untrusted)?.to_owned();
    let reference = SoftwareRef {
        query: query.clone(),
        cursor: DataCursor {
            snapshot: snapshot.clone(),
            offset,
        },
        installation: plan::id(text(row, "id").ok_or(Error::Untrusted)?)?,
    };
    Ok(Some(InstalledSoftware {
        reference,
        kind,
        name: text(row, "name")
            .unwrap_or(&native_id)
            .chars()
            .take(256)
            .collect(),
        version: text(row, "version").map(str::to_owned),
        native_id,
        origin: origin.into(),
        scope,
        publisher: text(row, "publisher")
            .or_else(|| text(row, "team_id"))
            .map(str::to_owned),
        actions: Vec::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn query() -> QueryRef {
        QueryRef {
            id: plan::id("native:software").unwrap(),
            revision: Digest::new("11".repeat(32)).unwrap(),
        }
    }
    #[test]
    fn device_inventory_does_not_expose_other_users_installations() {
        let row = json!({"id":"installation","native_id":"package","name":"App","version":"1","scope":"user","owner":"other-user","origin":"msix.registered"});
        assert!(software_item(
            &row,
            &query(),
            &Digest::new("22".repeat(32)).unwrap(),
            0,
            "current-user"
        )
        .unwrap()
        .is_none());
    }
    #[test]
    fn application_reference_freezes_the_snapshot_position_without_accepting_paths_as_input() {
        let row = json!({"id":"installation","native_id":"com.example.app","name":"App","version":"1","scope":"device","owner":"","origin":"application","location":"/Applications/App.app"});
        let snapshot = Digest::new("22".repeat(32)).unwrap();
        let item = software_item(&row, &query(), &snapshot, 129, "current-user")
            .unwrap()
            .unwrap();
        assert_eq!(item.reference.cursor.snapshot, snapshot);
        assert_eq!(item.reference.cursor.offset, 129);
        assert_eq!(item.kind, InstalledSoftwareKind::Application);
        assert_eq!(item.scope, DataScope::Device);
        assert!(serde_json::to_string(&item)
            .unwrap()
            .find("/Applications/")
            .is_none());
    }
    #[test]
    fn a_receipt_is_not_a_gui_application() {
        let row = json!({"id":"receipt","native_id":"com.example.pkg","name":"Package","version":"1","scope":"device","owner":"","origin":"pkg_receipt"});
        let item = software_item(
            &row,
            &query(),
            &Digest::new("22".repeat(32)).unwrap(),
            0,
            "user",
        )
        .unwrap()
        .unwrap();
        assert_eq!(item.kind, InstalledSoftwareKind::Pkg);
    }
}
