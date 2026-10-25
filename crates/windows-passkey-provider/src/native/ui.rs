use super::*;
use windows_sys::Win32::UI::Controls::*;
pub struct NativeUi;
unsafe extern "system" fn callback(
    hwnd: HWND,
    message: u32,
    _w: usize,
    _l: isize,
    data: isize,
) -> i32 {
    // SAFETY: synchronous TaskDialogIndirect retains the borrowed operation until it returns.
    if message == TDN_TIMER as u32 && unsafe { &*(data as *const Operation) }.check().is_err() {
        unsafe {
            SendMessageW(hwnd, TDM_CLICK_BUTTON as u32, IDCANCEL as usize, 0);
        }
    }
    0
}
impl HostUi for NativeUi {
    fn choose(&self, request: &Context<'_>, operation: &Operation) -> Result<Option<Vec<u8>>> {
        operation.check()?;
        let title = wide("RSS Personal Passkeys");
        let instruction = wide(&format!(
            "{} · {}",
            match request.kind {
                passkey_vault::ceremony::Kind::Register => "创建 Passkey",
                passkey_vault::ceremony::Kind::Assert => "选择用于登录的账号",
            },
            request.rp_id
        ));
        let labels: Vec<Vec<u16>> = request
            .accounts
            .iter()
            .map(|a| wide(&format!("{} — {}", a.display_name, a.username)))
            .collect();
        if labels.len() > 128 {
            return Err(Error::Capacity);
        }
        let buttons: Vec<TASKDIALOG_BUTTON> = labels
            .iter()
            .enumerate()
            .map(|(i, s)| TASKDIALOG_BUTTON {
                nButtonID: 100 + i as i32,
                pszButtonText: s.as_ptr(),
            })
            .collect();
        let content=wide("操作仅对当前网站和本次请求有效。取消不会重试。\n凭据保存在此 Windows 用户的个人 Vault。");
        let mut config: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
        config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
        config.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_CALLBACK_TIMER | TDF_USE_COMMAND_LINKS;
        config.pszWindowTitle = title.as_ptr();
        config.pszMainInstruction = instruction.as_ptr();
        config.pszContent = content.as_ptr();
        config.dwCommonButtons = TDCBF_CANCEL_BUTTON;
        if matches!(request.kind, passkey_vault::ceremony::Kind::Register) {
            config.dwCommonButtons |= TDCBF_OK_BUTTON;
        } else if buttons.is_empty() {
            return Err(Error::NotFound);
        }
        config.cButtons = buttons.len() as u32;
        config.pButtons = buttons.as_ptr();
        config.pfCallback = Some(callback);
        config.lpCallbackData = operation as *const Operation as isize;
        let mut selected = 0;
        // SAFETY: all strings/button arrays and callback context outlive the modal call.
        api::hr(unsafe {
            TaskDialogIndirect(
                &config,
                &mut selected,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        })?;
        operation.check()?;
        if selected == IDCANCEL {
            return Err(Error::Cancelled);
        }
        if matches!(request.kind, passkey_vault::ceremony::Kind::Register) && selected == IDOK {
            return Ok(None);
        }
        request
            .accounts
            .get(selected.checked_sub(100).ok_or(Error::Input)? as usize)
            .map(|a| Some(a.credential_id.clone()))
            .ok_or(Error::Input)
    }
    fn open_management(&self) -> Result<()> {
        use windows_sys::Win32::{
            System::Com::CoTaskMemFree,
            UI::Shell::{FOLDERID_CommonPrograms, SHGetKnownFolderPath, ShellExecuteW},
        };
        // The existing installer owns this stable all-users shortcut; do not guess versioned paths.
        let mut value = std::ptr::null_mut();
        let path = unsafe {
            api::hr(SHGetKnownFolderPath(
                &FOLDERID_CommonPrograms,
                0,
                std::ptr::null_mut(),
                &mut value,
            ))?;
            let mut len = 0;
            while *value.add(len) != 0 {
                len += 1;
            }
            let result = String::from_utf16(std::slice::from_raw_parts(value, len))
                .map_err(|_| Error::Owner);
            CoTaskMemFree(value.cast());
            std::path::PathBuf::from(result?).join("RSS MDM Agent.lnk")
        };
        if !path.is_file() {
            unsafe {
                MessageBoxW(std::ptr::null_mut(),wide("尚未安装 RSS MDM Agent 桌面管理器。请先安装桌面并在个人 Passkey 页面创建 Vault，再启用此 Provider。个人 Vault 不要求企业登录或 Agent 在线。").as_ptr(),wide("缺少个人 Vault 管理器").as_ptr(),MB_OK|MB_ICONINFORMATION);
            }
            return Err(Error::NotFound);
        }
        unsafe {
            MessageBoxW(std::ptr::null_mut(),wide("将在 RSS MDM Agent 桌面打开管理入口；请进入个人 Passkey 页面进行锁定、删除或恢复操作。").as_ptr(),wide("个人 Passkey 管理").as_ptr(),MB_OK|MB_ICONINFORMATION);
            let result = ShellExecuteW(
                std::ptr::null_mut(),
                wide("open").as_ptr(),
                wide(&path.to_string_lossy()).as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            );
            if result as isize <= 32 {
                return Err(Error::SystemUnavailable);
            }
        }
        Ok(())
    }
}
