//! Windows owns the credential result; RSS never converts a factor success into an OS success.
pub fn message(status: i32, substatus: i32) -> Option<&'static str> {
    if status >= 0 {
        return None;
    }
    Some(
        match if substatus < 0 {
            substatus as u32
        } else {
            status as u32
        } {
            0xc000006a | 0xc000006d => "Windows 用户名或密码不正确。请重新输入密码和新的验证码。",
            0xc0000071 | 0xc0000224 => {
                "Windows 密码已到期或必须更改。请通过系统登录选项完成改密后重试。"
            }
            0xc0000072 | 0xc0000193 => "Windows 账号已停用或到期。RSS 恢复密钥不能解除账号限制。",
            0xc0000234 => "Windows 账号已锁定。请按组织的账号恢复流程处理。",
            0xc000005e => "Windows 无法联系登录服务器，且本次登录未被缓存凭据接受。",
            _ => "Windows 未接受本次登录。请使用系统登录选项查看或处理原生账号限制。",
        },
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn success_and_failure_keep_windows_authority_separate() {
        assert_eq!(message(0, 0xc0000072u32 as i32), None);
        assert!(message(0xc000006eu32 as i32, 0xc0000072u32 as i32)
            .unwrap()
            .contains("停用"));
        assert!(message(0xc0000224u32 as i32, 0).unwrap().contains("改密"));
        assert!(message(0xc000005eu32 as i32, 0).unwrap().contains("缓存"));
        assert!(message(-1, 0).unwrap().contains("未接受"));
    }
}
