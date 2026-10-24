//! Immediate registration control on the existing communication owner, ahead of ordinary tasks.
use super::*;
use crate::credential_worker::state;
use execution_ipc::{
    host::{Reply, Request},
    platform_sso::{Operation, Response},
};
use rss_mdm_agent_wire::platform_sso as w;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use uuid::Uuid;

impl crate::deployment::PlatformSsoConfiguration {
    pub fn verify(&self) -> Result<(), Error> {
        if !cfg!(all(target_os = "macos", target_arch = "aarch64"))
            || self.team_id.len() != 10
            || !self
                .team_id
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Err(Error::Unsupported);
        }
        #[cfg(target_os = "macos")]
        {
            let mut version = [0u8; 32];
            let mut length = version.len();
            // SAFETY: bounded output buffer and valid sysctl name; no owned kernel pointer escapes.
            if unsafe {
                libc::sysctlbyname(
                    c"kern.osproductversion".as_ptr(),
                    version.as_mut_ptr().cast(),
                    &mut length,
                    std::ptr::null_mut(),
                    0,
                )
            } != 0
                || length == 0
                || length > version.len()
                || &version[..length] != b"26.4\0"
            {
                return Err(Error::Unsupported);
            }
        }
        w::Endpoints::new(&self.origin, Uuid::from_u128(1))?;
        self.extension
            .verify(&self.extension.path)
            .map_err(|_| Error::Identity)?;
        #[cfg(target_os = "macos")]
        installation_security::verify_apple_identity(
            &self.extension.path,
            &self.team_id,
            "com.rss.mdmagent.psso",
        )
        .map_err(|_| Error::Identity)?;
        Ok(())
    }
}
struct Pending {
    subject: String,
    session: u32,
    session_binding: Option<execution_contract::Id>,
    context: ::platform_sso::Context,
    challenge: w::BootstrapResponse,
}
#[derive(Default)]
pub(super) struct State {
    pub commands: VecDeque<Command>,
    pending: BTreeMap<Uuid, Pending>,
    next_retirement: Option<std::time::Instant>,
    retirement_cursor: Option<String>,
}
/// Root-owned record name; a generated UID cannot introduce path syntax.
pub(crate) fn binding_name(account: &str) -> Result<String, Error> {
    if !wire::generated_uid_valid(account) {
        return Err(Error::Identity);
    }
    Ok(format!("{account}.psso-binding"))
}
impl<S: IdentityProvider> DeviceService<S> {
    fn psso_context(
        &self,
        command: &Command,
        kind: w::Kind,
        require_enabled: bool,
    ) -> Result<::platform_sso::Context, Error> {
        let deployment = self.installation.as_ref().ok_or(Error::Configuration)?;
        let config = deployment.platform_sso.as_ref().ok_or(Error::Unsupported)?;
        if require_enabled {
            enabled(&self.core.access.root, deployment.service_id)?;
        }
        let profile = self.client.registration()?;
        let mut context = ::platform_sso::Context {
            tenant: deployment.tenant,
            agent_registration: profile.registration_id,
            installation: self.core.access.installation,
            origin: config.origin.clone(),
            kind,
            binding: None,
            binding_generation: None,
            generated_uid: None,
        };
        if kind == w::Kind::User {
            #[cfg(target_os = "macos")]
            {
                let uid: u32 = command.subject.parse().map_err(|_| Error::Identity)?;
                if !execution_ipc::macos_identity::console_user(uid) || command.session == 0 {
                    return Err(Error::Identity);
                }
            }
            let account =
                crate::account_identity::generated_uid(&command.subject)?.ok_or(Error::Identity)?;
            let binding: ::platform_sso::LocalBinding =
                state::read_optional(&self.core.access.root, &binding_name(&account)?)?
                    .ok_or(Error::NotFound)?;
            if (require_enabled && !binding.enabled)
                || !binding.matches_identity(
                    context.installation,
                    deployment.tenant,
                    &profile,
                    &account,
                )
            {
                return Err(Error::Identity);
            }
            context.binding = Some(binding.binding);
            context.binding_generation = Some(binding.binding_generation);
            context.generated_uid = Some(account);
        }
        context.validate().map_err(|_| Error::Identity)?;
        Ok(context)
    }
    pub(super) async fn drive_platform_sso(
        &mut self,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<(), Error> {
        let now = (crate::wall()? / 1000) as i64;
        self.core
            .psso
            .pending
            .retain(|_, p| p.challenge.expires_at > now);
        let Some(command) = self.core.psso.commands.pop_front() else {
            self.retry_retirement(commands).await;
            return Ok(());
        };
        if command.reply.is_closed() {
            return Ok(());
        }
        let result = self.platform_sso(&command, commands).await;
        let reply = match result {
            Ok(value) => Reply::PlatformSso { value },
            Err(Error::Unavailable | Error::Network | Error::Storage) => Reply::Unavailable,
            Err(_) => Reply::Rejected,
        };
        let _ = command.reply.send(reply);
        Ok(())
    }
    async fn platform_sso(
        &mut self,
        command: &Command,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Response, Error> {
        let Some(Request::PlatformSso { input }) = command.request.as_operation() else {
            return Err(Error::Denied);
        };
        match input {
            Operation::Bootstrap { scope } => {
                if self.core.psso.pending.len() >= 16 {
                    return Err(Error::Capacity);
                }
                let context = match self.psso_context(command, *scope, true) {
                    Err(Error::NotFound) if *scope == w::Kind::User => {
                        return Ok(Response::NeedsAccount {})
                    }
                    result => result?,
                };
                let input = w::Bootstrap {
                    operation_id: Uuid::new_v4(),
                    kind: *scope,
                    binding_id: context.binding,
                    binding_generation: context.binding_generation,
                };
                let challenge =
                    network(self.client.psso_bootstrap(&input), &mut self.core, commands).await?;
                context
                    .check_bootstrap(&challenge, (crate::wall()? / 1000) as i64)
                    .map_err(|_| Error::Untrusted)?;
                self.core.psso.pending.insert(
                    challenge.id,
                    Pending {
                        subject: command.subject.clone(),
                        session: command.session,
                        session_binding: command.binding.clone(),
                        context: context.clone(),
                        challenge: challenge.clone(),
                    },
                );
                Ok(Response::Bootstrap { context, challenge })
            }
            Operation::Register { input } => {
                let pending = self
                    .core
                    .psso
                    .pending
                    .remove(&input.challenge_id)
                    .ok_or(Error::Identity)?;
                if pending.subject != command.subject
                    || pending.session != command.session
                    || pending.session_binding != command.binding
                    || pending.challenge.expires_at <= (crate::wall()? / 1000) as i64
                {
                    return Err(Error::Identity);
                }
                let current = self.psso_context(command, pending.context.kind, true)?;
                if current != pending.context {
                    return Err(Error::Identity);
                }
                let receipt =
                    network(self.client.psso_register(input), &mut self.core, commands).await?;
                if !receipt.registered
                    || receipt.source_id != pending.challenge.source_id
                    || receipt.source_version != pending.challenge.source_version
                    || receipt.kind != current.kind
                {
                    return Err(Error::Untrusted);
                }
                let _lock = state::lock(&self.core.access.root)?;
                let coordinate = Coordinate {
                    context: current,
                    source: receipt.source_id,
                    source_version: receipt.source_version,
                };
                state::write(
                    &self.core.access.root,
                    &coordinate.file("source")?,
                    &coordinate,
                )?;
                Ok(Response::Registered { receipt })
            }
            Operation::Status { scope } => {
                let context = self.psso_context(command, *scope, false)?;
                let value = network(
                    self.client.psso_status(context.binding),
                    &mut self.core,
                    commands,
                )
                .await?;
                let source = match scope {
                    w::Kind::Device if value.device_registered => value.device_source.as_ref(),
                    w::Kind::User if value.user_registered => value.user_source.as_ref(),
                    _ => None,
                };
                if let Some(source) = source {
                    let _lock = state::lock(&self.core.access.root)?;
                    let coordinate = Coordinate {
                        context,
                        source: source.id,
                        source_version: source.version,
                    };
                    state::write(
                        &self.core.access.root,
                        &coordinate.file("source")?,
                        &coordinate,
                    )?;
                }
                Ok(Response::Status { value })
            }
            Operation::Observe {
                scope,
                source,
                source_version,
                device_registered,
                user_registered,
            } => {
                let context = self.psso_context(command, *scope, false)?;
                let account = context.generated_uid.as_deref().unwrap_or("device");
                let _lock = state::lock(&self.core.access.root)?;
                let retained: Coordinate = state::read_optional(
                    &self.core.access.root,
                    &format!("{account}.psso-source"),
                )?
                .ok_or(Error::Identity)?;
                if !retained.same_owner(&context)
                    || retained.source != *source
                    || retained.source_version != *source_version
                {
                    return Err(Error::Identity);
                }
                // Native observations remain available while HTTPS is unavailable.
                state::write(
                    &self.core.access.root,
                    &format!("{account}.psso-observation"),
                    &serde_json::json!({
                    "source": source, "sourceVersion": source_version,
                    "bindingGeneration": retained.context.binding_generation, "observedAt": crate::wall()?,
                    "deviceRegistered": device_registered, "userRegistered": user_registered }),
                )?;
                Ok(Response::Observed {})
            }
            Operation::Unregister {
                scope,
                source,
                source_version,
            } => {
                let context = self.psso_context(command, *scope, false)?;
                let coordinate = Coordinate {
                    context,
                    source: *source,
                    source_version: *source_version,
                };
                if coordinate.source.is_nil() || coordinate.source_version < 1 {
                    return Err(Error::Identity);
                }
                let coordinate = {
                    let _lock = state::lock(&self.core.access.root)?;
                    let retained: Coordinate =
                        state::read_optional(&self.core.access.root, &coordinate.file("source")?)?
                            .ok_or(Error::Identity)?;
                    if !retained.same_owner(&coordinate.context)
                        || retained.source != coordinate.source
                    {
                        return Err(Error::Identity);
                    }
                    if retained.source_version > coordinate.source_version {
                        return Ok(Response::Retired {});
                    }
                    if retained.source_version != coordinate.source_version {
                        return Err(Error::Identity);
                    }
                    state::write(&self.core.access.root, &retained.file("retire")?, &retained)?;
                    retained
                };
                self.retire_coordinate(&coordinate, commands).await
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeObservation {
    source: Uuid,
    source_version: i64,
    binding_generation: Option<i64>,
    observed_at: u64,
    device_registered: bool,
    user_registered: bool,
}
impl<S: IdentityProvider> DeviceService<S> {
    pub(super) async fn mac_access_status(
        &mut self,
        command: &Command,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<execution_ipc::device_access::MacStatus, Error> {
        use execution_ipc::device_access::MacStatus;
        let installed = self
            .installation
            .as_ref()
            .and_then(|d| d.platform_sso.as_ref())
            .is_some_and(|c| c.verify().is_ok());
        let mut result = MacStatus {
            extension_installed: installed,
            device_registered: None,
            user_registered: None,
            login_ready: None,
            native_device_registered: None,
            native_user_registered: None,
            observed_at: None,
        };
        if !installed {
            return Ok(result);
        }
        let context = match self.psso_context(command, w::Kind::User, false) {
            Ok(v) => v,
            Err(Error::NotFound) => return Ok(result),
            Err(e) => return Err(e),
        };
        if let Some(account) = context.generated_uid.as_deref() {
            let retained: Option<Coordinate> =
                state::read_optional(&self.core.access.root, &format!("{account}.psso-source"))?;
            let observation: Option<NativeObservation> = state::read_optional(
                &self.core.access.root,
                &format!("{account}.psso-observation"),
            )?;
            if let Some(o) = observation.filter(|o| {
                context.binding_generation == o.binding_generation
                    && retained.as_ref().is_some_and(|c| {
                        c.context == context
                            && c.source == o.source
                            && c.source_version == o.source_version
                    })
            }) {
                result.native_device_registered = Some(o.device_registered);
                result.native_user_registered = Some(o.user_registered);
                result.observed_at = Some(o.observed_at);
            }
        }
        if let Ok(value) = network(
            self.client.psso_status(context.binding),
            &mut self.core,
            commands,
        )
        .await
        {
            result.device_registered = Some(value.device_registered);
            result.user_registered = Some(value.user_registered);
            result.login_ready = Some(value.login_ready);
        }
        Ok(result)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Coordinate {
    context: ::platform_sso::Context,
    source: Uuid,
    source_version: i64,
}
impl Coordinate {
    fn same_owner(&self, current: &::platform_sso::Context) -> bool {
        let mut owner = self.context.clone();
        if owner.binding_generation > current.binding_generation {
            return false;
        }
        owner.binding_generation = current.binding_generation;
        owner == *current
    }
    fn file(&self, suffix: &str) -> Result<String, Error> {
        self.context.validate().map_err(|_| Error::Identity)?;
        let account = self.context.generated_uid.as_deref().unwrap_or("device");
        Ok(format!("{account}.psso-{suffix}"))
    }
}
impl<S: IdentityProvider> DeviceService<S> {
    async fn retire_coordinate(
        &mut self,
        coordinate: &Coordinate,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Response, Error> {
        let registration = self.client.registration()?;
        if coordinate.context.agent_registration != registration.registration_id
            || coordinate.context.tenant != self.client.configuration().tenant
        {
            finish_retirement(&self.core.access.root, coordinate)?;
            return Ok(Response::Retired {});
        }
        let status = network(
            self.client.psso_status(coordinate.context.binding),
            &mut self.core,
            commands,
        )
        .await?;
        let source = match coordinate.context.kind {
            w::Kind::Device => status.device_source,
            w::Kind::User => status.user_source,
        };
        // The decision avoids unnecessary writes; the server CAS is the race fence.
        let response = match ::platform_sso::retirement(
            coordinate.source,
            coordinate.source_version,
            source.as_ref(),
        )
        .map_err(|_| Error::Identity)?
        {
            ::platform_sso::Retirement::Revoke => {
                let receipt = network(
                    self.client
                        .psso_revoke(coordinate.source, coordinate.source_version),
                    &mut self.core,
                    commands,
                )
                .await?;
                if !receipt.revoked
                    || receipt.source_id != coordinate.source
                    || Some(receipt.source_version) != coordinate.source_version.checked_add(1)
                {
                    return Err(Error::Untrusted);
                }
                Response::Revoked { receipt }
            }
            ::platform_sso::Retirement::AlreadyRevoked => Response::Revoked {
                receipt: w::RevocationReceipt {
                    source_id: coordinate.source,
                    source_version: source.ok_or(Error::Untrusted)?.version,
                    revoked: true,
                },
            },
            ::platform_sso::Retirement::Superseded => Response::Retired {},
        };
        finish_retirement(&self.core.access.root, coordinate)?;
        Ok(response)
    }
    async fn retry_retirement(&mut self, commands: &mut tokio::sync::mpsc::Receiver<Command>) {
        let now = std::time::Instant::now();
        if self
            .core
            .psso
            .next_retirement
            .is_some_and(|next| next > now)
        {
            return;
        }
        self.core.psso.next_retirement = Some(now + std::time::Duration::from_secs(5));
        if self.client.runtime_readiness().is_err() {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&self.core.access.root) else {
            return;
        };
        let mut names = entries
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".psso-retire"))
            .take(257)
            .collect::<Vec<_>>();
        if names.len() > 256 {
            return;
        }
        names.sort();
        let name = names
            .iter()
            .find(|name| {
                self.core
                    .psso
                    .retirement_cursor
                    .as_ref()
                    .is_none_or(|previous| *name > previous)
            })
            .or_else(|| names.first())
            .cloned();
        if let Some(name) = name {
            self.core.psso.retirement_cursor = Some(name.clone());
            if let Ok(Some(coordinate)) =
                state::read_optional::<Coordinate>(&self.core.access.root, &name)
            {
                let _ = self.retire_coordinate(&coordinate, commands).await;
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Disabled {
    installation: Uuid,
}
pub(crate) fn enabled(root: &std::path::Path, installation: Uuid) -> Result<(), Error> {
    if let Some(disabled) = state::read_optional::<Disabled>(root, "platform-sso.disabled")? {
        return Err(if disabled.installation == installation {
            Error::Denied
        } else {
            Error::Identity
        });
    }
    Ok(())
}
/// P6 calls these bounded root operations; Apple key cleanup remains in native callbacks.
pub(crate) fn component(
    deployment: &crate::deployment::Deployment,
    args: &[std::ffi::OsString],
) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    if unsafe { libc::geteuid() } != 0 {
        return Err(Error::Denied);
    }
    #[cfg(not(target_os = "macos"))]
    return Err(Error::Unsupported);
    let [action] = args else {
        return Err(Error::Configuration);
    };
    let action = action.to_str().ok_or(Error::Configuration)?;
    if !["inspect", "disable", "retire", "cleanup"].contains(&action) {
        return Err(Error::Configuration);
    }
    deployment
        .service
        .verify(&std::env::current_exe()?)
        .map_err(|_| Error::Identity)?;
    let root = crate::access_worker::root(&deployment.execution);
    let _lock = state::lock(&root)?;
    let mut coordinates = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let name = entry?
            .file_name()
            .into_string()
            .map_err(|_| Error::Storage)?;
        if !name.ends_with(".psso-source") {
            continue;
        }
        if coordinates.len() >= 256 {
            return Err(Error::Capacity);
        }
        let coordinate: Coordinate = state::read_optional(&root, &name)?.ok_or(Error::Storage)?;
        if coordinate.file("source")? != name
            || coordinate.context.installation != deployment.service_id
            || coordinate.context.tenant != deployment.tenant
        {
            return Err(Error::Identity);
        }
        coordinates.push(coordinate);
    }
    if action != "inspect" {
        state::write(
            &root,
            "platform-sso.disabled",
            &Disabled {
                installation: deployment.service_id,
            },
        )?;
    }
    if action == "retire" {
        for coordinate in &coordinates {
            state::write(&root, &coordinate.file("retire")?, coordinate)?;
        }
    }
    let mut observations = Vec::new();
    let mut complete = true;
    for coordinate in &coordinates {
        let retired: Option<Coordinate> =
            state::read_optional(&root, &coordinate.file("retired")?)?;
        let native: Option<NativeObservation> =
            state::read_optional(&root, &coordinate.file("observation")?)?;
        let remote_done = retired.as_ref().is_some_and(|r| r == coordinate);
        let native_done = native.is_some_and(|n| {
            n.source == coordinate.source
                && n.source_version == coordinate.source_version
                && n.binding_generation == coordinate.context.binding_generation
                && !n.user_registered
                && (coordinate.context.kind == w::Kind::User || !n.device_registered)
        });
        complete &= remote_done && native_done;
        observations.push(serde_json::json!({"source": coordinate.source, "sourceVersion": coordinate.source_version,
            "scope": coordinate.context.kind, "remoteRetired": remote_done, "nativeCleanupConfirmed": native_done}));
    }
    if action == "cleanup" && complete {
        for coordinate in &coordinates {
            for suffix in ["source", "retire", "retired", "observation"] {
                state::remove(&root, &coordinate.file(suffix)?)?;
            }
        }
    }
    let disabled = match enabled(&root, deployment.service_id) {
        Ok(()) => false,
        Err(Error::Denied) => true,
        Err(e) => return Err(e),
    };
    println!(
        "{}",
        serde_json::json!({"componentValid": deployment.platform_sso.as_ref().is_some_and(|c| c.verify().is_ok()),
        "disabled": disabled, "sources": observations,
        "cleanupComplete": action == "cleanup" && complete})
    );
    if action == "cleanup" && !complete {
        return Err(Error::Unavailable);
    }
    Ok(())
}

fn finish_retirement(root: &std::path::Path, coordinate: &Coordinate) -> Result<(), Error> {
    let _lock = state::lock(root)?;
    let pending: Option<Coordinate> = state::read_optional(root, &coordinate.file("retire")?)?;
    if pending.as_ref() == Some(coordinate) {
        state::write(root, &coordinate.file("retired")?, coordinate)?;
        state::remove(root, &coordinate.file("retire")?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn coordinate() -> Coordinate {
        Coordinate {
            context: ::platform_sso::Context {
                tenant: Uuid::from_u128(1),
                agent_registration: Uuid::from_u128(2),
                installation: Uuid::from_u128(3),
                origin: "https://mdm.example".into(),
                kind: w::Kind::User,
                binding: Some(Uuid::from_u128(4)),
                binding_generation: Some(1),
                generated_uid: Some(Uuid::from_u128(5).to_string()),
            },
            source: Uuid::from_u128(6),
            source_version: 1,
        }
    }
    #[test]
    fn lost_retirement_reply_cannot_clear_a_new_persisted_request() {
        let root = std::env::temp_dir().join(format!("psso-retirement-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let old = coordinate();
        let mut replacement = old.clone();
        replacement.source_version = 2;
        state::write(&root, &replacement.file("retire").unwrap(), &replacement).unwrap();
        finish_retirement(&root, &old).unwrap();
        let pending: Coordinate = state::read_optional(&root, &old.file("retire").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(pending.source_version, 2);
        assert!(
            state::read_optional::<Coordinate>(&root, &old.file("retired").unwrap())
                .unwrap()
                .is_none()
        );
        finish_retirement(&root, &replacement).unwrap();
        assert!(
            state::read_optional::<Coordinate>(&root, &old.file("retire").unwrap())
                .unwrap()
                .is_none()
        );
        let completed: Coordinate = state::read_optional(&root, &old.file("retired").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(completed.source_version, 2);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cleanup_observation_owner_does_not_follow_account_or_registration_replacement() {
        let old = coordinate();
        let mut current = old.context.clone();
        current.binding_generation = Some(2);
        assert!(old.same_owner(&current)); // Unbind may advance the same account binding.
        current.generated_uid = Some(Uuid::new_v4().to_string());
        assert!(!old.same_owner(&current));
        current = old.context.clone();
        current.agent_registration = Uuid::new_v4();
        assert!(!old.same_owner(&current));
        current = old.context.clone();
        current.binding_generation = Some(0);
        assert!(!old.same_owner(&current));
    }
}
