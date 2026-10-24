//! Native ABI adapter. Private key handles stay in Objective-C/Apple Security.
use execution_ipc::{
    host::{Reply, Request, ServiceClient},
    platform_sso::{Operation, Response},
};
use platform_sso::Registration;
use rss_mdm_agent_wire::platform_sso as w;
use std::{
    ffi::{c_char, c_void, CStr, CString},
    panic::{catch_unwind, AssertUnwindSafe},
};
use uuid::Uuid;

pub struct Handle {
    client: ServiceClient,
    challenge: w::BootstrapResponse,
    registration: Registration,
    fields: Vec<CString>,
}
type Sign = unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut u8) -> i32;
fn now() -> Result<i64, ()> {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ())?
            .as_secs(),
    )
    .map_err(|_| ())
}
fn request(client: &ServiceClient, operation: Operation) -> Result<Response, ()> {
    match client
        .request(Request::PlatformSso { input: operation })
        .map_err(|_| ())?
    {
        Reply::PlatformSso { value } => Ok(value),
        _ => Err(()),
    }
}
fn begin(scope: u32, signing: &[u8], encryption: &[u8]) -> Result<Handle, i32> {
    let kind = match scope {
        0 => w::Kind::Device,
        1 => w::Kind::User,
        _ => return Err(-1),
    };
    let client = ServiceClient::installed().map_err(|_| -1)?;
    let (context, challenge) =
        match request(&client, Operation::Bootstrap { scope: kind }).map_err(|_| -1)? {
            Response::Bootstrap { context, challenge } => (context, challenge),
            Response::NeedsAccount {} => return Err(1),
            _ => return Err(-1),
        };
    let registration = Registration::prepare(
        context,
        &challenge,
        platform_sso::public_key(signing).map_err(|_| -1)?,
        platform_sso::public_key(encryption).map_err(|_| -1)?,
        now().map_err(|_| -1)?,
    )
    .map_err(|_| -1)?;
    let metadata = serde_json::to_string(&registration).map_err(|_| -1)?;
    let fields = [
        challenge.client_id.to_string(),
        challenge.endpoints.issuer.clone(),
        challenge.endpoints.issuer.clone(),
        challenge.endpoints.nonce.clone(),
        challenge.endpoints.token.clone(),
        challenge.endpoints.jwks.clone(),
        challenge.login_user_name.clone(),
        metadata,
        w::LOGIN_TYPE.into(),
        w::ASSERTION_TYPE.into(),
    ]
    .into_iter()
    .map(CString::new)
    .collect::<Result<_, _>>()
    .map_err(|_| -1)?;
    Ok(Handle {
        client,
        challenge,
        registration,
        fields,
    })
}
/// # Safety
/// Both points reference 65 readable bytes; status references one writable i32 for this call.
#[no_mangle]
pub unsafe extern "C" fn rss_psso_begin(
    scope: u32,
    signing: *const u8,
    encryption: *const u8,
    status: *mut i32,
) -> *mut Handle {
    if status.is_null() {
        return std::ptr::null_mut();
    }
    *status = -1;
    if signing.is_null() || encryption.is_null() {
        return std::ptr::null_mut();
    }
    match catch_unwind(AssertUnwindSafe(|| {
        begin(
            scope,
            std::slice::from_raw_parts(signing, 65),
            std::slice::from_raw_parts(encryption, 65),
        )
    })) {
        Ok(Ok(handle)) => {
            *status = 0;
            Box::into_raw(Box::new(handle))
        }
        Ok(Err(code)) => {
            *status = code;
            std::ptr::null_mut()
        }
        Err(_) => std::ptr::null_mut(),
    }
}
/// # Safety
/// Handle is live and exclusively owned by this registration callback; signer writes at most 64 bytes.
#[no_mangle]
pub unsafe extern "C" fn rss_psso_register(
    handle: *mut Handle,
    sign: Option<Sign>,
    context: *mut c_void,
) -> i32 {
    if handle.is_null() || sign.is_none() {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let handle = &*handle;
        let input = handle.registration.proof(&handle.challenge, now()?).map_err(|_| ())?;
        let mut signature = [0u8;64];
        if sign.ok_or(())?(context,input.as_ptr(),input.len(),signature.as_mut_ptr()) != 0 { return Err(()); }
        let proof = platform_sso::signed(&input,&signature).map_err(|_| ())?;
        let result = request(&handle.client, Operation::Register { input: Box::new(w::Registration {
            operation_id: Uuid::new_v4(), challenge_id: handle.challenge.id,
            signing_key: Some(handle.registration.signing_key.clone()), encryption_key: handle.registration.encryption_key.clone(), proof,
        })});
        if matches!(result, Ok(Response::Registered { ref receipt }) if handle.registration.accepts(receipt)) { return Ok(()); }
        // A transport error is an unknown outcome. Never replay a consumed registration challenge.
        match request(&handle.client, Operation::Status { scope: handle.registration.context.kind })? {
            Response::Status { value } if handle.registration.recovered(&value) => Ok(()), _ => Err(()),
        }
    })).ok().and_then(Result::ok).map_or(-1, |_| 0)
}
/// # Safety
/// Handle must remain alive until the native caller copies the returned UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn rss_psso_field(handle: *const Handle, index: u32) -> *const c_char {
    if handle.is_null() {
        return std::ptr::null();
    }
    (&*handle)
        .fields
        .get(index as usize)
        .map_or(std::ptr::null(), |s| s.as_ptr())
}
/// # Safety
/// Free exactly once, after all asynchronous uses have completed.
#[no_mangle]
pub unsafe extern "C" fn rss_psso_free(handle: *mut Handle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}
unsafe fn text<'a>(value: *const c_char) -> Result<&'a str, ()> {
    if value.is_null() {
        return Err(());
    }
    let value = CStr::from_ptr(value).to_str().map_err(|_| ())?;
    if value.len() > 65536 {
        return Err(());
    }
    Ok(value)
}
/// # Safety
/// All strings are native-owned NUL-terminated UTF-8, live for this call; signer writes 64 bytes.
#[no_mangle]
pub unsafe extern "C" fn rss_psso_browser(
    metadata: *const c_char,
    issuer: *const c_char,
    url: *const c_char,
    body: *const c_char,
    token: *const c_char,
    sign: Option<Sign>,
    context: *mut c_void,
) -> *mut c_char {
    catch_unwind(AssertUnwindSafe(|| -> Result<CString, ()> {
        let registration: Registration = serde_json::from_str(text(metadata)?).map_err(|_| ())?;
        let endpoints =
            w::Endpoints::new(&registration.context.origin, registration.context.tenant)
                .map_err(|_| ())?;
        if endpoints.issuer != text(issuer)? {
            return Err(());
        }
        let mut body: w::BrowserComplete = serde_json::from_str(text(body)?).map_err(|_| ())?;
        if body.challenge_id.is_nil() || !body.proof.is_empty() {
            return Err(());
        }
        let input = registration
            .browser_input(text(url)?, &body.nonce, text(token)?, now()?)
            .map_err(|_| ())?;
        let mut signature = [0u8; 64];
        if sign.ok_or(())?(context, input.as_ptr(), input.len(), signature.as_mut_ptr()) != 0 {
            return Err(());
        }
        body.proof = platform_sso::signed(&input, &signature).map_err(|_| ())?;
        CString::new(serde_json::to_string(&body).map_err(|_| ())?).map_err(|_| ())
    }))
    .ok()
    .and_then(Result::ok)
    .map_or(std::ptr::null_mut(), CString::into_raw)
}
/// # Safety
/// Pointer was returned by rss_psso_browser and has not been freed.
#[no_mangle]
pub unsafe extern "C" fn rss_psso_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}
/// # Safety
/// Metadata is native-owned NUL-terminated public registration data.
#[no_mangle]
pub unsafe extern "C" fn rss_psso_observe(
    metadata: *const c_char,
    device: bool,
    user: bool,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| -> Result<(), ()> {
        let registration: Registration = serde_json::from_str(text(metadata)?).map_err(|_| ())?;
        let client = ServiceClient::installed().map_err(|_| ())?;
        request(
            &client,
            Operation::Observe {
                scope: registration.context.kind,
                source: registration.source_id,
                source_version: registration.source_version,
                device_registered: device,
                user_registered: user,
            },
        )?;
        Ok(())
    }))
    .ok()
    .and_then(Result::ok)
    .map_or(-1, |_| 0)
}
/// Revoke only the calling native user's source. Returns unknown when remote completion is unavailable.
/// # Safety
/// Metadata is a native-owned NUL-terminated string, live for this call.
#[no_mangle]
pub unsafe extern "C" fn rss_psso_unregister(metadata: *const c_char) -> i32 {
    catch_unwind(AssertUnwindSafe(|| -> Result<(), ()> {
        let client = ServiceClient::installed().map_err(|_| ())?;
        let registration: Registration = serde_json::from_str(text(metadata)?).map_err(|_| ())?;
        match request(
            &client,
            Operation::Unregister {
                scope: registration.context.kind,
                source: registration.source_id,
                source_version: registration.source_version,
            },
        )? {
            Response::Revoked { receipt } if receipt.revoked => Ok(()),
            Response::Retired {} => Ok(()),
            _ => Err(()),
        }
    }))
    .ok()
    .and_then(Result::ok)
    .map_or(-1, |_| 0)
}
