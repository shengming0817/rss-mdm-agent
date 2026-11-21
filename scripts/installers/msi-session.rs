//! Immediate Windows Installer custom action. Give this execute sequence its own operation
//! identifier before scheduling the deferred actions and observing the native result.
//! ref: Microsoft msiquery.h (MsiSetPropertyW), combaseapi.h (CoCreateGuid, StringFromGUID2).
#[repr(C)]
#[derive(Default)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoCreateGuid(value: *mut Guid) -> i32;
    fn StringFromGUID2(value: *const Guid, text: *mut u16, capacity: i32) -> i32;
}
#[link(name = "msi")]
unsafe extern "system" {
    fn MsiSetPropertyW(install: u32, name: *const u16, value: *const u16) -> u32;
}

#[unsafe(no_mangle)]
pub extern "system" fn RssBeginInstallation(install: u32) -> u32 {
    let mut guid = Guid::default();
    let mut value = [0u16; 39];
    let name: Vec<u16> = "RssInstallationOperation"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // Windows Installer owns the session handle for this synchronous callback.
    unsafe {
        if CoCreateGuid(&mut guid) < 0 || StringFromGUID2(&guid, value.as_mut_ptr(), 39) != 39 {
            return 1603;
        }
        MsiSetPropertyW(install, name.as_ptr(), value.as_ptr())
    }
}
