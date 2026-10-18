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
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}
struct Value(VARIANT);
impl Drop for Value {
    fn drop(&mut self) {
        unsafe {
            // Secret BSTR values are erased before COM releases the allocation.
            if self.0.Anonymous.Anonymous.vt == VT_BSTR {
                let b = &self.0.Anonymous.Anonymous.Anonymous.bstrVal;
                if !b.is_empty() {
                    std::ptr::write_bytes(b.as_ptr().cast_mut(), 0, b.len());
                }
            }
            let _ = VariantClear(&mut self.0);
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
fn connect() -> Result<(Apartment, IWbemServices), F> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(error)?;
        let apartment = Apartment;
        // This worker is a fresh process. Repeated calls may report security already initialized.
        if let Err(e) = CoInitializeSecurity(
            None,
            -1,
            None,
            None,
            RPC_C_AUTHN_LEVEL_DEFAULT,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
            None,
        ) {
            if e.code().0 as u32 != 0x80010119 {
                return Err(error(e));
            }
        }
        let locator: IWbemLocator =
            CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).map_err(error)?;
        let services = locator
            .ConnectServer(
                &BSTR::from("ROOT\\CIMV2\\Security\\MicrosoftVolumeEncryption"),
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
        Ok((apartment, services))
    }
}
fn get(object: &IWbemClassObject, name: &str) -> Result<Value, F> {
    unsafe {
        let mut value = Value(VARIANT::default());
        object
            .Get(PCWSTR(wide(name).as_ptr()), 0, &mut value.0, None, None)
            .map_err(error)?;
        Ok(value)
    }
}
fn text(object: &IWbemClassObject, name: &str) -> Result<Zeroizing<String>, F> {
    let v = get(object, name)?;
    unsafe {
        if v.0.Anonymous.Anonymous.vt != VT_BSTR {
            return Err(F::NativeFailure);
        }
        let b = &v.0.Anonymous.Anonymous.Anonymous.bstrVal;
        if b.len() > 4096 {
            return Err(F::NativeFailure);
        }
        Ok(Zeroizing::new(b.to_string()))
    }
}
fn number(object: &IWbemClassObject, name: &str) -> Result<u32, F> {
    let v = get(object, name)?;
    unsafe {
        let inner = &v.0.Anonymous.Anonymous;
        if inner.vt == VT_I4 {
            Ok(inner.Anonymous.lVal as u32)
        } else if inner.vt == VT_UI4 {
            Ok(inner.Anonymous.ulVal)
        } else {
            Err(F::NativeFailure)
        }
    }
}
fn strings(object: &IWbemClassObject, name: &str) -> Result<Vec<String>, F> {
    let v = get(object, name)?;
    unsafe {
        let inner = &v.0.Anonymous.Anonymous;
        if inner.vt.0 != (VT_ARRAY.0 | VT_BSTR.0) {
            return Err(F::NativeFailure);
        }
        let array = inner.Anonymous.parray;
        if array.is_null() || SafeArrayGetDim(array) != 1 {
            return Err(F::NativeFailure);
        }
        let lo = SafeArrayGetLBound(array, 1).map_err(error)?;
        let hi = SafeArrayGetUBound(array, 1).map_err(error)?;
        if i64::from(hi) - i64::from(lo) + 1 > 512 {
            return Err(F::NativeFailure);
        }
        let mut result = vec![];
        for index in lo..=hi {
            let mut b = BSTR::new();
            SafeArrayGetElement(array, &index, (&mut b as *mut BSTR).cast()).map_err(error)?;
            if b.len() > 256 {
                return Err(F::NativeFailure);
            }
            result.push(b.to_string());
        }
        Ok(result)
    }
}
fn volumes(s: &IWbemServices) -> Result<Vec<IWbemClassObject>, F> {
    unsafe {
        let iterator = s
            .ExecQuery(
                &BSTR::from("WQL"),
                &BSTR::from("SELECT * FROM Win32_EncryptableVolume"),
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                None,
            )
            .map_err(error)?;
        let mut result = vec![];
        loop {
            let mut values = [None];
            let mut count = 0;
            let status = iterator.Next(5000, &mut values, &mut count);
            status.ok().map_err(error)?;
            if count == 1 {
                result.push(values[0].take().ok_or(F::NativeFailure)?);
            }
            if result.len() > 512 {
                return Err(F::NativeFailure);
            }
            if status.0 == WBEM_S_FALSE.0 {
                break;
            }
            if count != 1 {
                return Err(F::NativeFailure);
            }
        }
        Ok(result)
    }
}
fn method(
    s: &IWbemServices,
    v: &IWbemClassObject,
    name: &str,
    args: &[(&str, Value)],
) -> Result<IWbemClassObject, F> {
    unsafe {
        let path = text(v, "__PATH")?;
        let mut input = None;
        let mut output_definition = None;
        v.GetMethod(
            PCWSTR(wide(name).as_ptr()),
            0,
            &mut input,
            &mut output_definition,
        )
        .map_err(error)?;
        let input = if let Some(def) = input {
            Some(def.SpawnInstance(0).map_err(error)?)
        } else {
            None
        };
        for (key, value) in args {
            input
                .as_ref()
                .ok_or(F::NativeFailure)?
                .Put(PCWSTR(wide(key).as_ptr()), 0, &value.0, 0)
                .map_err(error)?;
        }
        let mut output = None;
        s.ExecMethod(
            &BSTR::from(path.as_str()),
            &BSTR::from(name),
            WBEM_GENERIC_FLAG_TYPE(0),
            None,
            input.as_ref(),
            Some(&mut output),
            None,
        )
        .map_err(error)?;
        let output = output.ok_or(F::NativeFailure)?;
        match number(&output, "ReturnValue")? {
            0 => Ok(output),
            5 | 0x80070005 => Err(F::PermissionDenied),
            _ => Err(F::NativeFailure),
        }
    }
}
fn string_value(s: &str) -> Value {
    Value(VARIANT::from(s))
}
fn ids(s: &IWbemServices, v: &IWbemClassObject) -> Result<Vec<String>, F> {
    strings(
        &method(
            s,
            v,
            "GetKeyProtectors",
            &[("KeyProtectorType", Value(VARIANT::from(3u32)))],
        )?,
        "VolumeKeyProtectorID",
    )
}
pub(super) fn collect() -> Result<Inventory, F> {
    let (_apartment, s) = connect()?;
    let mut materials = vec![];
    let mut failure = None;
    for v in volumes(&s)? {
        // VolumeType 0 = OS, 1 = fixed data; removable data is outside this capability.
        if number(&v, "VolumeType")? > 1 {
            continue;
        }
        let volume = text(&v, "DeviceID")?;
        let result = (|| {
            if number(&method(&s, &v, "GetLockStatus", &[])?, "LockStatus")? != 0 {
                return Err(F::NativeFailure);
            }
            let protectors = ids(&s, &v)?;
            if protectors.is_empty() {
                return Err(F::NoMaterials);
            }
            for protector in protectors {
                let id = uuid::Uuid::parse_str(protector.trim_matches(['{', '}']))
                    .map_err(|_| F::NativeFailure)?;
                let output = method(
                    &s,
                    &v,
                    "GetKeyProtectorNumericalPassword",
                    &[("VolumeKeyProtectorID", string_value(&protector))],
                )?;
                let secret = text(&output, "NumericalPassword")?;
                materials.push(Material {
                    subject: Subject::Bitlocker {
                        volume: volume.to_string(),
                        protector: id,
                    },
                    secret,
                });
                if materials.len() > 512 {
                    return Err(F::NativeFailure);
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            failure = Some(e)
        }
    }
    materials.sort_by(|a, b| a.subject.cmp(&b.subject));
    Ok(Inventory { materials, failure })
}
fn selected(s: &IWbemServices, volume: &str) -> Result<IWbemClassObject, F> {
    for v in volumes(s)? {
        if text(&v, "DeviceID")?.as_str() == volume && number(&v, "VolumeType")? <= 1 {
            return Ok(v);
        }
    }
    Err(F::TargetMissing)
}
pub(super) fn create(volume: &str, secret: &str) -> Result<(), F> {
    let (_a, s) = connect()?;
    let v = selected(&s, volume)?;
    method(
        &s,
        &v,
        "ProtectKeyWithNumericalPassword",
        &[
            ("FriendlyName", string_value("RSS recovery")),
            ("NumericalPassword", string_value(secret)),
        ],
    )?;
    Ok(())
}
pub(super) fn remove(volume: &str, protector: uuid::Uuid) -> Result<(), F> {
    let (_a, s) = connect()?;
    let v = selected(&s, volume)?;
    let old = ids(&s, &v)?
        .into_iter()
        .find(|id| uuid::Uuid::parse_str(id.trim_matches(['{', '}'])).ok() == Some(protector));
    if let Some(old) = old {
        method(
            &s,
            &v,
            "DeleteKeyProtector",
            &[("VolumeKeyProtectorID", string_value(&old))],
        )?;
    }
    if ids(&s, &v)?
        .iter()
        .any(|id| uuid::Uuid::parse_str(id.trim_matches(['{', '}'])).ok() == Some(protector))
    {
        return Err(F::NativeFailure);
    }
    Ok(())
}
