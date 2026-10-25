use windows_sys::Win32::Graphics::Gdi::COLOR_WINDOW;
// ref: Microsoft IUserConsentVerifierInterop; Win32 ES_PASSWORD / WM_WTSSESSION_CHANGE.
use super::*;
use std::{
    sync::atomic::{AtomicIsize, AtomicU64, Ordering},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::RemoteDesktop::*,
    UI::{
        Controls::EM_SETLIMITTEXT,
        Input::KeyboardAndMouse::{EnableWindow, SetFocus},
        Shell::*,
        WindowsAndMessaging::*,
    },
};
static MAIN: AtomicIsize = AtomicIsize::new(0);
static DIALOG: AtomicIsize = AtomicIsize::new(0);
static CANCEL: AtomicU64 = AtomicU64::new(0);
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
pub fn cancel() {
    CANCEL.fetch_add(1, Ordering::AcqRel);
    let hwnd = DIALOG.load(Ordering::Acquire) as HWND;
    if !hwnd.is_null() {
        // SAFETY: PostMessage does not dereference window state; stale HWND is harmless.
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }
}
pub fn verify() -> Result<()> {
    use windows::{
        core::{factory, HSTRING},
        Security::Credentials::UI::{UserConsentVerificationResult, UserConsentVerifier},
        Win32::{Foundation::HWND as Window, System::WinRT::IUserConsentVerifierInterop},
    };
    let hwnd = MAIN.load(Ordering::Acquire) as HWND;
    // SAFETY: reads the current interactive foreground window.
    if hwnd.is_null() || unsafe { GetForegroundWindow() } != hwnd {
        return Err(Error::Authentication);
    }
    let generation = CANCEL.load(Ordering::Acquire);
    let interop = factory::<UserConsentVerifier, IUserConsentVerifierInterop>()
        .map_err(|_| Error::SystemUnavailable)?;
    // SAFETY: main HWND belongs to this application; returned operation owns its lifetime.
    let operation: windows_future::IAsyncOperation<UserConsentVerificationResult> = unsafe {
        interop.RequestVerificationForWindowAsync(
            Window(hwnd),
            &HSTRING::from("解锁或管理个人 Passkey Vault"),
        )
    }
    .map_err(|_| Error::SystemUnavailable)?;
    let started = Instant::now();
    loop {
        if started.elapsed() > Duration::from_secs(120)
            || generation != CANCEL.load(Ordering::Acquire)
        {
            let _ = operation.Cancel();
            return Err(Error::Cancelled);
        }
        if operation.Status().map_err(|_| Error::Authentication)?
            != windows_future::AsyncStatus::Started
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if operation.GetResults().map_err(|_| Error::Authentication)?
        == UserConsentVerificationResult::Verified
    {
        Ok(())
    } else {
        Err(Error::Authentication)
    }
}
struct Input {
    edit: HWND,
    value: Option<Zeroizing<String>>,
}
unsafe extern "system" fn dialog(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: pointer installed by password() lives until the window is destroyed.
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Input;
    if msg == WM_COMMAND && (w & 0xffff) == 1 && !pointer.is_null() {
        let input = unsafe { &mut *pointer };
        let len = unsafe { GetWindowTextLengthW(input.edit) };
        if (1..=1024).contains(&len) {
            let mut buf = Zeroizing::new(vec![0u16; len as usize + 1]);
            unsafe {
                GetWindowTextW(input.edit, buf.as_mut_ptr(), buf.len() as i32);
            }
            if let Ok(value) = String::from_utf16(&buf[..len as usize]) {
                let value = Zeroizing::new(value);
                if value.len() <= 1024 {
                    input.value = Some(value);
                }
            }
        }
        unsafe {
            SetWindowTextW(input.edit, wide("").as_ptr());
            DestroyWindow(hwnd);
        }
        return 0;
    }
    if msg == WM_CLOSE || msg == WM_TIMER || (msg == WM_COMMAND && (w & 0xffff) == 2) {
        if !pointer.is_null() {
            unsafe {
                SetWindowTextW((*pointer).edit, wide("").as_ptr());
            }
        }
        unsafe {
            DestroyWindow(hwnd);
        }
        return 0;
    }
    if msg == WM_DESTROY {
        unsafe {
            PostQuitMessage(0);
        }
        return 0;
    }
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}
pub fn password(label: &str) -> Result<Zeroizing<String>> {
    let owner = MAIN.load(Ordering::Acquire) as HWND;
    if owner.is_null() || unsafe { GetForegroundWindow() } != owner {
        return Err(Error::Authentication);
    }
    let generation = CANCEL.load(Ordering::Acquire);
    let class = wide("RSSPersonalVaultInput");
    let mut input = Input {
        edit: std::ptr::null_mut(),
        value: None,
    };
    // SAFETY: all control strings remain live through each synchronous Win32 call.
    unsafe {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(dialog),
            lpszClassName: class.as_ptr(),
            hbrBackground: (COLOR_WINDOW + 1) as _,
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class.as_ptr(),
            wide("个人 Passkey Vault").as_ptr(),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            210,
            owner,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err(Error::SystemUnavailable);
        }
        CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide(label).as_ptr(),
            WS_CHILD | WS_VISIBLE,
            20,
            20,
            470,
            45,
            hwnd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        input.edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            wide("EDIT").as_ptr(),
            wide("").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_PASSWORD as u32 | ES_AUTOHSCROLL as u32,
            20,
            70,
            470,
            30,
            hwnd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        SendMessageW(input.edit, EM_SETLIMITTEXT, 1025, 0);
        for (id, text, x) in [(1, "继续", 280), (2, "取消", 390)] {
            CreateWindowExW(
                0,
                wide("BUTTON").as_ptr(),
                wide(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                x,
                120,
                100,
                30,
                hwnd,
                id as _,
                std::ptr::null_mut(),
                std::ptr::null(),
            );
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&mut input as *mut Input) as isize);
        EnableWindow(owner, 0);
        SetForegroundWindow(hwnd);
        SetFocus(input.edit);
        DIALOG.store(hwnd as isize, Ordering::Release);
        SetTimer(hwnd, 1, 120000, None);
        let mut message = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        DIALOG.store(0, Ordering::Release);
        EnableWindow(owner, 1);
        SetForegroundWindow(owner);
    }
    if generation != CANCEL.load(Ordering::Acquire) {
        return Err(Error::Cancelled);
    }
    input.value.ok_or(Error::Cancelled)
}
unsafe extern "system" fn session(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    if (msg == WM_WTSSESSION_CHANGE
        && matches!(
            w as u32,
            WTS_SESSION_LOCK | WTS_SESSION_LOGOFF | WTS_CONSOLE_DISCONNECT | WTS_REMOTE_DISCONNECT
        ))
        || (msg == WM_POWERBROADCAST && w == 4)
    {
        // SAFETY: retained Arc pointer belongs to this subclass until WM_NCDESTROY.
        unsafe {
            (&*(data as *const Vault)).lock();
        }
        cancel();
    }
    if msg == WM_NCDESTROY {
        unsafe {
            WTSUnRegisterSessionNotification(hwnd);
            RemoveWindowSubclass(hwnd, Some(session), id);
            drop(Arc::from_raw(data as *const Vault));
        }
    }
    unsafe { DefSubclassProc(hwnd, msg, w, l) }
}
pub fn observe<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>, vault: Arc<Vault>) {
    if let Ok(handle) = window.hwnd() {
        let hwnd = handle.0;
        MAIN.store(hwnd as isize, Ordering::Release);
        let pointer = Arc::into_raw(vault);
        // SAFETY: called on the native main thread, subclass retains the Arc until destruction.
        unsafe {
            if SetWindowSubclass(hwnd, Some(session), 2928, pointer as usize) == 0 {
                drop(Arc::from_raw(pointer));
                return;
            }
            WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
        }
    }
}
