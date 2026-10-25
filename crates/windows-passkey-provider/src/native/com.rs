//! Minimal local COM server ABI. No webview, service IPC, or caller-supplied UV.
//! ref: microsoft/webauthn pluginauthenticator.idl; Windows-classic-samples PasskeyManager.
use super::*;
use std::{
    ffi::c_void,
    sync::atomic::{AtomicU32, Ordering},
};
use windows_sys::{
    core::GUID,
    Win32::{
        Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
        System::{Com::*, RemoteDesktop::*},
    },
};
const UNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);
const FACTORY: GUID = GUID::from_u128(0x00000001_0000_0000_c000_000000000046);
const PLUGIN: GUID = GUID::from_u128(0xd26bcf6f_b54c_43ff_9f06_d5bf148625f7);
const E_POINTER: i32 = 0x80004003u32 as i32;
#[repr(C)]
struct Base {
    query: unsafe extern "system" fn(*mut Object, *const GUID, *mut *mut c_void) -> i32,
    add: unsafe extern "system" fn(*mut Object) -> u32,
    release: unsafe extern "system" fn(*mut Object) -> u32,
}
#[repr(C)]
struct Plugin {
    base: Base,
    create: unsafe extern "system" fn(*mut Object, *const api::Request, *mut api::Response) -> i32,
    assertion:
        unsafe extern "system" fn(*mut Object, *const api::Request, *mut api::Response) -> i32,
    cancel: unsafe extern "system" fn(*mut Object, *const api::Cancel) -> i32,
    locked: unsafe extern "system" fn(*mut Object, *mut i32) -> i32,
}
#[repr(C)]
struct Factory {
    base: Base,
    create:
        unsafe extern "system" fn(*mut Object, *mut c_void, *const GUID, *mut *mut c_void) -> i32,
    lock: unsafe extern "system" fn(*mut Object, i32) -> i32,
}
#[repr(C)]
struct Object {
    vtable: *const c_void,
    refs: AtomicU32,
    runtime: Arc<ProviderRuntime>,
    factory: bool,
}
const BASE: Base = Base {
    query,
    add,
    release,
};
static PLUGIN_TABLE: Plugin = Plugin {
    base: BASE,
    create,
    assertion,
    cancel,
    locked,
};
static FACTORY_TABLE: Factory = Factory {
    base: BASE,
    create: instance,
    lock: server_lock,
};
fn object(runtime: Arc<ProviderRuntime>, factory: bool) -> *mut Object {
    Box::into_raw(Box::new(Object {
        vtable: if factory {
            &FACTORY_TABLE as *const _ as _
        } else {
            &PLUGIN_TABLE as *const _ as _
        },
        refs: AtomicU32::new(1),
        runtime,
        factory,
    }))
}
unsafe extern "system" fn query(this: *mut Object, iid: *const GUID, out: *mut *mut c_void) -> i32 {
    if out.is_null() || iid.is_null() {
        return E_POINTER;
    }
    unsafe {
        *out = std::ptr::null_mut();
        let expected = if (*this).factory { &FACTORY } else { &PLUGIN };
        if identity::guid_bytes(&*iid) != identity::guid_bytes(&UNKNOWN)
            && identity::guid_bytes(&*iid) != identity::guid_bytes(expected)
        {
            return 0x80004002u32 as i32;
        }
        *out = this.cast();
        add(this);
        0
    }
}
unsafe extern "system" fn add(this: *mut Object) -> u32 {
    unsafe { (*this).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn release(this: *mut Object) -> u32 {
    let remaining = unsafe { (*this).refs.fetch_sub(1, Ordering::AcqRel) - 1 };
    if remaining == 0 {
        unsafe {
            drop(Box::from_raw(this));
        }
    }
    remaining
}
unsafe extern "system" fn instance(
    this: *mut Object,
    outer: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if !outer.is_null() {
        return 0x80040110u32 as i32;
    }
    unsafe {
        let value = object((*this).runtime.clone(), false);
        let result = query(value, iid, out);
        release(value);
        result
    }
}
unsafe extern "system" fn server_lock(_this: *mut Object, _lock: i32) -> i32 {
    0
}
fn error(error: Error) -> i32 {
    match error {
        Error::Cancelled => 0x80090036u32 as i32,
        Error::Busy => 0x800700aau32 as i32,
        Error::NotFound => 0x80090011u32 as i32,
        Error::Excluded => 0x8009000fu32 as i32,
        Error::Input => 0x80070057u32 as i32,
        Error::Owner | Error::Authentication => 0x80070005u32 as i32,
        _ => 0x80004005u32 as i32,
    }
}
unsafe fn invoke(
    this: *mut Object,
    request: *const api::Request,
    response: *mut api::Response,
    kind: OperationKind,
) -> i32 {
    if request.is_null() || response.is_null() {
        return E_POINTER;
    }
    unsafe {
        *response = api::Response {
            len: 0,
            bytes: std::ptr::null_mut(),
        };
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        (*this).runtime.execute(kind, &*request)
    }));
    match result {
        Ok(Ok(bytes)) => unsafe {
            let memory = CoTaskMemAlloc(bytes.len()).cast::<u8>();
            if memory.is_null() {
                return 0x8007000eu32 as i32;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), memory, bytes.len());
            *response = api::Response {
                len: bytes.len() as u32,
                bytes: memory,
            };
            0
        },
        Ok(Err(e)) => error(e),
        Err(_) => 0x80004005u32 as i32,
    }
}
unsafe extern "system" fn create(
    this: *mut Object,
    request: *const api::Request,
    response: *mut api::Response,
) -> i32 {
    unsafe { invoke(this, request, response, OperationKind::Create) }
}
unsafe extern "system" fn assertion(
    this: *mut Object,
    request: *const api::Request,
    response: *mut api::Response,
) -> i32 {
    unsafe { invoke(this, request, response, OperationKind::Assert) }
}
unsafe extern "system" fn cancel(this: *mut Object, request: *const api::Cancel) -> i32 {
    if request.is_null() {
        return E_POINTER;
    }
    // Official V1 docs do not specify cancel signature encoding. Only terminate the exact
    // existing transaction; never interpret cancellation as authorization or a new request.
    unsafe {
        match (*this)
            .runtime
            .operations
            .cancel(identity::guid_bytes(&(*request).transaction))
        {
            Ok(()) => 0,
            Err(e) => error(e),
        }
    }
}
unsafe extern "system" fn locked(_this: *mut Object, value: *mut i32) -> i32 {
    if value.is_null() {
        E_POINTER
    } else {
        unsafe {
            *value = 0;
        }
        match registration::clear_dirty() {
            Ok(()) => 0,
            Err(e) => error(e),
        }
    }
}
unsafe extern "system" fn window(hwnd: HWND, msg: u32, w: usize, l: isize) -> isize {
    let runtime = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const ProviderRuntime;
    if !runtime.is_null()
        && ((msg == WM_WTSSESSION_CHANGE
            && matches!(
                w as u32,
                WTS_SESSION_LOCK
                    | WTS_SESSION_LOGOFF
                    | WTS_CONSOLE_DISCONNECT
                    | WTS_REMOTE_DISCONNECT
            ))
            || (msg == WM_POWERBROADCAST && w == 4)
            || msg == WM_QUERYENDSESSION)
    {
        unsafe {
            (*runtime).operations.cancel_active();
            SetEvent((*runtime).cancel.0);
        }
    }
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}
pub fn run(runtime: Arc<ProviderRuntime>) -> Result<()> {
    // SAFETY: this thread owns COM initialization, registration and the notification window.
    unsafe {
        api::hr(CoInitializeEx(
            std::ptr::null(),
            COINIT_MULTITHREADED as u32,
        ))?;
        struct Com;
        impl Drop for Com {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _com = Com;
        initialize_security(&runtime.owner)?;
        let factory = object(runtime.clone(), true);
        let mut cookie = 0;
        let result = CoRegisterClassObject(
            &api::ID,
            factory.cast(),
            CLSCTX_LOCAL_SERVER,
            REGCLS_MULTIPLEUSE as u32,
            &mut cookie,
        );
        release(factory);
        api::hr(result)?;
        struct Registration(u32);
        impl Drop for Registration {
            fn drop(&mut self) {
                unsafe {
                    CoRevokeClassObject(self.0);
                }
            }
        }
        let _registration = Registration(cookie);
        let name = wide("RSSPersonalPasskeySession");
        let mut class: WNDCLASSW = std::mem::zeroed();
        class.lpfnWndProc = Some(window);
        class.lpszClassName = name.as_ptr();
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err(Error::SystemUnavailable);
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Arc::as_ptr(&runtime) as isize);
        if WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) == 0 {
            DestroyWindow(hwnd);
            return Err(Error::SystemUnavailable);
        }
        let mut cache_check = std::time::Instant::now();
        while !runtime.stop.load(Ordering::Acquire) || !runtime.operations.is_idle() {
            if WaitForSingleObject(runtime.cancel.0, 0) == WAIT_OBJECT_0 {
                runtime.operations.cancel_active();
            }
            if WaitForSingleObject(runtime.stop_event.0, 0) == WAIT_OBJECT_0 {
                runtime.shutdown();
            }
            if cache_check.elapsed() >= std::time::Duration::from_millis(250) {
                let _ = registration::clear_dirty();
                cache_check = std::time::Instant::now();
            }
            let mut message: MSG = std::mem::zeroed();
            while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        WTSUnRegisterSessionNotification(hwnd);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        DestroyWindow(hwnd);
    }
    Ok(())
}

pub fn refresh_index() -> Result<()> {
    unsafe {
        api::hr(CoInitializeEx(
            std::ptr::null(),
            COINIT_MULTITHREADED as u32,
        ))?;
        let mut object: *mut Object = std::ptr::null_mut();
        let result = CoCreateInstance(
            &api::ID,
            std::ptr::null_mut(),
            CLSCTX_LOCAL_SERVER,
            &PLUGIN,
            (&mut object as *mut *mut Object).cast(),
        );
        if result < 0 {
            CoUninitialize();
            return Err(Error::SystemUnavailable);
        }
        let table = &*((*object).vtable as *const Plugin);
        let mut status = 0;
        let result = (table.locked)(object, &mut status);
        (table.base.release)(object);
        CoUninitialize();
        api::hr(result)
    }
}

fn initialize_security(owner: &str) -> Result<()> {
    use windows_sys::Win32::Security::*;
    // CoInitializeSecurity requires an absolute descriptor. Its pointer fields borrow
    // the relative descriptor's SID/ACL backing allocation through the complete call.
    unsafe {
        let mut relative = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide(&format!("O:{0}G:{0}D:(A;;0xB;;;{0})(A;;0xB;;;SY)", owner)).as_ptr(),
            1,
            &mut relative,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(Error::Owner);
        }
        struct Relative(PSECURITY_DESCRIPTOR);
        impl Drop for Relative {
            fn drop(&mut self) {
                unsafe {
                    LocalFree(self.0.cast());
                }
            }
        }
        let _backing = Relative(relative);
        let (mut sid, mut group, mut dacl) = (
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        let (mut defaulted, mut present) = (0, 0);
        let mut absolute: SECURITY_DESCRIPTOR = std::mem::zeroed();
        let descriptor = (&mut absolute as *mut SECURITY_DESCRIPTOR).cast();
        if GetSecurityDescriptorOwner(relative, &mut sid, &mut defaulted) == 0
            || GetSecurityDescriptorGroup(relative, &mut group, &mut defaulted) == 0
            || GetSecurityDescriptorDacl(relative, &mut present, &mut dacl, &mut defaulted) == 0
            || present == 0
            || dacl.is_null()
            || InitializeSecurityDescriptor(descriptor, 1) == 0
            || SetSecurityDescriptorOwner(descriptor, sid, 0) == 0
            || SetSecurityDescriptorGroup(descriptor, group, 0) == 0
            || SetSecurityDescriptorDacl(descriptor, 1, dacl, 0) == 0
        {
            return Err(Error::Owner);
        }
        api::hr(CoInitializeSecurity(
            descriptor,
            -1,
            std::ptr::null(),
            std::ptr::null(),
            RPC_C_AUTHN_LEVEL_PKT_PRIVACY,
            RPC_C_IMP_LEVEL_IDENTIFY,
            std::ptr::null(),
            EOAC_DISABLE_AAA as u32,
            std::ptr::null(),
        ))
    }
}
