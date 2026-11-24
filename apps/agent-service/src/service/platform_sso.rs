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
        if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
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
        crate::installation::validate_native_artifact(&self.extension)?;
        Ok(())
    }
}
pub(crate) struct Pending {
    callback_generation: u64,
    authentication: w::AuthenticationMode,
    subject: String,
    session: u32,
    session_binding: Option<execution_contract::Id>,
    context: ::platform_sso::Context,
    challenge: w::BootstrapResponse,
}
#[derive(Default)]
pub(crate) struct State {
    pub commands: VecDeque<Command>,
    pending: BTreeMap<Uuid, Pending>,
    next_retirement: Option<std::time::Instant>,
    retirement_cursor: Option<String>,
    retiring: bool,
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
            enabled(&self.core.access.root, deployment.mdm()?.instance)?;
        }
        let profile = self.client.registration()?;
        let context = ::platform_sso::Context {
            authority: rss_mdm_agent_wire::platform_sso::RegistrationTrust::MdmBootstrapPop,
            tenant: deployment.mdm()?.tenant,
            device_id: profile.device_id,
            installation: self.core.access.installation,
            origin: config.origin.clone(),
            kind,
            binding: None,
            binding_generation: None,
            generated_uid: None,
            binding_revision: 0,
            configuration: None,
        };
        complete_context(&self.core.access.root, command, context, require_enabled)
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
            Mdm {
                service: self,
                commands,
            }
            .retry_retirement()
            .await;
            return Ok(());
        };
        if command.reply.is_closed() {
            return Ok(());
        }
        let result =
            if self.core.psso.retiring && !cleanup_operation(command.request.as_operation()) {
                Err(Error::Denied)
            } else {
                Mdm {
                    service: self,
                    commands,
                }
                .platform_sso(&command)
                .await
            };
        let reply = match result {
            Ok(value) => Reply::PlatformSso { value },
            Err(Error::Unavailable | Error::Network | Error::Storage) => Reply::Unavailable,
            Err(_) => Reply::Rejected,
        };
        let _ = command.reply.send(reply);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub(crate) mod da;

fn complete_context(
    root: &std::path::Path,
    command: &Command,
    mut context: ::platform_sso::Context,
    require_enabled: bool,
) -> Result<::platform_sso::Context, Error> {
    if require_enabled {
        require_owner(
            root,
            (
                context.authority,
                context.installation,
                context.tenant,
                context.device_id,
            ),
        )?;
    }
    if context.kind == w::Kind::User {
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
            state::read_optional(root, &binding_name(&account)?)?.ok_or(Error::NotFound)?;
        if binding.authority != context.authority
            || (require_enabled && !binding.enabled)
            || !binding.matches_identity(
                context.installation,
                context.tenant,
                context.device_id,
                &account,
            )
        {
            return Err(Error::Identity);
        }
        context.binding = Some(binding.binding);
        context.binding_generation = Some(binding.binding_generation);
        context.generated_uid = Some(account);
        context.binding_revision = binding.revision;
        context.configuration = Some(binding.configuration);
    }
    context.validate().map_err(|_| Error::Identity)?;
    Ok(context)
}
type Identity = (
    w::RegistrationTrust,
    Uuid,
    Uuid,
    rss_mdm_agent_wire::DeviceId,
);

struct Mdm<'a, S: IdentityProvider> {
    service: &'a mut DeviceService<S>,
    commands: &'a mut tokio::sync::mpsc::Receiver<Command>,
}
impl<S: IdentityProvider> Owner for Mdm<'_, S> {
    fn state(&mut self) -> &mut State {
        &mut self.service.core.psso
    }
    fn root(&self) -> &std::path::Path {
        &self.service.core.access.root
    }
    fn identity(&self) -> Result<Identity, Error> {
        let client = &self.service.client;
        Ok((
            w::RegistrationTrust::MdmBootstrapPop,
            self.service.core.access.installation,
            client.configuration().tenant,
            client.registration()?.device_id,
        ))
    }
    async fn context(
        &self,
        command: &Command,
        kind: w::Kind,
        require_enabled: bool,
    ) -> Result<::platform_sso::Context, Error> {
        self.service.psso_context(command, kind, require_enabled)
    }
    async fn bootstrap(&mut self, input: &w::Bootstrap) -> Result<w::BootstrapResponse, Error> {
        network(
            self.service.client.psso_bootstrap(input),
            &mut self.service.core,
            self.commands,
        )
        .await
    }
    async fn register(
        &mut self,
        input: &w::Registration,
    ) -> Result<Option<w::RegistrationReceipt>, Error> {
        network(
            self.service.client.psso_register(input),
            &mut self.service.core,
            self.commands,
        )
        .await
    }
    async fn status(&mut self, binding: Option<Uuid>) -> Result<w::Status, Error> {
        network(
            self.service.client.psso_status(binding),
            &mut self.service.core,
            self.commands,
        )
        .await
    }
    async fn revoke(&mut self, source: Uuid, version: i64) -> Result<w::RevocationReceipt, Error> {
        network(
            self.service.client.psso_revoke(source, version),
            &mut self.service.core,
            self.commands,
        )
        .await
    }
}

pub(crate) trait Owner {
    fn state(&mut self) -> &mut State;
    fn root(&self) -> &std::path::Path;
    fn identity(&self) -> Result<Identity, Error>;
    async fn context(
        &self,
        command: &Command,
        kind: w::Kind,
        require_enabled: bool,
    ) -> Result<::platform_sso::Context, Error>;
    async fn bootstrap(&mut self, input: &w::Bootstrap) -> Result<w::BootstrapResponse, Error>;
    async fn register(
        &mut self,
        input: &w::Registration,
    ) -> Result<Option<w::RegistrationReceipt>, Error>;
    async fn status(&mut self, binding: Option<Uuid>) -> Result<w::Status, Error>;
    async fn revoke(&mut self, source: Uuid, version: i64) -> Result<w::RevocationReceipt, Error>;
    async fn mac_status(
        &mut self,
        command: &Command,
        installed: bool,
    ) -> Result<execution_ipc::device_access::MacStatus, Error> {
        use execution_ipc::device_access::MacStatus;
        let mut result = MacStatus {
            extension_installed: installed,
            configuration: None,
            password_exchange: None,
            device_registered: None,
            user_registered: None,
            login_ready: None,
            native_configuration_matches: None,
            login_keychain: execution_ipc::device_access::KeychainStatus::Unavailable,
            native_device_registered: None,
            native_user_registered: None,
            observed_at: None,
        };
        if !installed {
            return Ok(result);
        }
        let context = match self.context(command, w::Kind::User, false).await {
            Ok(v) => v,
            Err(Error::NotFound) => return Ok(result),
            Err(e) => return Err(e),
        };
        result.configuration = context.configuration.clone();
        if let Some(account) = context.generated_uid.as_deref() {
            let retained: Option<Coordinate> =
                state::read_optional(self.root(), &format!("{account}.psso-source"))?;
            let observation: Option<NativeObservation> =
                state::read_optional(self.root(), &format!("{account}.psso-observation"))?;
            if let Some(o) = observation.filter(|o| {
                context.binding_generation == o.binding_generation
                    && o.live_session()
                    && retained.as_ref().is_some_and(|c| {
                        observation_owner(&c.context, &context)
                            && o.belongs_to(c)
                            && !retirement_requested(self.root(), c).unwrap_or(true)
                            && c.source == o.source
                            && c.source_version == o.source_version
                    })
            }) {
                result.native_configuration_matches = o.configuration;
                result.native_device_registered = Some(o.device_registered);
                result.native_user_registered = Some(o.user_registered);
                result.observed_at = Some(o.observed_at);
            }
        }
        if let Ok(value) = self.status(context.binding).await {
            if value.registration_trust != context.authority
                || value.hardware_attested
                || self.context(command, w::Kind::User, false).await? != context
            {
                return Err(Error::Identity);
            }
            result.password_exchange = value.password_exchange;
            result.device_registered = Some(value.device_registered);
            result.user_registered = Some(value.user_registered);
            result.login_ready = Some(value.login_ready);
        }
        Ok(result)
    }
    async fn platform_sso(&mut self, command: &Command) -> Result<Response, Error> {
        let Some(Request::PlatformSso { input }) = command.request.as_operation() else {
            return Err(Error::Denied);
        };
        match input {
            Operation::Bootstrap {
                callback_generation,
                scope,
                authentication,
            } => {
                let now = (crate::wall()? / 1000) as i64;
                self.state()
                    .pending
                    .retain(|_, p| p.challenge.expires_at > now);
                if *callback_generation == 0 {
                    return Err(Error::Identity);
                }
                if self.state().pending.len() >= 16 {
                    return Err(Error::Capacity);
                }
                let context = match self.context(command, *scope, true).await {
                    Err(Error::NotFound) if *scope == w::Kind::User => {
                        return Ok(Response::NeedsAccount {})
                    }
                    result => result?,
                };
                if context
                    .configuration
                    .as_ref()
                    .is_some_and(|c| c.authentication != *authentication)
                {
                    return Err(Error::Unsupported);
                }
                let input = w::Bootstrap {
                    operation_id: Uuid::new_v4(),
                    kind: *scope,
                    binding_id: context.binding,
                    binding_generation: context.binding_generation,
                };
                let challenge = self.bootstrap(&input).await?;
                if self.context(command, *scope, true).await? != context {
                    return Err(Error::Identity);
                }
                context
                    .check_bootstrap(&challenge, (crate::wall()? / 1000) as i64)
                    .map_err(|_| Error::Untrusted)?;
                self.state().pending.insert(
                    challenge.id,
                    Pending {
                        callback_generation: *callback_generation,
                        authentication: *authentication,
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
                    .state()
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
                let current = self.context(command, pending.context.kind, true).await?;
                if current != pending.context
                    || (current.kind == w::Kind::User
                        && current.configuration.as_ref().is_some_and(|c| {
                            (c.authentication == w::AuthenticationMode::Password)
                                != input.signing_key.is_none()
                        }))
                {
                    return Err(Error::Identity);
                }
                let coordinate = Coordinate {
                    operation: input.operation_id,
                    client_id: pending.challenge.client_id,
                    authentication: pending.authentication,
                    callback_generation: pending.callback_generation,
                    context: current.clone(),
                    source: pending.challenge.source_id,
                    source_version: pending.challenge.source_version,
                };
                {
                    let _lock = state::lock(self.root())?;
                    enabled(self.root(), current.installation)?;
                    require_owner(
                        self.root(),
                        (
                            current.authority,
                            current.installation,
                            current.tenant,
                            current.device_id,
                        ),
                    )?;
                    if retirement_requested(self.root(), &coordinate)? {
                        return Err(Error::Denied);
                    }
                    state::write(self.root(), &coordinate.file("registering")?, &coordinate)?;
                }
                let receipt = self.register(input).await?;
                let Some(receipt) = receipt else {
                    settle_rejected_registration(self.root(), &coordinate)?;
                    return Err(Error::Denied);
                };
                if receipt.registration_trust != current.authority
                    || receipt.hardware_attested
                    || !receipt.registered
                    || receipt.source_id != pending.challenge.source_id
                    || receipt.source_version != pending.challenge.source_version
                    || receipt.kind != current.kind
                {
                    return Err(Error::Untrusted);
                }
                if self
                    .context(command, pending.context.kind, true)
                    .await
                    .ok()
                    .as_ref()
                    != Some(&current)
                {
                    queue_retirement(self.root(), coordinate)?;
                    return Err(Error::Identity);
                }
                if !retain_registration(self.root(), &coordinate)? {
                    return Err(Error::Denied);
                }
                Ok(Response::Registered { receipt })
            }
            Operation::Status { scope } => {
                let context = self.context(command, *scope, true).await?;
                let value = self.status(context.binding).await?;
                if value.registration_trust != context.authority
                    || value.hardware_attested
                    || self.context(command, *scope, true).await? != context
                {
                    return Err(Error::Identity);
                }
                let source = match scope {
                    w::Kind::Device if value.device_registered => value.device_source.as_ref(),
                    w::Kind::User if value.user_registered => value.user_source.as_ref(),
                    _ => None,
                };
                if let Some(source) = source {
                    // Recover only the protected original registering intent. Server status alone
                    // cannot reconstruct the expected native configuration.
                    let coordinate =
                        retained_coordinate(self.root(), &context, source.id, source.version)?;
                    if !retain_registration(self.root(), &coordinate)? {
                        return Err(Error::Denied);
                    }
                }
                Ok(Response::Status { value })
            }
            Operation::Observe { scope, .. } => {
                let context = self.context(command, *scope, false).await?;
                record_observation(
                    self.root(),
                    &context,
                    input,
                    &command.subject,
                    command.session,
                    command.binding.as_ref(),
                )?;
                Ok(Response::Observed {})
            }
            Operation::Unregister {
                scope,
                source,
                source_version,
            } => {
                let context = self.context(command, *scope, false).await?;
                let coordinate =
                    retained_coordinate(self.root(), &context, *source, *source_version)?;
                if coordinate.source.is_nil() || coordinate.source_version < 1 {
                    return Err(Error::Identity);
                }
                // Remove any not-yet-submitted challenge first. Register and Unregister share this
                // single owner; a request already on the wire has a durable registering record.
                self.state().pending.retain(|_, p| {
                    !(p.context == coordinate.context
                        && p.challenge.source_id == coordinate.source
                        && p.challenge.source_version == coordinate.source_version)
                });
                if let Some(RemoteRetirement::RegistrationWithdrawn { .. }) =
                    resolution(self.root(), &coordinate)?
                {
                    return Ok(Response::RegistrationWithdrawn {});
                }
                let coordinate = queue_retirement(self.root(), coordinate)?;
                self.retire_coordinate(&coordinate).await
            }
        }
    }
    async fn retire_coordinate(&mut self, coordinate: &Coordinate) -> Result<Response, Error> {
        if let Some(RemoteRetirement::RegistrationWithdrawn { .. }) =
            resolution(self.root(), coordinate)?
        {
            return Ok(Response::RegistrationWithdrawn {});
        }
        if self.identity()?
            != (
                coordinate.context.authority,
                coordinate.context.installation,
                coordinate.context.tenant,
                coordinate.context.device_id,
            )
        {
            return Err(Error::Identity);
        }
        let status = self.status(coordinate.context.binding).await?;
        if status.registration_trust != coordinate.context.authority || status.hardware_attested {
            return Err(Error::Untrusted);
        }
        let source = match coordinate.context.kind {
            w::Kind::Device => status.device_source,
            w::Kind::User => status.user_source,
        };
        if registration_unresolved(self.root(), coordinate, source.as_ref())? {
            // Absence is a snapshot, not a fence against a still-unresolved first registration.
            return Err(Error::Unavailable);
        }
        // The decision avoids unnecessary writes; the server CAS is the race fence.
        let response = match ::platform_sso::retirement(
            coordinate.source,
            coordinate.source_version,
            source.as_ref(),
        )
        .map_err(|_| Error::Identity)?
        {
            ::platform_sso::Retirement::Revoke => {
                let receipt = self
                    .revoke(coordinate.source, coordinate.source_version)
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
        finish_retirement(
            self.root(),
            coordinate,
            if matches!(response, Response::Revoked { .. }) {
                RemoteRetirement::Revoked
            } else {
                RemoteRetirement::Superseded
            },
        )?;
        Ok(response)
    }
    async fn retry_retirement(&mut self) {
        let now = std::time::Instant::now();
        if self.state().next_retirement.is_some_and(|next| next > now) {
            return;
        }
        self.state().next_retirement = Some(now + std::time::Duration::from_secs(5));
        let Ok(entries) = std::fs::read_dir(self.root()) else {
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
                self.state()
                    .retirement_cursor
                    .as_ref()
                    .is_none_or(|previous| *name > previous)
            })
            .or_else(|| names.first())
            .cloned();
        if let Some(name) = name {
            self.state().retirement_cursor = Some(name.clone());
            if let Ok(Some(coordinate)) = state::read_optional::<Coordinate>(self.root(), &name) {
                let _ = self.retire_coordinate(&coordinate).await;
            }
        }
    }
}
fn record_observation(
    root: &std::path::Path,
    context: &::platform_sso::Context,
    input: &Operation,
    subject: &str,
    session: u32,
    session_binding: Option<&execution_contract::Id>,
) -> Result<(), Error> {
    let Operation::Observe {
        callback_generation,
        configuration,
        source,
        source_version,
        device_registered,
        user_registered,
        ..
    } = input
    else {
        return Err(Error::Protocol);
    };
    let account = context.generated_uid.as_deref().unwrap_or("device");
    let _lock = state::lock(root)?;
    let retained: Coordinate =
        state::read_optional(root, &format!("{account}.psso-source"))?.ok_or(Error::Identity)?;
    if !retained.same_owner(context)
        || retained.source != *source
        || retained.source_version != *source_version
        || retained.callback_generation != *callback_generation
    {
        return Err(Error::Identity);
    }
    let observation = NativeObservation {
        source: *source,
        source_version: *source_version,
        binding_generation: retained.context.binding_generation,
        observed_at: crate::wall()?,
        callback_generation: *callback_generation,
        subject: subject.into(),
        session,
        session_binding: session_binding.cloned(),
        configuration: configuration
            .as_ref()
            .map(|actual| {
                actual.matches(
                    &retained.context,
                    retained.client_id,
                    retained.authentication,
                    retained.source,
                )
            })
            .transpose()
            .map_err(|_| Error::Identity)?,
        device_registered: *device_registered,
        user_registered: *user_registered,
    };
    state::write(root, &format!("{account}.psso-observation"), &observation)
}
fn observation_owner(
    retained: &::platform_sso::Context,
    current: &::platform_sso::Context,
) -> bool {
    let mut retained = retained.clone();
    if retained.binding_revision > current.binding_revision {
        return false;
    }
    retained.binding_revision = current.binding_revision;
    retained == *current
}
fn cleanup_operation(request: Option<&Request>) -> bool {
    matches!(
        request,
        Some(Request::PlatformSso {
            input: Operation::Observe { .. } | Operation::Unregister { .. }
        })
    )
}
impl<S: IdentityProvider> DeviceService<S> {
    /// Keep authenticated native cleanup alive after business admission is revoked.
    pub(super) async fn drain_platform_sso(
        &mut self,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<bool, Error> {
        self.core.psso.retiring = true;
        self.core.psso.pending.clear();
        let root = self.core.access.root.clone();
        if !root.exists() {
            return Ok(true);
        }
        #[cfg(target_os = "macos")]
        if has_authority(&root, w::RegistrationTrust::DaDevicePop)? {
            return Ok(false);
        }
        sweep_retirement(&root, self.core.access.installation)?;
        while let Ok(command) = commands.try_recv() {
            self.core.handle(command);
        }
        self.drive_platform_sso(commands).await?;
        let complete = sweep_retirement(&root, self.core.access.installation)?;
        if !complete {
            // Cleanup may outlive trust/CRL validity. Keep the existing PKI owner's bounded
            // recovery running without reopening business admission or new registration.
            // Process native observations first, even when the network is unavailable.
            network(self.client.drive_pki(), &mut self.core, commands).await?;
        }
        Ok(complete)
    }
}
/// The existing owner lock serializes retirement with registering and late completion.
fn sweep_retirement(root: &std::path::Path, installation: Uuid) -> Result<bool, Error> {
    let _lock = state::lock(root)?;
    state::write(root, "platform-sso.disabled", &Disabled { installation })?;
    let mut complete = true;
    let mut count = 0;
    for entry in std::fs::read_dir(root)? {
        let name = entry?
            .file_name()
            .into_string()
            .map_err(|_| Error::Storage)?;
        if name.ends_with(".psso-retire") {
            complete = false;
            continue;
        }
        let in_flight = name.ends_with(".psso-registering");
        if !in_flight && !name.ends_with(".psso-source") {
            continue;
        }
        count += 1;
        if count > 256 {
            return Err(Error::Capacity);
        }
        let c: Coordinate = state::read_optional(root, &name)?.ok_or(Error::Storage)?;
        if c.context.installation != installation
            || c.file(if in_flight { "registering" } else { "source" })? != name
        {
            return Err(Error::Identity);
        }
        let retired = resolution(root, &c)?;
        let native: Option<NativeObservation> =
            state::read_optional(root, &c.file("observation")?)?;
        if retired.is_some() && native_cleared(&c, in_flight, native.as_ref()) {
            clear_coordinate(root, &c, in_flight)?;
        } else {
            if retired.is_none() {
                state::write(root, &c.file("retire")?, &c)?;
            }
            complete = false;
        }
    }
    Ok(complete)
}
fn native_cleared(c: &Coordinate, in_flight: bool, native: Option<&NativeObservation>) -> bool {
    in_flight
        || native.is_some_and(|n| {
            n.source == c.source
                && n.source_version == c.source_version
                && n.binding_generation == c.context.binding_generation
                && n.callback_generation == c.callback_generation
                && !n.user_registered
                && (c.context.kind == w::Kind::User || !n.device_registered)
        })
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeObservation {
    callback_generation: u64,
    subject: String,
    session: u32,
    session_binding: Option<execution_contract::Id>,
    configuration: Option<bool>,
    source: Uuid,
    source_version: i64,
    binding_generation: Option<i64>,
    observed_at: u64,
    device_registered: bool,
    user_registered: bool,
}
impl NativeObservation {
    fn belongs_to(&self, c: &Coordinate) -> bool {
        self.source == c.source
            && self.source_version == c.source_version
            && self.callback_generation == c.callback_generation
            && self.binding_generation == c.context.binding_generation
    }
    fn live_session(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.session_binding.as_ref().is_some_and(|binding| {
                execution_ipc::macos_identity::verify_console(&self.subject, self.session, binding)
                    .is_ok()
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
}
/// Protected native observations for the exact signed local binding; no network or account writes.
pub(crate) fn native_facts(
    root: &std::path::Path,
    binding: &::platform_sso::LocalBinding,
) -> Result<Option<wire::AccessMacPlatformSsoFacts>, Error> {
    native_facts_in_session(root, binding, NativeObservation::live_session)
}
fn native_facts_in_session(
    root: &std::path::Path,
    binding: &::platform_sso::LocalBinding,
    session: impl FnOnce(&NativeObservation) -> bool,
) -> Result<Option<wire::AccessMacPlatformSsoFacts>, Error> {
    if !binding.enabled || enabled(root, binding.installation).is_err() {
        return Ok(None);
    }
    let Some(c) = state::read_optional::<Coordinate>(
        root,
        &format!("{}.psso-source", binding.generated_uid),
    )?
    else {
        return Ok(None);
    };
    let mut context = c.context.clone();
    context.binding = Some(binding.binding);
    context.binding_generation = Some(binding.binding_generation);
    context.binding_revision = binding.revision;
    context.configuration = Some(binding.configuration.clone());
    if c.context.authority != binding.authority
        || c.context.installation != binding.installation
        || c.context.tenant != binding.tenant
        || c.context.device_id != binding.device_id
        || c.context.generated_uid.as_deref() != Some(binding.generated_uid.as_str())
        || !observation_owner(&c.context, &context)
        || retirement_requested(root, &c)?
    {
        return Ok(None);
    }
    let Some(o) = state::read_optional::<NativeObservation>(root, &c.file("observation")?)? else {
        return Ok(None);
    };
    if !o.belongs_to(&c) || !session(&o) {
        return Ok(None);
    }
    let fact = |v| {
        if v {
            wire::AccessObservation::Present
        } else {
            wire::AccessObservation::Absent
        }
    };
    Ok(Some(wire::AccessMacPlatformSsoFacts {
        configuration: o
            .configuration
            .map_or(wire::AccessObservation::Unknown, fact),
        device_registered: fact(o.device_registered),
        user_registered: fact(o.user_registered),
        user_source_id: o.source,
        user_source_version: o.source_version,
    }))
}
impl<S: IdentityProvider> DeviceService<S> {
    pub(super) async fn mac_access_status(
        &mut self,
        command: &Command,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<execution_ipc::device_access::MacStatus, Error> {
        let installed = self
            .installation
            .as_ref()
            .and_then(|d| d.platform_sso.as_ref())
            .is_some_and(|c| c.verify().is_ok());
        Mdm {
            service: self,
            commands,
        }
        .mac_status(command, installed)
        .await
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Coordinate {
    operation: Uuid,
    client_id: Uuid,
    authentication: w::AuthenticationMode,
    callback_generation: u64,
    context: ::platform_sso::Context,
    source: Uuid,
    source_version: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum RemoteRetirement {
    Revoked,
    Superseded,
    /// The registration was revoked. Its now-inaccessible source has no independent revocation receipt.
    RegistrationWithdrawn {
        operation: Uuid,
        credential: Uuid,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Retired {
    coordinate: Coordinate,
    result: RemoteRetirement,
}
fn resolution(root: &Path, c: &Coordinate) -> Result<Option<RemoteRetirement>, Error> {
    let record: Option<Retired> = state::read_optional(root, &c.file("retired")?)?;
    record
        .map(|r| {
            if &r.coordinate == c {
                Ok(r.result)
            } else {
                Err(Error::Identity)
            }
        })
        .transpose()
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CleanupReceipt {
    operation: Uuid,
    installation: Uuid,
    tenant: Uuid,
    device: rss_mdm_agent_wire::DeviceId,
    authority: w::RegistrationTrust,
    binding: Option<Uuid>,
    binding_generation: Option<i64>,
    revision: i64,
    scope: w::Kind,
    source: Uuid,
    source_version: i64,
    remote: Option<RemoteRetirement>,
    native_cleanup_confirmed: Option<bool>,
}
fn cleanup_receipt(
    c: &Coordinate,
    in_flight: bool,
    remote: Option<RemoteRetirement>,
    native: bool,
) -> CleanupReceipt {
    CleanupReceipt {
        operation: c.operation,
        installation: c.context.installation,
        tenant: c.context.tenant,
        device: c.context.device_id,
        authority: c.context.authority,
        binding: c.context.binding,
        binding_generation: c.context.binding_generation,
        revision: c.context.binding_revision,
        scope: c.context.kind,
        source: c.source,
        source_version: c.source_version,
        remote,
        native_cleanup_confirmed: (!in_flight).then_some(native),
    }
}
fn clear_coordinate(root: &std::path::Path, c: &Coordinate, in_flight: bool) -> Result<(), Error> {
    let receipt = cleanup_receipt(
        c,
        in_flight,
        Some(resolution(root, c)?.ok_or(Error::Unavailable)?),
        true,
    );
    let name = format!("{}.psso-result", c.operation);
    state::write(root, &name, &receipt)?;
    state::remove(
        root,
        &c.file(if in_flight { "registering" } else { "source" })?,
    )?;
    state::remove(root, &c.file("retire")?)?;
    Ok(())
}

fn visit_live_sources(
    root: &std::path::Path,
    mut visit: impl FnMut(Coordinate) -> Result<(), Error>,
) -> Result<(), Error> {
    if !root.try_exists()? {
        return Ok(());
    }
    let mut count = 0;
    for entry in std::fs::read_dir(root)? {
        let name = entry?
            .file_name()
            .into_string()
            .map_err(|_| Error::Storage)?;
        if let Some((_, suffix)) = name.rsplit_once(".psso-") {
            if matches!(suffix, "source" | "registering" | "retire") {
                count += 1;
                if count > 256 {
                    return Err(Error::Capacity);
                }
                let c: Coordinate = state::read_optional(root, &name)?.ok_or(Error::Storage)?;
                if c.file(suffix)? != name {
                    return Err(Error::Identity);
                }
                visit(c)?;
            }
        }
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn find_source(
    root: &std::path::Path,
    id: Uuid,
    version: i64,
) -> Result<Option<Coordinate>, Error> {
    let mut found = None;
    visit_live_sources(root, |c| {
        if (c.source, c.source_version) == (id, version) {
            if found.as_ref().is_some_and(|old| old != &c) {
                return Err(Error::Conflict);
            }
            found = Some(c);
        }
        Ok(())
    })?;
    Ok(found)
}
#[cfg(target_os = "macos")]
fn has_authority(root: &std::path::Path, trust: w::RegistrationTrust) -> Result<bool, Error> {
    let mut found = false;
    visit_live_sources(root, |c| {
        found |= c.context.authority == trust;
        Ok(())
    })?;
    Ok(found)
}
/// A single Apple device source is shared across all users of this extension.
fn require_owner(root: &std::path::Path, identity: Identity) -> Result<(), Error> {
    visit_live_sources(root, |c| {
        if (
            c.context.authority,
            c.context.installation,
            c.context.tenant,
            c.context.device_id,
        ) != identity
        {
            return Err(Error::Conflict);
        }
        Ok(())
    })
}

fn retained_coordinate(
    root: &std::path::Path,
    context: &::platform_sso::Context,
    source: Uuid,
    version: i64,
) -> Result<Coordinate, Error> {
    let account = context.generated_uid.as_deref().unwrap_or("device");
    for file in [
        format!("{account}.psso-source"),
        format!("{account}-{source}-{version}.psso-registering"),
        format!("{account}-{source}-{version}.psso-retire"),
        format!("{account}-{source}-{version}.psso-retired"),
    ] {
        let record = if file.ends_with(".psso-retired") {
            state::read_optional::<Retired>(root, &file)?.map(|r| r.coordinate)
        } else {
            state::read_optional(root, &file)?
        };
        if let Some(c) = record {
            if c.source == source && c.source_version == version && c.same_owner(context) {
                return Ok(c);
            }
        }
    }
    Err(Error::Identity)
}
impl Coordinate {
    fn same_owner(&self, current: &::platform_sso::Context) -> bool {
        let mut owner = self.context.clone();
        if owner.binding_generation > current.binding_generation {
            return false;
        }
        owner.binding_generation = current.binding_generation;
        owner.binding_revision = current.binding_revision;
        owner.configuration = current.configuration.clone();
        owner == *current
    }
    fn file(&self, suffix: &str) -> Result<String, Error> {
        self.context.validate().map_err(|_| Error::Identity)?;
        let account = self.context.generated_uid.as_deref().unwrap_or("device");
        if ["registering", "retire", "retired"].contains(&suffix) {
            return Ok(format!(
                "{account}-{}-{}.psso-{suffix}",
                self.source, self.source_version
            ));
        }
        Ok(format!("{account}.psso-{suffix}"))
    }
}

/// Called with the existing access-state lock. A mode change retires the old source before
/// publishing the replacement policy; pending cleanup never silently becomes a new binding.
pub(crate) fn save_binding(
    root: &std::path::Path,
    next: &::platform_sso::LocalBinding,
) -> Result<(), Error> {
    require_owner(
        root,
        (
            next.authority,
            next.installation,
            next.tenant,
            next.device_id,
        ),
    )?;
    let name = binding_name(&next.generated_uid)?;
    let previous: Option<::platform_sso::LocalBinding> = state::read_optional(root, &name)?;
    if let Some(old) = previous.as_ref() {
        if old.authority != next.authority
            || old.installation != next.installation
            || old.tenant != next.tenant
            || old.device_id != next.device_id
            || (old.binding == next.binding && old.binding_generation > next.binding_generation)
            || (old.binding == next.binding && old.revision > next.revision)
            || (old.binding == next.binding
                && old.revision == next.revision
                && (old.configuration != next.configuration || old.enabled != next.enabled))
        {
            return Err(Error::Identity);
        }
        if !next.enabled
            || old.configuration != next.configuration
            || old.binding != next.binding
            || old.binding_generation != next.binding_generation
        {
            let mut disabled = old.clone();
            disabled.enabled = false;
            state::write(root, &name, &disabled)?;
            let prefix = next.generated_uid.clone();
            let mut complete = true;
            for entry in std::fs::read_dir(root)? {
                let file = entry?
                    .file_name()
                    .into_string()
                    .map_err(|_| Error::Storage)?;
                if !file.starts_with(&prefix)
                    || !(file.ends_with(".psso-source") || file.ends_with(".psso-registering"))
                {
                    continue;
                }
                let coordinate: Coordinate =
                    state::read_optional(root, &file)?.ok_or(Error::Storage)?;
                if coordinate.context.generated_uid.as_ref() != Some(&next.generated_uid)
                    || coordinate.context.installation != next.installation
                {
                    return Err(Error::Identity);
                }
                let retired = resolution(root, &coordinate)?;
                let observation: Option<NativeObservation> =
                    state::read_optional(root, &coordinate.file("observation")?)?;
                let native_done = native_cleared(
                    &coordinate,
                    file.ends_with(".psso-registering"),
                    observation.as_ref(),
                );
                if retired.is_some() && native_done {
                    clear_coordinate(root, &coordinate, file.ends_with(".psso-registering"))?;
                } else {
                    if retired.is_none() {
                        state::write(root, &coordinate.file("retire")?, &coordinate)?;
                    }
                    complete = false;
                }
            }
            if !complete {
                return Err(Error::Unavailable);
            }
        }
    }
    state::write(root, &name, next)
}

/// Validate all current native records before changing the installed extension.
pub(crate) fn upgrade_preflight(
    deployment: &crate::deployment::Deployment,
    has_extension: bool,
) -> Result<(), Error> {
    let root = crate::access_worker::root(&deployment.execution);
    if !root.try_exists()? {
        return Ok(());
    }
    for entry in std::fs::read_dir(&root)? {
        let name = entry?
            .file_name()
            .into_string()
            .map_err(|_| Error::Storage)?;
        if name.ends_with(".psso-binding") {
            let binding: ::platform_sso::LocalBinding =
                state::read_optional(&root, &name)?.ok_or(Error::Storage)?;
            binding.configuration.validate()?;
            if binding.enabled && !has_extension {
                return Err(Error::Unavailable);
            }
        } else if [
            ".psso-source",
            ".psso-registering",
            ".psso-retire",
            ".psso-retired",
        ]
        .iter()
        .any(|suffix| name.ends_with(suffix))
        {
            if !has_extension && !name.ends_with(".psso-retired") {
                return Err(Error::Unavailable);
            }
            let coordinate: Coordinate = if name.ends_with(".psso-retired") {
                state::read_optional::<Retired>(&root, &name)?.map(|r| r.coordinate)
            } else {
                state::read_optional(&root, &name)?
            }
            .ok_or(Error::Storage)?;
            coordinate.context.validate().map_err(|_| Error::Identity)?;
        }
    }
    Ok(())
}

fn configured_owner(
    deployment: &crate::deployment::Deployment,
) -> Result<(w::RegistrationTrust, Uuid, Uuid), Error> {
    let root = crate::access_worker::root(&deployment.execution);
    let mut retained = None;
    visit_live_sources(&root, |c| {
        let id = (
            c.context.authority,
            c.context.installation,
            c.context.tenant,
        );
        let matches = match id.0 {
            w::RegistrationTrust::DaDevicePop => deployment
                .modules
                .da
                .backend()
                .is_ok_and(|b| (b.instance, b.tenant) == (id.1, id.2)),
            w::RegistrationTrust::MdmBootstrapPop => deployment
                .mdm()
                .is_ok_and(|b| (b.instance, b.tenant) == (id.1, id.2)),
        };
        if !matches || retained.replace(id).is_some_and(|old| old != id) {
            return Err(Error::Conflict);
        }
        Ok(())
    })?;
    if let Some(id) = retained {
        return Ok(id);
    }
    if deployment.modules.da.enabled() {
        let da = deployment.modules.da.backend()?;
        Ok((w::RegistrationTrust::DaDevicePop, da.instance, da.tenant))
    } else {
        let mdm = deployment.mdm()?;
        Ok((
            w::RegistrationTrust::MdmBootstrapPop,
            mdm.instance,
            mdm.tenant,
        ))
    }
}

/// Keep the service and extension available until native and remote cleanup has completed.
pub(crate) fn uninstall_preflight(deployment: &crate::deployment::Deployment) -> Result<(), Error> {
    let root = crate::access_worker::root(&deployment.execution);
    if !root.exists() {
        return Ok(());
    }
    let _lock = state::lock(&root)?;
    state::write(
        &root,
        "platform-sso.disabled",
        &Disabled {
            installation: configured_owner(deployment)?.1,
        },
    )?;
    for entry in std::fs::read_dir(&root)? {
        let name = entry?
            .file_name()
            .into_string()
            .map_err(|_| Error::Storage)?;
        if let Some(id) = name.strip_suffix(".psso-result") {
            let receipt: CleanupReceipt =
                state::read_optional(&root, &name)?.ok_or(Error::Storage)?;
            if Uuid::parse_str(id).ok() != Some(receipt.operation) {
                return Err(Error::Identity);
            }
            crate::installation::preserve_psso_result(receipt.operation, &receipt)?;
        }
        if [".psso-source", ".psso-registering", ".psso-retire"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
        {
            return Err(Error::Unavailable);
        }
    }
    Ok(())
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
    if !["inspect", "enable", "disable", "retire", "cleanup"].contains(&action) {
        return Err(Error::Configuration);
    }
    deployment
        .service
        .verify(&std::env::current_exe()?)
        .map_err(|_| Error::Identity)?;
    let root = crate::access_worker::root(&deployment.execution);
    let (authority, instance, tenant) = configured_owner(deployment)?;
    let _lock = state::lock(&root)?;
    let mut coordinates = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let name = entry?
            .file_name()
            .into_string()
            .map_err(|_| Error::Storage)?;
        let in_flight = name.ends_with(".psso-registering");
        if !in_flight && !name.ends_with(".psso-source") {
            continue;
        }
        if coordinates.len() >= 256 {
            return Err(Error::Capacity);
        }
        let coordinate: Coordinate = state::read_optional(&root, &name)?.ok_or(Error::Storage)?;
        if coordinate.file(if in_flight { "registering" } else { "source" })? != name
            || (
                coordinate.context.authority,
                coordinate.context.installation,
                coordinate.context.tenant,
            ) != (authority, instance, tenant)
        {
            return Err(Error::Identity);
        }
        coordinates.push((coordinate, in_flight));
    }
    if action == "enable" {
        if !coordinates.is_empty() {
            return Err(Error::Unavailable);
        }
        deployment
            .platform_sso
            .as_ref()
            .ok_or(Error::Unsupported)?
            .verify()?;
        state::remove(&root, "platform-sso.disabled")?;
    }
    if action != "inspect" && action != "enable" {
        state::write(
            &root,
            "platform-sso.disabled",
            &Disabled {
                installation: instance,
            },
        )?;
    }
    if action == "retire" {
        for (coordinate, _) in &coordinates {
            state::write(&root, &coordinate.file("retire")?, coordinate)?;
        }
    }
    let mut observations = Vec::new();
    let mut complete = true;
    for (coordinate, in_flight) in &coordinates {
        let retired = resolution(&root, coordinate)?;
        let native: Option<NativeObservation> =
            state::read_optional(&root, &coordinate.file("observation")?)?;
        let remote_done = retired.is_some();
        // An in-flight record never authorized saving an Apple login configuration.
        let native_done = native_cleared(coordinate, *in_flight, native.as_ref());
        complete &= remote_done && native_done;
        observations.push(cleanup_receipt(
            coordinate,
            *in_flight,
            retired,
            native_done,
        ));
    }
    #[cfg(target_os = "macos")]
    let accounts = if action == "inspect" {
        let mut facts = Vec::new();
        for entry in std::fs::read_dir(&root)? {
            let name = entry?
                .file_name()
                .into_string()
                .map_err(|_| Error::Storage)?;
            if !name.ends_with(".psso-binding") {
                continue;
            }
            let binding: ::platform_sso::LocalBinding =
                state::read_optional(&root, &name)?.ok_or(Error::Storage)?;
            if binding_name(&binding.generated_uid)? != name
                || (binding.authority, binding.installation, binding.tenant)
                    != (authority, instance, tenant)
            {
                return Err(Error::Identity);
            }
            if facts.len() >= 256 {
                return Err(Error::Capacity);
            }
            let native=crate::access_worker::macos::inspect(&binding.generated_uid,false).ok().map(|a|{
                let mut facts=crate::access_worker::macos::facts(&a);
                facts.platform_sso=native_facts(&root,&binding).ok().flatten();
                serde_json::json!({"role":if a.administrator {"administrator"}else{"standard"},"facts":facts})
            });
            facts.push(serde_json::json!({"binding":binding.binding,"revision":binding.revision,"generatedUid":binding.generated_uid,"bindingGeneration":binding.binding_generation,"observation":native}));
        }
        Some(facts)
    } else {
        None
    };
    #[cfg(not(target_os = "macos"))]
    let accounts = None::<()>;
    let custody = if deployment.modules.mdm.enabled() {
        deployment
            .network()
            .and_then(|cfg| {
                agent_client::filevault::inspect(&deployment.state_root.join("communication"), &cfg)
            })
            .unwrap_or(execution_contract::FilevaultState::Unavailable)
    } else {
        execution_contract::FilevaultState::Unavailable
    };
    if action == "cleanup" && complete {
        for (coordinate, in_flight) in &coordinates {
            clear_coordinate(&root, coordinate, *in_flight)?;
            if !in_flight {
                state::remove(&root, &coordinate.file("observation")?)?;
            }
        }
    }
    let disabled = match enabled(&root, instance) {
        Ok(()) => false,
        Err(Error::Denied) => true,
        Err(e) => return Err(e),
    };
    println!(
        "{}",
        serde_json::json!({"componentValid": deployment.platform_sso.as_ref().is_some_and(|c| c.verify().is_ok()),
        "disabled": disabled, "sources": observations,"accounts":accounts,"filevaultCustody":custody,
        "cleanupComplete": action == "cleanup" && complete})
    );
    if action == "cleanup" && !complete {
        return Err(Error::Unavailable);
    }
    Ok(())
}

fn finish_retirement(
    root: &std::path::Path,
    coordinate: &Coordinate,
    result: RemoteRetirement,
) -> Result<(), Error> {
    let _lock = state::lock(root)?;
    let pending: Option<Coordinate> = state::read_optional(root, &coordinate.file("retire")?)?;
    if pending.as_ref() == Some(coordinate) {
        state::write(
            root,
            &coordinate.file("retired")?,
            &Retired {
                coordinate: coordinate.clone(),
                result,
            },
        )?;
        state::remove(root, &coordinate.file("retire")?)?;
    }
    Ok(())
}

// Only an authenticated native peer's current context reaches here. With no local record,
// Status is still scoped to that context; retirement never revokes a different returned source.
fn queue_retirement(
    root: &std::path::Path,
    mut coordinate: Coordinate,
) -> Result<Coordinate, Error> {
    let _lock = state::lock(root)?;
    for suffix in ["registering", "source"] {
        if let Some(retained) = state::read_optional::<Coordinate>(root, &coordinate.file(suffix)?)?
        {
            if retained.source == coordinate.source
                && retained.source_version == coordinate.source_version
            {
                if !retained.same_owner(&coordinate.context) {
                    return Err(Error::Identity);
                }
                coordinate = retained;
                break;
            }
        }
    }
    state::write(root, &coordinate.file("retire")?, &coordinate)?;
    Ok(coordinate)
}
fn settle_rejected_registration(
    root: &std::path::Path,
    coordinate: &Coordinate,
) -> Result<(), Error> {
    let _lock = state::lock(root)?;
    if state::read_optional::<Coordinate>(root, &coordinate.file("registering")?)?.as_ref()
        == Some(coordinate)
    {
        state::remove(root, &coordinate.file("registering")?)?;
    }
    Ok(())
}

fn registration_unresolved(
    root: &std::path::Path,
    coordinate: &Coordinate,
    source: Option<&w::SourceStatus>,
) -> Result<bool, Error> {
    let pending: Option<Coordinate> = state::read_optional(root, &coordinate.file("registering")?)?;
    Ok(pending.as_ref() == Some(coordinate)
        && source
            .is_none_or(|s| s.id == coordinate.source && s.version < coordinate.source_version))
}

fn retirement_requested(root: &std::path::Path, coordinate: &Coordinate) -> Result<bool, Error> {
    Ok(
        state::read_optional::<Coordinate>(root, &coordinate.file("retire")?)?.as_ref()
            == Some(coordinate)
            || resolution(root, coordinate)?.is_some(),
    )
}

fn retain_registration(root: &std::path::Path, coordinate: &Coordinate) -> Result<bool, Error> {
    let _lock = state::lock(root)?;
    match enabled(root, coordinate.context.installation).and_then(|()| {
        if retirement_requested(root, coordinate)? {
            Err(Error::Denied)
        } else {
            Ok(())
        }
    }) {
        Ok(()) => {
            require_owner(
                root,
                (
                    coordinate.context.authority,
                    coordinate.context.installation,
                    coordinate.context.tenant,
                    coordinate.context.device_id,
                ),
            )?;
            state::write(root, &coordinate.file("source")?, coordinate)?;
            state::remove(root, &coordinate.file("registering")?)?;
            Ok(true)
        }
        Err(Error::Denied) => {
            // Keep the unknown/late registration out of native configuration and queue exact cleanup.
            state::write(root, &coordinate.file("registering")?, coordinate)?;
            state::write(root, &coordinate.file("retire")?, coordinate)?;
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn coordinate() -> Coordinate {
        Coordinate {
            operation: Uuid::new_v4(),
            client_id: Uuid::from_u128(7),
            authentication: w::AuthenticationMode::SecureEnclave,
            callback_generation: 1,
            context: ::platform_sso::Context {
                authority: rss_mdm_agent_wire::platform_sso::RegistrationTrust::MdmBootstrapPop,
                tenant: Uuid::from_u128(1),
                device_id: Uuid::from_u128(2).try_into().unwrap(),
                installation: Uuid::from_u128(3),
                origin: "https://mdm.example".into(),
                kind: w::Kind::User,
                binding: Some(Uuid::from_u128(4)),
                binding_generation: Some(1),
                generated_uid: Some(Uuid::from_u128(5).to_string()),
                binding_revision: 1,
                configuration: Some(w::LoginConfiguration::default()),
            },
            source: Uuid::from_u128(6),
            source_version: 1,
        }
    }
    fn binding() -> ::platform_sso::LocalBinding {
        let c = coordinate().context;
        ::platform_sso::LocalBinding {
            authority: rss_mdm_agent_wire::platform_sso::RegistrationTrust::MdmBootstrapPop,
            installation: c.installation,
            tenant: c.tenant,
            device_id: c.device_id,
            binding: c.binding.unwrap(),
            binding_generation: 1,
            revision: 1,
            generated_uid: c.generated_uid.unwrap(),
            enabled: true,
            configuration: w::LoginConfiguration::default(),
        }
    }
    #[test]
    fn da_source_excludes_mdm_and_cleanup_is_queryable_after_restart() {
        let root = std::env::temp_dir().join(format!("psso-da-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let mut c = coordinate();
        c.context.authority = w::RegistrationTrust::DaDevicePop;
        state::write(&root, &c.file("registering").unwrap(), &c).unwrap();
        let id = (
            c.context.authority,
            c.context.installation,
            c.context.tenant,
            c.context.device_id,
        );
        assert!(require_owner(&root, id).is_ok());
        assert_eq!(
            require_owner(
                &root,
                (w::RegistrationTrust::MdmBootstrapPop, id.1, id.2, id.3)
            ),
            Err(Error::Conflict)
        );
        assert!(!sweep_retirement(&root, id.1).unwrap());
        finish_retirement(&root, &c, RemoteRetirement::Revoked).unwrap();
        assert!(sweep_retirement(&root, id.1).unwrap());
        let receipt: CleanupReceipt =
            state::read_optional(&root, &format!("{}.psso-result", c.operation))
                .unwrap()
                .unwrap();
        assert_eq!(
            receipt,
            cleanup_receipt(&c, true, Some(RemoteRetirement::Revoked), true)
        );
        assert!(receipt.native_cleanup_confirmed.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn mode_switch_waits_for_both_retirement_and_native_cleanup_without_touching_other_users() {
        let root = std::env::temp_dir().join(format!("psso-mode-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let old = binding();
        save_binding(&root, &old).unwrap();
        let c = coordinate();
        state::write(&root, &c.file("source").unwrap(), &c).unwrap();
        let mut other = c.clone();
        other.context.generated_uid = Some(Uuid::new_v4().to_string());
        state::write(&root, &other.file("source").unwrap(), &other).unwrap();
        let mut next = old.clone();
        next.revision += 1;
        next.configuration.authentication = w::AuthenticationMode::Password;
        next.configuration.password_provider = Some(Uuid::new_v4());
        assert_eq!(save_binding(&root, &next), Err(Error::Unavailable));
        assert!(
            state::read_optional::<Coordinate>(&root, &c.file("retire").unwrap())
                .unwrap()
                .is_some()
        );
        assert!(
            state::read_optional::<Coordinate>(&root, &other.file("retire").unwrap())
                .unwrap()
                .is_none()
        );
        finish_retirement(&root, &c, RemoteRetirement::Revoked).unwrap();
        assert_eq!(save_binding(&root, &next), Err(Error::Unavailable));
        state::write(
            &root,
            &c.file("observation").unwrap(),
            &serde_json::json!({
                "source": c.source, "sourceVersion": c.source_version, "bindingGeneration": 1,
                "callbackGeneration": 1, "subject": "501", "session": 1, "sessionBinding": null, "configuration": null,
                "observedAt": 10, "deviceRegistered": true, "userRegistered": false
            }),
        )
        .unwrap();
        save_binding(&root, &next).unwrap();
        let stored: ::platform_sso::LocalBinding =
            state::read_optional(&root, &binding_name(&next.generated_uid).unwrap())
                .unwrap()
                .unwrap();
        assert_eq!(stored.configuration, next.configuration);
        assert!(
            state::read_optional::<Retired>(&root, &c.file("retired").unwrap())
                .unwrap()
                .is_some()
        );
        assert!(
            state::read_optional::<Coordinate>(&root, &other.file("source").unwrap())
                .unwrap()
                .is_some()
        );
        assert_eq!(save_binding(&root, &old), Err(Error::Identity));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn revision_only_updates_preserve_observations_but_not_registration_authority() {
        let old = coordinate().context;
        let mut current = old.clone();
        current.binding_revision += 1;
        assert!(observation_owner(&old, &current));
        assert!(old != current);
        current.binding_generation = Some(2);
        assert!(!observation_owner(&old, &current));
        current = old.clone();
        current
            .configuration
            .as_mut()
            .unwrap()
            .maintenance_accounts
            .push("recovery".into());
        assert!(!observation_owner(&old, &current));
    }
    #[test]
    fn retirement_drain_preserves_native_cleanup_and_never_reopens_registration() {
        let root = std::env::temp_dir().join(format!("psso-drain-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let c = coordinate();
        state::write(&root, &c.file("source").unwrap(), &c).unwrap();
        assert!(!sweep_retirement(&root, c.context.installation).unwrap());
        assert_eq!(enabled(&root, c.context.installation), Err(Error::Denied));
        finish_retirement(&root, &c, RemoteRetirement::Revoked).unwrap();
        assert!(!sweep_retirement(&root, c.context.installation).unwrap());
        state::write(
            &root,
            &c.file("observation").unwrap(),
            &serde_json::json!({
                "source": c.source, "sourceVersion": c.source_version, "bindingGeneration": 1,
                "callbackGeneration": 1, "subject": "501", "session": 1, "sessionBinding": null, "configuration": null,
                "observedAt": 10, "deviceRegistered": true, "userRegistered": false
            }),
        )
        .unwrap();
        assert!(sweep_retirement(&root, c.context.installation).unwrap());
        assert!(
            state::read_optional::<Retired>(&root, &c.file("retired").unwrap())
                .unwrap()
                .is_some()
        );
        assert!(!cleanup_operation(Some(&Request::PlatformSso {
            input: Operation::Bootstrap {
                callback_generation: 1,
                scope: w::Kind::Device,
                authentication: w::AuthenticationMode::Password
            }
        })));
        assert!(cleanup_operation(Some(&Request::PlatformSso {
            input: Operation::Unregister {
                scope: w::Kind::User,
                source: c.source,
                source_version: 1
            }
        })));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn configuration_removal_fences_first_registration_and_only_its_version() {
        let root = std::env::temp_dir().join(format!("psso-remove-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let coordinate = coordinate();
        // Removal can arrive before Register, with no saved source or registering record.
        let retiring = queue_retirement(&root, coordinate.clone()).unwrap();
        assert!(retirement_requested(&root, &coordinate).unwrap());
        assert!(!retain_registration(&root, &coordinate).unwrap());
        assert!(
            state::read_optional::<Coordinate>(&root, &coordinate.file("source").unwrap())
                .unwrap()
                .is_none()
        );
        finish_retirement(&root, &retiring, RemoteRetirement::Revoked).unwrap();
        assert!(retirement_requested(&root, &coordinate).unwrap());
        let mut newer = coordinate.clone();
        newer.source_version += 1;
        assert!(!retirement_requested(&root, &newer).unwrap());
        assert!(retain_registration(&root, &newer).unwrap());
        // Repeating old removal never deletes or fences the newer source.
        queue_retirement(&root, coordinate.clone()).unwrap();
        assert!(retain_registration(&root, &newer).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn definite_rejection_settles_only_the_exact_inflight_request() {
        let root = std::env::temp_dir().join(format!("psso-rejected-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let coordinate = coordinate();
        let mut newer = coordinate.clone();
        newer.source_version += 1;
        for c in [&coordinate, &newer] {
            state::write(&root, &c.file("registering").unwrap(), c).unwrap();
        }
        settle_rejected_registration(&root, &coordinate).unwrap();
        assert!(!registration_unresolved(&root, &coordinate, None).unwrap());
        assert!(registration_unresolved(&root, &newer, None).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_source_does_not_settle_an_unknown_inflight_registration() {
        let root = std::env::temp_dir().join(format!("psso-unknown-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let coordinate = coordinate();
        assert!(!registration_unresolved(&root, &coordinate, None).unwrap());
        state::write(&root, &coordinate.file("registering").unwrap(), &coordinate).unwrap();
        assert!(registration_unresolved(&root, &coordinate, None).unwrap());
        let key = w::PublicKey {
            kty: "EC".into(),
            crv: "P-256".into(),
            x: String::new(),
            y: String::new(),
        };
        let current = w::SourceStatus {
            id: coordinate.source,
            version: coordinate.source_version,
            revoked: false,
            signing_key: Some(key.clone()),
            encryption_key: key,
        };
        assert!(!registration_unresolved(&root, &coordinate, Some(&current)).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn late_registration_after_disable_is_retired_without_authorizing_native_configuration() {
        let root = std::env::temp_dir().join(format!("psso-late-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let coordinate = coordinate();
        state::write(&root, &coordinate.file("registering").unwrap(), &coordinate).unwrap();
        state::write(
            &root,
            "platform-sso.disabled",
            &Disabled {
                installation: coordinate.context.installation,
            },
        )
        .unwrap();
        assert!(!retain_registration(&root, &coordinate).unwrap());
        assert!(
            state::read_optional::<Coordinate>(&root, &coordinate.file("source").unwrap())
                .unwrap()
                .is_none()
        );
        let pending: Coordinate = state::read_optional(&root, &coordinate.file("retire").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(pending.source_version, coordinate.source_version);
        finish_retirement(&root, &coordinate, RemoteRetirement::Revoked).unwrap();
        assert!(
            state::read_optional::<Retired>(&root, &coordinate.file("retired").unwrap())
                .unwrap()
                .is_some()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn lost_retirement_reply_cannot_clear_a_new_persisted_request() {
        let root = std::env::temp_dir().join(format!("psso-retirement-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let old = coordinate();
        let mut replacement = old.clone();
        replacement.source_version = 2;
        state::write(&root, &replacement.file("retire").unwrap(), &replacement).unwrap();
        finish_retirement(&root, &old, RemoteRetirement::Revoked).unwrap();
        let pending: Coordinate = state::read_optional(&root, &replacement.file("retire").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(pending.source_version, 2);
        assert!(
            state::read_optional::<Retired>(&root, &old.file("retired").unwrap())
                .unwrap()
                .is_none()
        );
        finish_retirement(&root, &replacement, RemoteRetirement::Revoked).unwrap();
        assert!(
            state::read_optional::<Coordinate>(&root, &old.file("retire").unwrap())
                .unwrap()
                .is_none()
        );
        let completed: Retired = state::read_optional(&root, &replacement.file("retired").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(completed.coordinate.source_version, 2);
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
        current.device_id = rss_mdm_agent_wire::DeviceId::new_v4();
        assert!(!old.same_owner(&current));
        current = old.context.clone();
        current.binding_generation = Some(0);
        assert!(!old.same_owner(&current));
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "platform_sso_completion.rs"]
mod completion_tests;
