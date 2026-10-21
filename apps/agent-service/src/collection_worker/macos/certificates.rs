use super::*;
use der::Decode;
pub(super) fn collect(
    c: &Context<'_>,
    store: w::MacosCertificateStore,
    rows: &mut Rows<'_>,
) -> Result<()> {
    let owner = match store {
        w::MacosCertificateStore::Login => c
            .input
            .read
            .execution
            .generated_uid
            .as_deref()
            .ok_or(Status::Denied)?,
        _ => "",
    };
    let path = match store {
        w::MacosCertificateStore::System => PathBuf::from("/Library/Keychains/System.keychain"),
        w::MacosCertificateStore::SystemRoots => {
            PathBuf::from("/System/Library/Keychains/SystemRootCertificates.keychain")
        }
        w::MacosCertificateStore::Login => {
            let w::SoftwareExecutionTarget::User { identity, .. } = &c.input.read.execution.target
            else {
                return Err(Status::Denied.into());
            };
            home(identity.parse().map_err(|_| Status::Denied)?)?
                .join("Library/Keychains/login.keychain-db")
        }
    };
    // Do not accept a substituted keychain at a symlinked final path.
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.is_file() {
        return Err(Status::Denied.into());
    }
    let store = rows.query.definition.clone();
    let w::NativeQueryDefinition::System { query } = store else {
        return Err(Status::Invalid.into());
    };
    let store = query.certificate_store().ok_or(Status::Invalid)?;
    native::certificates(c, &path, rows.query.budget.max_rows.into(), |bytes| {
        rows.push(decode(bytes, store, owner)?)?;
        Ok(())
    })
}
pub(super) fn decode(bytes: &[u8], store: &str, owner: &str) -> Result<Row> {
    let cert = x509_cert::Certificate::from_der(bytes).map_err(|_| Status::Invalid)?;
    let tbs = cert.tbs_certificate;
    let before = i64::try_from(tbs.validity.not_before.to_unix_duration().as_secs())
        .map_err(|_| Status::Invalid)?;
    let after = i64::try_from(tbs.validity.not_after.to_unix_duration().as_secs())
        .map_err(|_| Status::Invalid)?;
    if after < before {
        return Err(Status::Invalid.into());
    }
    Ok(row(&[
        ("id", hex(bytes).into()),
        ("subject", tbs.subject.to_string().into()),
        ("issuer", tbs.issuer.to_string().into()),
        ("not_before", before.into()),
        ("not_after", after.into()),
        ("store", store.into()),
        (
            "scope",
            if owner.is_empty() { "device" } else { "user" }.into(),
        ),
        ("owner", owner.into()),
    ]))
}
