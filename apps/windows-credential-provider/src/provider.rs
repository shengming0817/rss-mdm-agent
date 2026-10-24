#![allow(non_snake_case)]
use crate::serialization::{self, copy};
use device_access::{Decision, Factor, Store, SystemProtector};
use std::{
    ffi::c_void,
    ptr::null_mut,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::HBITMAP,
        System::Com::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    },
};
use zeroize::{Zeroize, Zeroizing};
pub const CLSID: GUID = GUID::from_u128(0xc61c6b95_9162_42a2_b80b_61ebca730510);
static OBJECTS: AtomicUsize = AtomicUsize::new(0);
static LOCKS: AtomicUsize = AtomicUsize::new(0);
struct Lifetime;
impl Lifetime {
    fn new() -> Self {
        OBJECTS.fetch_add(1, Ordering::SeqCst);
        Self
    }
}
impl Drop for Lifetime {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}
fn unavailable(_: device_access::Error) -> Error {
    E_ACCESSDENIED.into()
}
fn local_console() -> Result<()> {
    device_access::windows::require_system().map_err(unavailable)?;
    if unsafe { GetSystemMetrics(SM_REMOTESESSION) } != 0 {
        return Err(E_NOTIMPL.into());
    }
    Ok(())
}
fn store() -> Result<Store> {
    local_console()?;
    let config = device_access::windows::installation().map_err(unavailable)?;
    if device_access::windows::clock().map_err(unavailable)?.boot == config.installed_boot {
        return Err(E_ACCESSDENIED.into());
    }
    Store::open(&config.root, config.id).map_err(unavailable)
}
#[implement(IClassFactory)]
struct Factory {
    _life: Lifetime,
}
impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<'_, IUnknown>,
        iid: *const GUID,
        output: *mut *mut c_void,
    ) -> Result<()> {
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        if iid.is_null() || output.is_null() {
            return Err(E_POINTER.into());
        }
        local_console()?;
        let provider: ICredentialProvider = Provider {
            _life: Lifetime::new(),
            state: Mutex::new(ProviderState {
                scenario: None,
                credentials: vec![],
            }),
        }
        .into();
        unsafe { provider.query(iid, output).ok() }
    }
    fn LockServer(&self, lock: BOOL) -> Result<()> {
        if lock.as_bool() {
            LOCKS.fetch_add(1, Ordering::SeqCst);
        } else {
            LOCKS
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_sub(1))
                .map_err(|_| Error::from(E_UNEXPECTED))?;
        }
        Ok(())
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    class: *const GUID,
    iid: *const GUID,
    output: *mut *mut c_void,
) -> HRESULT {
    if class.is_null() || iid.is_null() || output.is_null() {
        return E_POINTER;
    }
    unsafe {
        *output = null_mut();
    }
    if unsafe { *class } != CLSID {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = Factory {
        _life: Lifetime::new(),
    }
    .into();
    unsafe { factory.query(iid, output) }
}
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECTS.load(Ordering::SeqCst) == 0 && LOCKS.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}
struct ProviderState {
    scenario: Option<CREDENTIAL_PROVIDER_USAGE_SCENARIO>,
    credentials: Vec<ICredentialProviderCredential>,
}
#[implement(ICredentialProvider, ICredentialProviderSetUserArray)]
struct Provider {
    _life: Lifetime,
    state: Mutex<ProviderState>,
}
const LABELS: [&str; 5] = [
    "RSS 登录",
    "Windows 密码",
    "验证码或恢复密钥",
    "使用恢复密钥",
    "登录",
];
const TYPES: [CREDENTIAL_PROVIDER_FIELD_TYPE; 5] = [
    CPFT_LARGE_TEXT,
    CPFT_PASSWORD_TEXT,
    CPFT_PASSWORD_TEXT,
    CPFT_CHECKBOX,
    CPFT_SUBMIT_BUTTON,
];
impl ICredentialProvider_Impl for Provider_Impl {
    fn SetUsageScenario(&self, scenario: CREDENTIAL_PROVIDER_USAGE_SCENARIO, _: u32) -> Result<()> {
        local_console()?;
        let mut state = self.state.lock().map_err(|_| Error::from(E_UNEXPECTED))?;
        state.credentials.clear();
        state.scenario = None;
        if scenario != CPUS_LOGON && scenario != CPUS_UNLOCK_WORKSTATION {
            return Err(E_NOTIMPL.into());
        }
        state.scenario = Some(scenario);
        Ok(())
    }
    fn SetSerialization(
        &self,
        _: *const CREDENTIAL_PROVIDER_CREDENTIAL_SERIALIZATION,
    ) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn Advise(&self, _: Ref<'_, ICredentialProviderEvents>, _: usize) -> Result<()> {
        Ok(())
    }
    fn UnAdvise(&self) -> Result<()> {
        Ok(())
    }
    fn GetFieldDescriptorCount(&self) -> Result<u32> {
        Ok(5)
    }
    fn GetFieldDescriptorAt(
        &self,
        index: u32,
    ) -> Result<*mut CREDENTIAL_PROVIDER_FIELD_DESCRIPTOR> {
        let label = LABELS
            .get(index as usize)
            .ok_or_else(|| Error::from(E_INVALIDARG))?;
        let text = copy(label)?;
        let descriptor =
            unsafe { CoTaskMemAlloc(std::mem::size_of::<CREDENTIAL_PROVIDER_FIELD_DESCRIPTOR>()) }
                .cast::<CREDENTIAL_PROVIDER_FIELD_DESCRIPTOR>();
        if descriptor.is_null() {
            unsafe {
                CoTaskMemFree(Some(text.0.cast()));
            }
            return Err(E_OUTOFMEMORY.into());
        }
        unsafe {
            descriptor.write(CREDENTIAL_PROVIDER_FIELD_DESCRIPTOR {
                dwFieldID: index,
                cpft: TYPES[index as usize],
                pszLabel: text,
                guidFieldType: GUID::zeroed(),
            });
        }
        Ok(descriptor)
    }
    fn GetCredentialCount(
        &self,
        count: *mut u32,
        default: *mut u32,
        automatic: *mut BOOL,
    ) -> Result<()> {
        if count.is_null() || default.is_null() || automatic.is_null() {
            return Err(E_POINTER.into());
        }
        let state = self.state.lock().map_err(|_| Error::from(E_UNEXPECTED))?;
        unsafe {
            *count = state.credentials.len() as u32;
            *default = CREDENTIAL_PROVIDER_NO_DEFAULT;
            *automatic = false.into();
        }
        Ok(())
    }
    fn GetCredentialAt(&self, index: u32) -> Result<ICredentialProviderCredential> {
        self.state
            .lock()
            .map_err(|_| Error::from(E_UNEXPECTED))?
            .credentials
            .get(index as usize)
            .cloned()
            .ok_or_else(|| E_INVALIDARG.into())
    }
}
impl ICredentialProviderSetUserArray_Impl for Provider_Impl {
    fn SetUserArray(&self, users: Ref<'_, ICredentialProviderUserArray>) -> Result<()> {
        let users = users.as_ref().ok_or_else(|| Error::from(E_INVALIDARG))?;
        let mut state = self.state.lock().map_err(|_| Error::from(E_UNEXPECTED))?;
        state.credentials.clear();
        let scenario = state.scenario.ok_or_else(|| Error::from(E_UNEXPECTED))?;
        let offline = store()?;
        let count = unsafe { users.GetCount()? };
        if count > 1024 {
            return Err(E_INVALIDARG.into());
        }
        for index in 0..count {
            let user = unsafe { users.GetAt(index)? };
            let raw = unsafe { user.GetSid()? };
            let sid = unsafe { serialization::text(PCWSTR(raw.0), 256) };
            unsafe {
                CoTaskMemFree(Some(raw.0.cast()));
            }
            let sid = sid?;
            let Some(authority) = offline.authority(&sid).map_err(unavailable)? else {
                continue;
            };
            if serialization::account(&authority).is_err() {
                continue;
            }
            let login = match unsafe {
                user.GetStringValue(
                    &windows::Win32::Storage::EnhancedStorage::PKEY_Identity_QualifiedUserName,
                )
            } {
                Ok(raw) => {
                    let value = unsafe { serialization::text(PCWSTR(raw.0), 513) };
                    unsafe {
                        CoTaskMemFree(Some(raw.0.cast()));
                    }
                    value?
                }
                Err(_) => authority.login.clone(),
            };
            let credential: ICredentialProviderCredential2 = Credential {
                _life: Lifetime::new(),
                authority,
                login,
                scenario,
                input: Mutex::new(Input {
                    password: Zeroizing::new(String::new()),
                    factor: Zeroizing::new(String::new()),
                    recovery: false,
                    events: None,
                }),
            }
            .into();
            state.credentials.push(credential.cast()?);
        }
        Ok(())
    }
}
struct Input {
    password: Zeroizing<String>,
    factor: Zeroizing<String>,
    recovery: bool,
    events: Option<ICredentialProviderCredentialEvents>,
}
impl Input {
    fn clear(&mut self) {
        self.password.zeroize();
        self.factor.zeroize();
    }
}
#[implement(ICredentialProviderCredential2)]
struct Credential {
    _life: Lifetime,
    authority: device_access::Authority,
    login: String,
    scenario: CREDENTIAL_PROVIDER_USAGE_SCENARIO,
    input: Mutex<Input>,
}
impl Credential_Impl {
    fn clear(&self) -> Result<()> {
        let events = {
            let mut input = self.input.lock().map_err(|_| Error::from(E_UNEXPECTED))?;
            input.clear();
            input.events.clone()
        };
        if let Some(events) = events {
            let credential: ICredentialProviderCredential2 = self.to_interface();
            let this: ICredentialProviderCredential = credential.cast()?;
            unsafe {
                events.SetFieldString(&this, 1, w!(""))?;
                events.SetFieldString(&this, 2, w!(""))?;
            }
        }
        Ok(())
    }
}
impl ICredentialProviderCredential2_Impl for Credential_Impl {
    fn GetUserSid(&self) -> Result<PWSTR> {
        copy(&self.authority.sid)
    }
}
impl ICredentialProviderCredential_Impl for Credential_Impl {
    fn Advise(&self, events: Ref<'_, ICredentialProviderCredentialEvents>) -> Result<()> {
        self.input
            .lock()
            .map_err(|_| Error::from(E_UNEXPECTED))?
            .events = events.as_ref().cloned();
        Ok(())
    }
    fn UnAdvise(&self) -> Result<()> {
        let mut input = self.input.lock().map_err(|_| Error::from(E_UNEXPECTED))?;
        input.clear();
        input.events = None;
        Ok(())
    }
    fn SetSelected(&self) -> Result<BOOL> {
        Ok(false.into())
    }
    fn SetDeselected(&self) -> Result<()> {
        self.clear()
    }
    fn GetFieldState(
        &self,
        id: u32,
        state: *mut CREDENTIAL_PROVIDER_FIELD_STATE,
        interactive: *mut CREDENTIAL_PROVIDER_FIELD_INTERACTIVE_STATE,
    ) -> Result<()> {
        if state.is_null() || interactive.is_null() {
            return Err(E_POINTER.into());
        }
        if id >= 5 {
            return Err(E_INVALIDARG.into());
        }
        unsafe {
            *state = if id == 0 {
                CPFS_DISPLAY_IN_BOTH
            } else {
                CPFS_DISPLAY_IN_SELECTED_TILE
            };
            *interactive = if id == 1 { CPFIS_FOCUSED } else { CPFIS_NONE };
        }
        Ok(())
    }
    fn GetStringValue(&self, id: u32) -> Result<PWSTR> {
        match id {
            0 => copy(LABELS[0]),
            1 | 2 => copy(""),
            4 => copy(LABELS[4]),
            _ => Err(E_INVALIDARG.into()),
        }
    }
    fn GetBitmapValue(&self, _: u32) -> Result<HBITMAP> {
        Err(E_NOTIMPL.into())
    }
    fn GetCheckboxValue(&self, id: u32, checked: *mut BOOL, label: *mut PWSTR) -> Result<()> {
        if checked.is_null() || label.is_null() {
            return Err(E_POINTER.into());
        }
        if id != 3 {
            return Err(E_INVALIDARG.into());
        }
        let value = self
            .input
            .lock()
            .map_err(|_| Error::from(E_UNEXPECTED))?
            .recovery;
        unsafe {
            *label = copy(LABELS[3])?;
            *checked = value.into();
        }
        Ok(())
    }
    fn GetSubmitButtonValue(&self, id: u32) -> Result<u32> {
        if id == 4 {
            Ok(2)
        } else {
            Err(E_INVALIDARG.into())
        }
    }
    fn GetComboBoxValueCount(&self, _: u32, _: *mut u32, _: *mut u32) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn GetComboBoxValueAt(&self, _: u32, _: u32) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }
    fn SetStringValue(&self, id: u32, value: &PCWSTR) -> Result<()> {
        let mut input = self.input.lock().map_err(|_| Error::from(E_UNEXPECTED))?;
        let value =
            Zeroizing::new(unsafe { serialization::text(*value, if id == 1 { 257 } else { 44 })? });
        match id {
            1 => input.password = value,
            2 => input.factor = value,
            _ => return Err(E_INVALIDARG.into()),
        }
        Ok(())
    }
    fn SetCheckboxValue(&self, id: u32, checked: BOOL) -> Result<()> {
        if id != 3 {
            return Err(E_INVALIDARG.into());
        }
        let mut input = self.input.lock().map_err(|_| Error::from(E_UNEXPECTED))?;
        input.recovery = checked.as_bool();
        input.factor.zeroize();
        Ok(())
    }
    fn SetComboBoxSelectedValue(&self, _: u32, _: u32) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn CommandLinkClicked(&self, _: u32) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn GetSerialization(
        &self,
        response: *mut CREDENTIAL_PROVIDER_GET_SERIALIZATION_RESPONSE,
        serialized: *mut CREDENTIAL_PROVIDER_CREDENTIAL_SERIALIZATION,
        text: *mut PWSTR,
        icon: *mut CREDENTIAL_PROVIDER_STATUS_ICON,
    ) -> Result<()> {
        if response.is_null() || serialized.is_null() || text.is_null() || icon.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe {
            *response = CPGSR_NO_CREDENTIAL_NOT_FINISHED;
            *serialized = Default::default();
            *text = PWSTR::null();
            *icon = CPSI_NONE;
        }
        let (password, factor, recovery) = {
            let mut input = self.input.lock().map_err(|_| Error::from(E_UNEXPECTED))?;
            (
                std::mem::replace(&mut input.password, Zeroizing::new(String::new())),
                std::mem::replace(&mut input.factor, Zeroizing::new(String::new())),
                input.recovery,
            )
        };
        let result = (|| {
            serialization::account(&self.authority)?;
            let decision = store()?
                .verify(
                    &self.authority,
                    if recovery {
                        Factor::Recovery(&factor)
                    } else {
                        Factor::Totp(&factor)
                    },
                    device_access::windows::clock().map_err(unavailable)?,
                    &SystemProtector,
                )
                .map_err(unavailable)?;
            if decision != Decision::Accepted {
                return Err(Error::from(E_ACCESSDENIED));
            }
            serialization::pack(&self.authority, &self.login, &password, self.scenario)
        })();
        // Clear both native buffers and LogonUI fields on success, failure and cancelled retry.
        drop(password);
        drop(factor);
        let _ = self.clear();
        match result {
            Ok(value) => unsafe {
                *serialized = value;
                *response = CPGSR_RETURN_CREDENTIAL_FINISHED;
            },
            Err(_) => unsafe {
                *text = copy("RSS 验证未通过或本地状态不可用。请重试，或使用系统登录选项。")
                    .unwrap_or(PWSTR::null());
                *icon = CPSI_ERROR;
            },
        }
        Ok(())
    }
    fn ReportResult(
        &self,
        status: NTSTATUS,
        substatus: NTSTATUS,
        text: *mut PWSTR,
        icon: *mut CREDENTIAL_PROVIDER_STATUS_ICON,
    ) -> Result<()> {
        if text.is_null() || icon.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe {
            *text = PWSTR::null();
            *icon = CPSI_NONE;
        }
        self.clear()?;
        if let Some(message) = crate::status::message(status.0, substatus.0) {
            unsafe {
                *text = copy(message)?;
                *icon = CPSI_ERROR;
            }
        }
        Ok(())
    }
}
