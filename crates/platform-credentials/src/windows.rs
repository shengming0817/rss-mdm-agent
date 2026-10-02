use std::{
    ffi::c_void,
    io,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn crypt(bytes: &[u8], protect: bool) -> io::Result<Vec<u8>> {
    if bytes.len() > 65536 {
        return Err(io::Error::other("key size"));
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    let result = unsafe {
        if protect {
            CryptProtectData(
                &input,
                null(),
                null(),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                null_mut(),
                null(),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    let _output = Local(output.pbData.cast());
    let value =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe {
        std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
    }
    Ok(value)
}
pub fn protect_key(bytes: &[u8]) -> io::Result<Vec<u8>> {
    crypt(bytes, true)
}
pub fn unprotect_key(bytes: &[u8]) -> io::Result<Vec<u8>> {
    crypt(bytes, false)
}
pub fn random_bytes(length: usize) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0; length];
    let size = u32::try_from(length).map_err(io::Error::other)?;
    if unsafe {
        BCryptGenRandom(
            null_mut(),
            bytes.as_mut_ptr(),
            size,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } != 0
    {
        return Err(io::Error::other("system RNG"));
    }
    Ok(bytes)
}
