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
        // Independent host retains the existing desktop's Vault-management ownership.
        let text=wide("请打开 RSS MDM Agent 的个人 Passkey 页面管理凭据。Windows Passkey 设置只管理 Provider 的启用状态。");
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                wide("个人 Passkey 管理").as_ptr(),
                MB_OK,
            );
        }
        Ok(())
    }
}
