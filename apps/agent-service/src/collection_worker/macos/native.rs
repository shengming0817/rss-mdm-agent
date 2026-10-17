//! Private bounded FFI bridge. Native objects never escape the query process.
use super::*;
use std::ffi::c_void;
type Emit = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
unsafe extern "C" {
    fn rss_macos_accounts(
        section: u32,
        selection: *const u8,
        length: usize,
        maximum: usize,
        emit: Emit,
        state: *mut c_void,
    ) -> i32;
    fn rss_macos_certificates(
        path: *const libc::c_char,
        maximum: usize,
        emit: Emit,
        state: *mut c_void,
    ) -> i32;
    fn rss_macos_app_paths(bundle: *const libc::c_char, emit: Emit, state: *mut c_void) -> i32;
}
struct Visitor<'a> {
    context: &'a Context<'a>,
    visit: &'a mut dyn FnMut(&[u8]) -> Result<()>,
    error: Option<Fault>,
}
unsafe extern "C" fn emit(state: *mut c_void, data: *const u8, length: usize) -> i32 {
    // SAFETY: native owner calls synchronously with a live Visitor and byte slice.
    let visitor = unsafe { &mut *state.cast::<Visitor<'_>>() };
    let result = visitor.context.consume(length).and_then(|()| {
        if data.is_null() {
            return Err(Status::Invalid.into());
        }
        (visitor.visit)(unsafe { std::slice::from_raw_parts(data, length) })
    });
    match result {
        Ok(()) => 0,
        Err(e) => {
            visitor.error = Some(e);
            -1
        }
    }
}
fn invoke(
    c: &Context<'_>,
    mut visit: impl FnMut(&[u8]) -> Result<()>,
    call: impl FnOnce(Emit, *mut c_void) -> i32,
) -> Result<()> {
    c.check()?;
    super::super::macos_supervisor::begin_read().map_err(|_| Fault::Unclosed)?;
    let mut visitor = Visitor {
        context: c,
        visit: &mut visit,
        error: None,
    };
    let code = call(emit, (&mut visitor as *mut Visitor<'_>).cast());
    super::super::macos_supervisor::end_read().map_err(|_| Fault::Unclosed)?;
    if let Some(error) = visitor.error {
        return Err(error);
    }
    c.check()?;
    match code {
        0 => Ok(()),
        1 => Err(Status::Partial.into()),
        2 => Err(Status::Denied.into()),
        3 => Err(Status::StoreUnavailable.into()),
        4 => Err(Status::Truncated.into()),
        _ => Err(Status::Failed.into()),
    }
}
pub(super) fn accounts(
    c: &Context<'_>,
    section: u32,
    selection: &w::AccountSelection,
    maximum: usize,
    visit: impl FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    let selected: Vec<&str> = match selection {
        w::AccountSelection::Local => vec![],
        w::AccountSelection::Selected { identities } => {
            identities.iter().map(String::as_str).collect()
        }
    };
    let bytes = serde_json::to_vec(&selected).map_err(|_| Status::Invalid)?;
    invoke(c, visit, |emit, state| unsafe {
        rss_macos_accounts(section, bytes.as_ptr(), bytes.len(), maximum, emit, state)
    })
}
pub(super) fn certificates(
    c: &Context<'_>,
    path: &Path,
    maximum: usize,
    visit: impl FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| Status::Invalid)?;
    invoke(c, visit, |emit, state| unsafe {
        rss_macos_certificates(path.as_ptr(), maximum, emit, state)
    })
}
pub(super) fn app_paths(
    c: &Context<'_>,
    bundle: &str,
    visit: impl FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    let bundle = CString::new(bundle).map_err(|_| Status::Invalid)?;
    invoke(c, visit, |emit, state| unsafe {
        rss_macos_app_paths(bundle.as_ptr(), emit, state)
    })
}
