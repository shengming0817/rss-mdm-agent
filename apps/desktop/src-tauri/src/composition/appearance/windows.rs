// ref: Microsoft DWM_SYSTEMBACKDROP_TYPE (Windows 11 build 22621+);
// microsoft/windows-rs crates/libs/windows/src/Windows/{UI/ViewManagement,Win32/Graphics/Dwm}@0.61.3.
use super::Preferences;
use tauri::{Runtime, WebviewWindow};
use windows::{
    Win32::{
        Graphics::Dwm::{
            DwmSetWindowAttribute, DWMSBT_MAINWINDOW, DWMSBT_NONE, DWMWA_SYSTEMBACKDROP_TYPE,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
        },
        UI::{
            Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
            WindowsAndMessaging::{
                SystemParametersInfoW, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
            },
        },
    },
    UI::ViewManagement::UISettings,
};
pub(super) fn probe<R: Runtime>(window: &WebviewWindow<R>) -> Result<Preferences, ()> {
    let settings = UISettings::new().map_err(|_| ())?;
    let mut contrast = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    // SAFETY: size and writable pointer correspond to HIGHCONTRASTW for this read-only SPI action.
    unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            contrast.cbSize,
            Some((&mut contrast as *mut HIGHCONTRASTW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .map_err(|_| ())?;
    Ok(Preferences {
        supported: windows_version::OsVersion::current().build >= 22621,
        reduce_transparency: !settings.AdvancedEffectsEnabled().map_err(|_| ())?,
        reduced_motion: !settings.AnimationsEnabled().map_err(|_| ())?,
        high_contrast: contrast.dwFlags.contains(HCF_HIGHCONTRASTON),
        dark: window.theme().map_err(|_| ())? == tauri::Theme::Dark,
    })
}
fn set<R: Runtime, T>(
    window: &WebviewWindow<R>,
    attribute: windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE,
    value: &T,
) -> Result<(), ()> {
    let hwnd = window.hwnd().map_err(|_| ())?;
    // SAFETY: live Tauri window handle and the typed value/size selected by our closed callsites.
    unsafe {
        DwmSetWindowAttribute(
            windows::Win32::Foundation::HWND(hwnd.0),
            attribute,
            (value as *const T).cast(),
            std::mem::size_of::<T>() as u32,
        )
    }
    .map_err(|_| ())
}
pub(super) fn apply<R: Runtime>(window: &WebviewWindow<R>, dark: bool) -> Result<(), ()> {
    set(window, DWMWA_USE_IMMERSIVE_DARK_MODE, &i32::from(dark))?;
    set(window, DWMWA_SYSTEMBACKDROP_TYPE, &DWMSBT_MAINWINDOW)
}
pub(super) fn clear<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), ()> {
    if windows_version::OsVersion::current().build < 22621 {
        return Ok(());
    }
    set(window, DWMWA_SYSTEMBACKDROP_TYPE, &DWMSBT_NONE)
}
