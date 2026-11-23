//! Consume the machine profile inventory from profiles(1), including the native companions.
//! ref: apple/device-management mdm/profiles/com.apple.ManagedClient.preferences.yaml
use agent_client::{da::ManagedSource, Error};
use plist::{Dictionary, Value};
use rss_mdm_agent_wire::{
    da::{Configuration, SignedConfiguration},
    platform_sso::{AuthenticationMode, Endpoints, LoginPolicy},
};
use uuid::Uuid;

fn text<'a>(d: &'a Dictionary, key: &str) -> Result<&'a str, Error> {
    d.get(key).and_then(Value::as_string).ok_or(Error::Protocol)
}
fn fields(d: &Dictionary) -> Result<&Dictionary, Error> {
    d.get("PayloadContent")
        .and_then(Value::as_dictionary)
        .ok_or(Error::Protocol)
}
fn carrier(d: &Dictionary) -> Result<Option<SignedConfiguration>, Error> {
    if text(d, "PayloadType")? != "com.apple.ManagedClient.preferences" {
        return Ok(None);
    }
    let content = fields(d)?;
    let Some(domain) = content.get("com.rss.device-access") else {
        return Ok(None);
    };
    let forced = domain
        .as_dictionary()
        .and_then(|d| d.get("Forced"))
        .and_then(Value::as_array)
        .ok_or(Error::Protocol)?;
    let [entry] = forced.as_slice() else {
        return Err(Error::Protocol);
    };
    let settings = entry
        .as_dictionary()
        .and_then(|d| d.get("mcx_preference_settings"))
        .and_then(Value::as_dictionary)
        .ok_or(Error::Protocol)?;
    if settings.len() != 1 {
        return Err(Error::Untrusted);
    }
    Ok(Some(
        serde_json::from_str(text(settings, "SignedConfiguration")?)
            .map_err(|_| Error::Protocol)?,
    ))
}

pub(super) fn snapshot(document: &Value) -> Result<Vec<ManagedSource>, Error> {
    let root = document.as_dictionary().ok_or(Error::Protocol)?;
    let Some(machine) = root.get("_computerlevel") else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    for profile in machine.as_array().ok_or(Error::Protocol)? {
        let profile = profile.as_dictionary().ok_or(Error::Protocol)?;
        let items = profile
            .get("ProfileItems")
            .and_then(Value::as_array)
            .ok_or(Error::Protocol)?;
        let mut found = None;
        for item in items {
            if let Some(signed) = carrier(item.as_dictionary().ok_or(Error::Protocol)?)? {
                if found.replace(signed).is_some() {
                    return Err(Error::Conflict);
                }
            }
        }
        let Some(envelope) = found else { continue };
        let config = envelope.decode()?;
        if profile
            .get("ProfileWasInstalledByMDM")
            .and_then(Value::as_boolean)
            != Some(true)
        {
            return Err(Error::Untrusted);
        }
        let binds = &config.native.bindings;
        if binds.len() != 4
            || items.len() != 3
            || text(profile, "ProfileIdentifier")? != binds[0].identifier
            || Uuid::parse_str(text(profile, "ProfileUUID")?).ok() != Some(binds[0].uuid)
            || profile
                .get("ProfileVersion")
                .and_then(Value::as_unsigned_integer)
                != Some(u64::from(binds[0].version))
        {
            return Err(Error::Untrusted);
        }
        for (item, binding) in items.iter().zip(&binds[1..]) {
            let d = item.as_dictionary().ok_or(Error::Protocol)?;
            let typ = text(d, "PayloadType")?;
            if binding.schema != format!("mdm/profiles/{typ}.yaml")
                || text(d, "PayloadIdentifier")? != binding.identifier
                || Uuid::parse_str(text(d, "PayloadUUID")?).ok() != Some(binding.uuid)
                || d.get("PayloadVersion").and_then(Value::as_unsigned_integer)
                    != Some(u64::from(binding.version))
            {
                return Err(Error::Untrusted);
            }
        }
        check_native(&config, items)?;
        out.push(ManagedSource { envelope });
        if out.len() > 15 {
            return Err(Error::Capacity);
        }
    }
    Ok(out)
}
fn check_native(c: &Configuration, items: &[Value]) -> Result<(), Error> {
    let mac = c.native.mac.as_ref().ok_or(Error::Untrusted)?;
    let login = c.content.mac.as_ref().ok_or(Error::Untrusted)?;
    let mut platform = Dictionary::new();
    platform.insert(
        "AuthenticationMethod".into(),
        Value::from(match login.authentication {
            AuthenticationMode::SecureEnclave => "UserSecureEnclaveKey",
            AuthenticationMode::Password => "Password",
        }),
    );
    platform.insert("UseSharedDeviceKeys".into(), true.into());
    for (name, policy) in [
        ("LoginPolicy", login.login),
        ("UnlockPolicy", login.unlock),
        ("FileVaultPolicy", login.filevault),
    ] {
        let mut v = match policy {
            LoginPolicy::Local => continue,
            LoginPolicy::Attempt => vec!["AttemptAuthentication".into()],
            LoginPolicy::Require => vec!["RequireAuthentication".into()],
        };
        if policy == LoginPolicy::Require {
            if login.offline_grace_seconds.is_some() {
                v.push("AllowOfflineGracePeriod".into());
            }
            if login.registration_grace_seconds.is_some() {
                v.push("AllowAuthenticationGracePeriod".into());
            }
        }
        platform.insert(name.into(), Value::Array(v));
    }
    for (name, seconds) in [
        ("OfflineGracePeriod", login.offline_grace_seconds),
        (
            "AuthenticationGracePeriod",
            login.registration_grace_seconds,
        ),
    ] {
        if let Some(v) = seconds {
            platform.insert(name.into(), Value::Integer(u64::from(v).into()));
        }
    }
    if !login.maintenance_accounts.is_empty() {
        platform.insert(
            "NonPlatformSSOAccounts".into(),
            Value::Array(
                login
                    .maintenance_accounts
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
    let endpoints = Endpoints::new(&mac.origin, c.tenant)?;
    let mut sso = Dictionary::new();
    sso.insert("Type".into(), "Redirect".into());
    sso.insert(
        "ExtensionIdentifier".into(),
        mac.deployment.extension_id.clone().into(),
    );
    sso.insert(
        "TeamIdentifier".into(),
        mac.deployment.team_id.clone().into(),
    );
    sso.insert(
        "URLs".into(),
        Value::Array(vec![format!("{}/browser/", endpoints.base).into()]),
    );
    sso.insert("PlatformSSO".into(), platform.into());
    let origin = url::Url::parse(&mac.origin).map_err(|_| Error::Protocol)?;
    let mut app = Dictionary::new();
    app.insert(
        "ApplicationIdentifier".into(),
        format!(
            "{}.{}",
            mac.deployment.team_id, mac.deployment.application_id
        )
        .into(),
    );
    app.insert(
        "AssociatedDomains".into(),
        Value::Array(vec![format!(
            "authsrv:{}?mode=managed",
            origin.domain().ok_or(Error::Protocol)?
        )
        .into()]),
    );
    app.insert("EnableDirectDownloads".into(), true.into());
    let mut domains = Dictionary::new();
    domains.insert("Configuration".into(), Value::Array(vec![app.into()]));
    for (item, (kind, expected)) in items.iter().zip([
        ("com.apple.extensiblesso", sso),
        ("com.apple.associated-domains", domains),
    ]) {
        let d = item.as_dictionary().ok_or(Error::Protocol)?;
        if text(d, "PayloadType")? != kind || fields(d)? != &expected {
            return Err(Error::Untrusted);
        }
    }
    Ok(())
}
