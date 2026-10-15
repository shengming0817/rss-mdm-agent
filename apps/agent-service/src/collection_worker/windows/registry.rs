//! ref: microsoft/windows-rs@0.61.0 Win32/System/Registry/mod.rs.
use super::*;
use ::windows::{
    core::{PCWSTR, PWSTR},
    Win32::{Foundation::*, System::Registry::*},
};
pub(super) struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
fn status(code: WIN32_ERROR) -> Status {
    match code {
        ERROR_ACCESS_DENIED => Status::Denied,
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => Status::Missing,
        ERROR_MORE_DATA => Status::Truncated,
        _ => Status::Invalid,
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub(super) fn open(hive: HKEY, path: &str, view: w::RegistryView) -> Result<Key, Status> {
    let path = wide(path);
    let mut key = HKEY::default();
    let view = match view {
        w::RegistryView::Registry32 => KEY_WOW64_32KEY,
        w::RegistryView::Registry64 => KEY_WOW64_64KEY,
    };
    // SAFETY: fixed local hive, terminated path, only read/enumeration rights.
    let code =
        unsafe { RegOpenKeyExW(hive, PCWSTR(path.as_ptr()), None, KEY_READ | view, &mut key) };
    if code != ERROR_SUCCESS {
        return Err(status(code));
    }
    Ok(Key(key))
}
pub(super) fn machine(path: &str, view: w::RegistryView) -> Result<Key, Status> {
    open(HKEY_LOCAL_MACHINE, path, view)
}
pub(super) fn user(sid: &str, path: &str, view: w::RegistryView) -> Result<Key, Status> {
    open(
        HKEY_USERS,
        &if path.is_empty() {
            sid.into()
        } else {
            format!("{sid}\\{path}")
        },
        view,
    )
}
pub(super) fn value(
    key: &Key,
    name: &str,
    kind: w::RegistryType,
) -> Result<serde_json::Value, Status> {
    let name = wide(name);
    let mut size = 0;
    let mut typ = REG_VALUE_TYPE::default();
    let code = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut typ),
            None,
            Some(&mut size),
        )
    };
    if code != ERROR_SUCCESS {
        return Err(status(code));
    }
    if size > 131072 {
        return Err(Status::Truncated);
    }
    let mut bytes = vec![0u8; size as usize];
    let code = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut typ),
            Some(bytes.as_mut_ptr()),
            Some(&mut size),
        )
    };
    if code != ERROR_SUCCESS {
        return Err(status(code));
    }
    if size as usize > bytes.len() {
        return Err(Status::Invalid);
    }
    bytes.truncate(size as usize);
    super::super::values::registry(kind, typ.0, &bytes)
}
pub(super) fn names(key: &Key, budget: &Budget, max: usize) -> Result<Vec<String>, Status> {
    let mut names = Vec::new();
    let mut unique = std::collections::BTreeSet::new();
    for index in 0..=max {
        budget.check()?;
        let mut buffer = [0u16; 256];
        let mut len = buffer.len() as u32;
        let code = unsafe {
            RegEnumKeyExW(
                key.0,
                index as u32,
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
        };
        if code == ERROR_NO_MORE_ITEMS {
            return Ok(names);
        }
        if code != ERROR_SUCCESS {
            return Err(status(code));
        }
        if index == max {
            return Err(Status::Truncated);
        }
        if len as usize >= buffer.len() {
            return Err(Status::Invalid);
        }
        let name = String::from_utf16(&buffer[..len as usize]).map_err(|_| Status::Invalid)?;
        if !unique.insert(name.to_lowercase()) {
            return Err(Status::Invalid);
        }
        names.push(name);
    }
    Err(Status::Truncated)
}
pub(super) fn read(
    def: &w::NativeQueryDefinition,
    budget: &Budget,
    mut emit: impl FnMut(Row) -> Result<(), Status>,
) -> Result<(), Status> {
    def.validate().map_err(|_| Status::Invalid)?;
    let w::NativeQueryDefinition::WindowsRegistry {
        hive,
        view,
        path,
        enumerate_subkeys,
        values,
    } = def
    else {
        return Err(Status::Invalid);
    };
    let open_path = |path: &str| match hive {
        w::RegistryHive::LocalMachine => machine(path, *view),
        w::RegistryHive::Users { sid } => user(sid, path, *view),
    };
    let root = open_path(path)?;
    let mut row = |key: &Key, name: Option<&str>| -> Result<(), Status> {
        let mut row = Row::new();
        if let Some(name) = name {
            row.insert("key".into(), name.into());
        }
        for (field, v) in values {
            budget.check()?;
            match value(key, &v.name, v.value_type) {
                Ok(v) => {
                    row.insert(field.clone(), v);
                }
                Err(Status::Missing) => (), // Omitted property remains Missing in the server mapping.
                Err(e) => return Err(e),
            }
        }
        emit(row)
    };
    if *enumerate_subkeys {
        let before = names(&root, budget, 65535)?;
        for name in &before {
            budget.check()?;
            let child = open_path(&format!("{path}\\{name}"))?;
            row(&child, Some(name))?;
        }
        let after = names(&root, budget, 65535)?;
        if before
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            != after.into_iter().collect()
        {
            return Err(Status::Invalid);
        }
    } else {
        row(&root, None)?;
    }
    Ok(())
}
