//! Native payload architecture, independent of the installer shell's architecture.
use super::Rejected;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    WindowsAmd64,
    WindowsArm64,
    MacosArm64,
}
impl Target {
    pub fn architecture(self) -> &'static str {
        match self {
            Self::WindowsAmd64 => "x64",
            Self::WindowsArm64 | Self::MacosArm64 => "arm64",
        }
    }
    pub fn platform(self) -> &'static str {
        match self {
            Self::WindowsAmd64 | Self::WindowsArm64 => "win32",
            Self::MacosArm64 => "darwin",
        }
    }
    /// Check the native OS and reject an emulated service or mixed native payload.
    pub fn current() -> Result<Self, Rejected> {
        #[cfg(windows)]
        {
            // ref: Microsoft IsWow64Process2, pNativeMachine. No environment inference.
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn IsWow64Process2(
                    process: *mut std::ffi::c_void,
                    emulated: *mut u16,
                    native: *mut u16,
                ) -> i32;
            }
            let (mut emulated, mut native) = (0, 0);
            // SAFETY: -1 is the documented current-process pseudo handle; outputs are live.
            if unsafe { IsWow64Process2(-1isize as *mut _, &mut emulated, &mut native) } == 0
                || emulated != 0
            {
                return Err(Rejected);
            }
            let value = Self::from_machine(native)?;
            if value.architecture()
                != if cfg!(target_arch = "x86_64") {
                    "x64"
                } else if cfg!(target_arch = "aarch64") {
                    "arm64"
                } else {
                    "unsupported"
                }
            {
                return Err(Rejected);
            }
            Ok(value)
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            Ok(Self::MacosArm64)
        }
        #[cfg(not(any(windows, all(target_os = "macos", target_arch = "aarch64"))))]
        {
            Err(Rejected)
        }
    }
    pub fn from_machine(machine: u16) -> Result<Self, Rejected> {
        match machine {
            0x8664 => Ok(Self::WindowsAmd64),
            0xaa64 => Ok(Self::WindowsArm64),
            _ => Err(Rejected),
        }
    }
    pub fn verify_image(self, path: &Path) -> Result<(), Rejected> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = std::fs::File::open(path).map_err(|_| Rejected)?;
        let mut header = [0u8; 64];
        file.read_exact(&mut header).map_err(|_| Rejected)?;
        if self == Self::MacosArm64 {
            if u32::from_le_bytes(header[..4].try_into().unwrap()) != 0xfeedfacf
                || u32::from_le_bytes(header[4..8].try_into().unwrap()) != 0x0100000c
            {
                return Err(Rejected);
            }
        } else {
            if &header[..2] != b"MZ" {
                return Err(Rejected);
            }
            let offset = u32::from_le_bytes(header[60..64].try_into().unwrap());
            if !(64..=1024 * 1024).contains(&offset) {
                return Err(Rejected);
            }
            file.seek(SeekFrom::Start(u64::from(offset)))
                .map_err(|_| Rejected)?;
            let mut pe = [0u8; 6];
            file.read_exact(&mut pe).map_err(|_| Rejected)?;
            if &pe[..4] != b"PE\0\0"
                || Self::from_machine(u16::from_le_bytes(pe[4..6].try_into().unwrap()))? != self
            {
                return Err(Rejected);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_architectures_and_mixed_payload_rejection() {
        assert_eq!(Target::from_machine(0x8664).unwrap(), Target::WindowsAmd64);
        assert_eq!(Target::from_machine(0xaa64).unwrap(), Target::WindowsArm64);
        assert!(Target::from_machine(0x14c).is_err());
        let path = std::env::temp_dir().join(format!("rss-target-{}", std::process::id()));
        let mut bytes = vec![0u8; 70];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[68..70].copy_from_slice(&0xaa64u16.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert!(Target::WindowsArm64.verify_image(&path).is_ok());
        assert!(Target::WindowsAmd64.verify_image(&path).is_err());
        assert!(Target::MacosArm64.verify_image(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
