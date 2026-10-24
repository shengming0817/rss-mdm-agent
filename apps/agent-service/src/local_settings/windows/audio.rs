//! Read default current-login endpoints through Core Audio; no default-device setter.
//! ref: microsoft/windows-rs@0.61.3 Win32/Media/Audio/{IMMDeviceEnumerator,Endpoints/IAudioEndpointVolume}.
use super::*;
use windows::Win32::{
    Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
    Media::Audio::{Endpoints::IAudioEndpointVolume, *},
    System::Com::{
        StructuredStorage::{PropVariantClear, PropVariantToStringAlloc},
        *,
    },
};
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
fn name(device: &IMMDevice) -> Result<String, Error> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }.map_err(|_| Error::Unavailable)?;
    let mut value =
        unsafe { store.GetValue(&PKEY_Device_FriendlyName) }.map_err(|_| Error::Unavailable)?;
    let result = unsafe { PropVariantToStringAlloc(&value) };
    let _ = unsafe { PropVariantClear(&mut value) };
    let pointer = result.map_err(|_| Error::Unavailable)?;
    let name = unsafe { pointer.to_string() }.map_err(|_| Error::Protocol);
    unsafe {
        CoTaskMemFree(Some(pointer.0.cast()));
    }
    name
}
pub(super) fn read() -> Result<Vec<SettingsTarget>, Error> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .map_err(|_| Error::Unavailable)?;
    let _apartment = Apartment;
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| Error::Unavailable)?;
    let mut fields = vec![];
    for (key, flow) in [("output", eRender), ("input", eCapture)] {
        match unsafe { enumerator.GetDefaultAudioEndpoint(flow, eMultimedia) } {
            Ok(device) => {
                let label = name(&device).ok();
                fields.push(field(
                    key,
                    label.map(|s| json!(s)),
                    Some("无法读取设备名称"),
                ));
                if key == "output" {
                    let endpoint: Result<IAudioEndpointVolume, _> =
                        unsafe { device.Activate(CLSCTX_INPROC_SERVER, None) };
                    let volume = endpoint
                        .ok()
                        .and_then(|v| unsafe { v.GetMasterVolumeLevelScalar() }.ok())
                        .filter(|v| v.is_finite() && (0.0..=1.0).contains(v));
                    fields.push(field(
                        "volume",
                        volume.map(|v| json!(v)),
                        Some("当前端点不提供可确认的主音量"),
                    ));
                }
            }
            Err(_) => fields.push(field(key, None, Some("未取得默认音频设备"))),
        }
    }
    Ok(vec![target(
        "audio",
        "当前音频",
        fields,
        false,
        Some("请通过系统音频设置修改"),
    )])
}
