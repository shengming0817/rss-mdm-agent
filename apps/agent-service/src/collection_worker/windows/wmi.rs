//! ref: microsoft/windows-rs@0.61.0 Win32/System/Wmi/mod.rs (semisynchronous enumeration).
use super::*;
use ::windows::{
    core::{BSTR, PCWSTR},
    Win32::System::{
        Com::*,
        Ole::*,
        Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE},
        Variant::*,
        Wmi::*,
    },
};
pub(super) struct Apartment;
impl Apartment {
    pub(super) fn open() -> Result<Self, Status> {
        // SAFETY: the query runs on its own synchronous process thread; balanced by Drop.
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(error)?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct Variant(VARIANT);
impl Drop for Variant {
    fn drop(&mut self) {
        unsafe {
            let _ = VariantClear(&mut self.0);
        }
    }
}

pub(super) fn read(
    def: &w::NativeQueryDefinition,
    budget: &Budget,
    mut emit: impl FnMut(Row) -> Result<(), Status>,
) -> Result<(), Status> {
    let query = super::super::values::select(def)?;
    let w::NativeQueryDefinition::WindowsWmi {
        namespace,
        properties,
        ..
    } = def
    else {
        return Err(Status::Invalid);
    };
    let _apartment = Apartment::open()?;
    // SAFETY: process-private COM apartment, local namespace and validated fixed WQL only.
    unsafe {
        let services = connect(namespace)?;
        budget.check()?;
        let iter = services
            .ExecQuery(
                &BSTR::from("WQL"),
                &BSTR::from(query.as_str()),
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                None,
            )
            .map_err(error)?;
        loop {
            budget.check()?;
            let mut objects = [None];
            let mut returned = 0;
            let wait = budget
                .timeout
                .saturating_sub(budget.start.elapsed())
                .as_millis()
                .min(1000) as i32;
            let status = iter.Next(wait, &mut objects, &mut returned);
            if status.is_err() {
                return Err(error(::windows::core::Error::from_hresult(status)));
            }
            if returned > 1 {
                return Err(Status::Invalid);
            }
            if returned == 1 {
                let object = objects[0].take().ok_or(Status::Invalid)?;
                let mut row = Row::new();
                for (name, property) in properties {
                    budget.check()?;
                    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
                    let mut raw = Variant(VARIANT::default());
                    let mut cim = 0;
                    match object.Get(PCWSTR(wide.as_ptr()), 0, &mut raw.0, Some(&mut cim), None) {
                        Ok(()) => (),
                        Err(e) if e.code().0 as u32 == 0x80041002 => continue,
                        Err(e) => return Err(error(e)),
                    }
                    if cim
                        != (cim_code(property.cim_type)
                            | if property.array { CIM_FLAG_ARRAY.0 } else { 0 })
                    {
                        return Err(Status::Invalid);
                    }
                    if let Some(value) = convert(&raw.0, property, budget.output)? {
                        row.insert(name.clone(), value);
                    }
                }
                emit(row)?;
            }
            if status.0 == WBEM_S_FALSE.0 {
                break;
            }
            if status.0 != WBEM_S_NO_ERROR.0 && status.0 != WBEM_S_TIMEDOUT.0 {
                return Err(Status::Invalid);
            }
        }
    }
    Ok(())
}
fn cim_code(c: w::CimType) -> i32 {
    use w::CimType::*;
    match c {
        String => 8,
        Boolean => 11,
        Sint8 => 16,
        Sint16 => 2,
        Sint32 => 3,
        Sint64 => 20,
        Uint8 => 17,
        Uint16 => 18,
        Uint32 => 19,
        Uint64 => 21,
        Real32 => 4,
        Real64 => 5,
        Datetime => 101,
    }
}
// SAFETY: raw owns its COM value; tag checks precede every union access. Array elements
// are copied into separately owned VARIANTs and cleared even on conversion failure.
unsafe fn convert(
    raw: &VARIANT,
    p: &w::WmiProperty,
    cap: usize,
) -> Result<Option<serde_json::Value>, Status> {
    unsafe {
        let inner = &raw.Anonymous.Anonymous;
        let vt = inner.vt;
        if vt == VT_NULL || vt == VT_EMPTY {
            return Ok(None);
        }
        if p.array {
            if vt.0 & VT_ARRAY.0 == 0 || vt.0 & VT_BYREF.0 != 0 {
                return Err(Status::Invalid);
            }
            let array = inner.Anonymous.parray;
            if array.is_null() || SafeArrayGetDim(array) != 1 {
                return Err(Status::Invalid);
            }
            let element = SafeArrayGetVartype(array).map_err(error)?;
            if ![
                VT_BSTR, VT_BOOL, VT_I2, VT_I4, VT_UI1, VT_UI4, VT_I8, VT_UI8, VT_R4, VT_R8,
            ]
            .contains(&element)
            {
                return Err(Status::Invalid);
            }
            if element.0 != vt.0 & !VT_ARRAY.0 {
                return Err(Status::Invalid);
            }
            let lower = SafeArrayGetLBound(array, 1).map_err(error)?;
            let upper = SafeArrayGetUBound(array, 1).map_err(error)?;
            let count = i64::from(upper) - i64::from(lower) + 1;
            if !(0..=65535).contains(&count) {
                return Err(Status::Truncated);
            }
            let mut values = Vec::new();
            let mut bytes = 0usize;
            for index in lower..=upper {
                let mut value = Variant(VARIANT::default());
                let element_value = &mut *value.0.Anonymous.Anonymous;
                element_value.vt = element;
                SafeArrayGetElement(
                    array,
                    &index,
                    (&mut element_value.Anonymous as *mut VARIANT_0_0_0).cast(),
                )
                .map_err(error)?;
                let value = scalar(&value.0, p.cim_type)?;
                bytes = bytes
                    .checked_add(
                        serde_json::to_vec(&value)
                            .map_err(|_| Status::Invalid)?
                            .len(),
                    )
                    .ok_or(Status::Truncated)?;
                if bytes > cap {
                    return Err(Status::Truncated);
                }
                values.push(value);
            }
            Ok(Some(values.into()))
        } else {
            if vt.0 & (VT_ARRAY.0 | VT_BYREF.0) != 0 {
                return Err(Status::Invalid);
            }
            scalar(raw, p.cim_type).map(Some)
        }
    }
}
unsafe fn scalar(raw: &VARIANT, c: w::CimType) -> Result<serde_json::Value, Status> {
    use super::super::values::{cim_value, CimValue as V};
    unsafe {
        let inner = &raw.Anonymous.Anonymous;
        let vt = inner.vt;
        let v = &inner.Anonymous;
        let raw = match vt {
            VT_BSTR => {
                if v.bstrVal.len() > 65536 {
                    return Err(Status::Truncated);
                }
                V::Text(String::from_utf16(&v.bstrVal).map_err(|_| Status::Invalid)?)
            }
            VT_BOOL => V::Bool(v.boolVal.0),
            VT_I2 => V::I16(v.iVal),
            VT_I4 => V::I32(v.lVal),
            VT_UI1 => V::U8(v.bVal),
            VT_UI4 => V::U32(v.ulVal),
            VT_I8 => V::I64(v.llVal),
            VT_UI8 => V::U64(v.ullVal),
            VT_R4 => V::F32(v.fltVal),
            VT_R8 => V::F64(v.dblVal),
            _ => return Err(Status::Invalid),
        };
        cim_value(c, raw)
    }
}

fn connect(namespace: &str) -> Result<IWbemServices, Status> {
    unsafe {
        static SECURITY: std::sync::OnceLock<Result<(), Status>> = std::sync::OnceLock::new();
        (*SECURITY.get_or_init(|| {
            CoInitializeSecurity(
                None,
                -1,
                None,
                None,
                RPC_C_AUTHN_LEVEL_DEFAULT,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
                None,
            )
            .map_err(error)
        }))?;
        let locator: IWbemLocator =
            CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).map_err(error)?;
        let services = locator
            .ConnectServer(
                &BSTR::from(namespace),
                &BSTR::new(),
                &BSTR::new(),
                &BSTR::new(),
                WBEM_FLAG_CONNECT_USE_MAX_WAIT.0,
                &BSTR::new(),
                None,
            )
            .map_err(error)?;
        CoSetProxyBlanket(
            &services,
            RPC_C_AUTHN_WINNT,
            RPC_C_AUTHZ_NONE,
            None,
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )
        .map_err(error)?;
        Ok(services)
    }
}

/// Fixed BitLocker observation methods only. Caller cannot supply a method or WMI path.
pub(super) fn bitlocker(budget: &Budget, rows: &mut Rows<'_>) -> Result<(), Status> {
    let _apartment = Apartment::open()?;
    unsafe {
        let services = connect("root\\cimv2\\Security\\MicrosoftVolumeEncryption")?;
        let iter = services
            .ExecQuery(
                &BSTR::from("WQL"),
                &BSTR::from("SELECT DeviceID FROM Win32_EncryptableVolume"),
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                None,
            )
            .map_err(error)?;
        let mut partial = false;
        loop {
            budget.check()?;
            let mut objects = [None];
            let mut count = 0;
            let status = iter.Next(250, &mut objects, &mut count);
            if status.is_err() {
                return Err(error(::windows::core::Error::from_hresult(status)));
            }
            if count == 1 {
                let object = objects[0].take().ok_or(Status::Invalid)?;
                let get = |object: &IWbemClassObject,
                           name: &str,
                           typ: w::CimType|
                 -> Result<Option<serde_json::Value>, Status> {
                    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
                    let mut raw = Variant(VARIANT::default());
                    object
                        .Get(PCWSTR(name.as_ptr()), 0, &mut raw.0, None, None)
                        .map_err(error)?;
                    convert(
                        &raw.0,
                        &w::WmiProperty {
                            cim_type: typ,
                            array: false,
                        },
                        budget.output,
                    )
                };
                let path = get(&object, "__PATH", w::CimType::String)?
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .ok_or(Status::Invalid)?;
                let id = get(&object, "DeviceID", w::CimType::String)?.ok_or(Status::Invalid)?;
                let mut row = Row::from([("id".into(), id)]);
                for (method, fields) in [
                    (
                        "GetConversionStatus",
                        &[
                            ("ConversionStatus", "conversion"),
                            ("EncryptionPercentage", "encrypted_percent"),
                        ][..],
                    ),
                    (
                        "GetProtectionStatus",
                        &[("ProtectionStatus", "protection")][..],
                    ),
                    ("GetLockStatus", &[("LockStatus", "lock")][..]),
                    (
                        "GetEncryptionMethod",
                        &[("EncryptionMethod", "algorithm")][..],
                    ),
                ] {
                    budget.check()?;
                    let mut output = None;
                    if services
                        .ExecMethod(
                            &BSTR::from(path.as_str()),
                            &BSTR::from(method),
                            WBEM_GENERIC_FLAG_TYPE(0),
                            None,
                            None,
                            Some(&mut output),
                            None,
                        )
                        .is_err()
                    {
                        partial = true;
                        continue;
                    }
                    let Some(output) = output else {
                        partial = true;
                        continue;
                    };
                    if get(&output, "ReturnValue", w::CimType::Uint32)?.and_then(|v| v.as_u64())
                        != Some(0)
                    {
                        partial = true;
                        continue;
                    }
                    for (property, key) in fields {
                        let value =
                            get(&output, property, w::CimType::Uint32)?.and_then(|v| v.as_u64());
                        if let Some(value) =
                            value.and_then(|v| super::super::system_values::bitlocker(key, v))
                        {
                            row.insert((*key).into(), value);
                        } else {
                            partial = true;
                        }
                    }
                }
                rows.push(row)?;
            } else if count > 1 {
                return Err(Status::Invalid);
            }
            if status.0 == WBEM_S_FALSE.0 {
                break;
            }
            if status.0 != WBEM_S_NO_ERROR.0 && status.0 != WBEM_S_TIMEDOUT.0 {
                return Err(Status::Failed);
            }
        }
        if partial {
            Err(Status::Partial)
        } else {
            Ok(())
        }
    }
}
