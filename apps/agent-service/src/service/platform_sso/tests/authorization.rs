//! Exercise the registration engine across a change in the deployed policy during HTTPS.
use super::*;

struct PolicyOwner<'a> {
    root: &'a Path,
    state: State,
    coordinate: Coordinate,
    selected: Option<w::AuthenticationMode>,
    withdraw_on_bootstrap: bool,
    switch_on_register: bool,
}
impl Owner for PolicyOwner<'_> {
    fn state(&mut self) -> &mut State {
        &mut self.state
    }
    fn root(&self) -> &Path {
        self.root
    }
    fn identity(&self) -> Result<Identity, Error> {
        let c = &self.coordinate.context;
        Ok((c.authority, c.installation, c.tenant, c.device_id))
    }
    async fn context(
        &self,
        _: &Command,
        kind: w::Kind,
        _: bool,
        mode: Option<w::AuthenticationMode>,
    ) -> Result<::platform_sso::Context, Error> {
        if self.selected.is_none()
            || mode.is_some() && self.selected != mode
            || kind != self.coordinate.context.kind
        {
            return Err(Error::Untrusted);
        }
        let mut context = self.coordinate.context.clone();
        if let Some(account) = context.generated_uid.clone() {
            apply_account_binding(self.root, &mut context, &account, true)?;
        }
        Ok(context)
    }
    async fn bootstrap(&mut self, _: &w::Bootstrap) -> Result<w::BootstrapResponse, Error> {
        if self.withdraw_on_bootstrap {
            self.selected = None;
        }
        let c = &self.coordinate;
        Ok(w::BootstrapResponse {
            id: Uuid::new_v4(),
            nonce: "fixture".into(),
            expires_at: (crate::wall()? / 1000) as i64 + 30,
            source_id: c.source,
            source_version: c.source_version,
            endpoints: w::Endpoints::new(&c.context.origin, c.context.tenant)?,
            client_id: c.client_id,
            login_user_name: c.source.to_string(),
        })
    }
    async fn register(
        &mut self,
        _: &w::Registration,
    ) -> Result<Option<w::RegistrationReceipt>, Error> {
        if self.switch_on_register {
            self.selected = Some(w::AuthenticationMode::Password);
        }
        let c = &self.coordinate;
        Ok(Some(w::RegistrationReceipt {
            source_id: c.source,
            source_version: c.source_version,
            kind: c.context.kind,
            registered: true,
            hardware_attested: false,
            registration_trust: c.context.authority,
        }))
    }
    async fn revoke(&mut self, _: Uuid, _: i64) -> Result<w::RevocationReceipt, Error> {
        unreachable!()
    }
    async fn status(&mut self, _: Option<Uuid>) -> Result<w::Status, Error> {
        unreachable!()
    }
}

fn command(input: Operation) -> Command {
    let (reply, _rx) = tokio::sync::oneshot::channel();
    Command {
        request: LocalRequest::operation(Request::PlatformSso { input }),
        subject: "501".into(),
        session: 1,
        binding: None,
        reply,
        fence_epoch: 0,
    }
}

#[cfg(target_os = "macos")]
pub(in crate::service::platform_sso) async fn register_saved_account(
    root: &Path,
    b: &::platform_sso::LocalBinding,
) {
    let mode = b.configuration.authentication;
    let mut c = coordinate();
    c.context.authority = b.authority;
    c.context.installation = b.installation;
    c.context.device_id = b.device_id;
    c.context.tenant = b.tenant;
    c.context.generated_uid = Some(b.generated_uid.clone());
    c.authentication = mode;
    let mut owner = PolicyOwner {
        root,
        state: State::default(),
        coordinate: c.clone(),
        selected: Some(mode),
        withdraw_on_bootstrap: false,
        switch_on_register: false,
    };
    let begin = command(Operation::Bootstrap {
        scope: w::Kind::User,
        authentication: mode,
        callback_generation: c.callback_generation,
    });
    let Response::Bootstrap { context, challenge } = owner.platform_sso(&begin).await.unwrap()
    else {
        panic!("missing user challenge")
    };
    assert_eq!(context.binding, Some(b.binding));
    assert_eq!(context.binding_generation, Some(b.binding_generation));
    let key = w::PublicKey {
        kty: "EC".into(),
        crv: "P-256".into(),
        x: "fixture".into(),
        y: "fixture".into(),
    };
    let req = command(Operation::Register {
        input: Box::new(w::Registration {
            operation_id: c.operation,
            challenge_id: challenge.id,
            signing_key: (mode == w::AuthenticationMode::SecureEnclave).then(|| key.clone()),
            encryption_key: key,
            proof: "fixture".into(),
        }),
    });
    assert!(matches!(
        owner.platform_sso(&req).await,
        Ok(Response::Registered { .. })
    ));
    let current = state::read_optional::<Coordinate>(root, &c.file("source").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(current.authentication, mode);
    assert!(current.context == context);
}
#[tokio::test]
async fn device_policy_is_checked_at_submission_and_after_both_remote_responses() {
    let root = crate::protocol_test_support::Root::new();
    let mut c = coordinate();
    c.context.authority = w::RegistrationTrust::DaDevicePop;
    c.context.kind = w::Kind::Device;
    c.context.generated_uid = None;
    c.context.binding = None;
    c.context.binding_generation = None;
    c.context.binding_revision = 0;
    c.context.configuration = None;
    let mut owner = PolicyOwner {
        root: &root.path,
        state: State::default(),
        coordinate: c.clone(),
        selected: Some(c.authentication),
        withdraw_on_bootstrap: true,
        switch_on_register: true,
    };
    let bootstrap = |authentication| {
        command(Operation::Bootstrap {
            scope: w::Kind::Device,
            callback_generation: c.callback_generation,
            authentication,
        })
    };
    assert!(owner
        .platform_sso(&bootstrap(w::AuthenticationMode::Password))
        .await
        .is_err());
    assert!(owner
        .platform_sso(&bootstrap(c.authentication))
        .await
        .is_err());
    assert!(owner.state.pending.is_empty());
    owner.selected = Some(c.authentication);
    owner.withdraw_on_bootstrap = false;
    let Response::Bootstrap { challenge, .. } = owner
        .platform_sso(&bootstrap(c.authentication))
        .await
        .unwrap()
    else {
        panic!("missing challenge")
    };
    let key = w::PublicKey {
        kty: "EC".into(),
        crv: "P-256".into(),
        x: "fixture".into(),
        y: "fixture".into(),
    };
    let register = command(Operation::Register {
        input: Box::new(w::Registration {
            operation_id: c.operation,
            challenge_id: challenge.id,
            signing_key: Some(key.clone()),
            encryption_key: key,
            proof: "fixture".into(),
        }),
    });
    assert!(matches!(
        owner.platform_sso(&register).await,
        Err(Error::Identity)
    ));
    assert!(
        state::read_optional::<Coordinate>(&root.path, &c.file("source").unwrap())
            .unwrap()
            .is_none()
    );
    assert!(
        state::read_optional::<Coordinate>(&root.path, &c.file("retire").unwrap()).unwrap()
            == Some(c)
    );
}
