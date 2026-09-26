//! Installation ACL policy. OS ancestors permit sibling creation, never replacement.
// ref: Microsoft Win32 File Access Rights Constants / ACE Inheritance Rules.
const REPLACE: u32 = 0x0001_0000 | 0x40 | 0x0004_0000 | 0x0008_0000;
const WRITE: u32 = REPLACE | 0x2 | 0x4 | 0x10 | 0x100;
const SYSTEM: &str = "S-1-5-18";
const ADMINISTRATORS: &str = "S-1-5-32-544";
const INSTALLER: &str = "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464";

pub(crate) fn trusted(subject: &str, product: bool) -> bool {
    matches!(subject, SYSTEM | ADMINISTRATORS) || (!product && subject == INSTALLER)
}

pub(crate) fn grant_allowed(
    subject: &str,
    mut mask: u32,
    flags: u8,
    directory: bool,
    product: bool,
) -> bool {
    // Generic rights are mapped before judging their effect on this file object.
    for (generic, specific) in [
        (0x1000_0000, 0x001f_01ff),
        (0x4000_0000, 0x0012_0116),
        (0x8000_0000, 0x0012_0089),
        (0x2000_0000, 0x0012_00a0),
    ] {
        if mask & generic != 0 {
            mask = (mask & !generic) | specific;
        }
    }
    let effective = flags & 0x08 == 0; // INHERIT_ONLY_ACE
    let propagated = directory && flags & 0x03 != 0; // OBJECT/CONTAINER_INHERIT_ACE
    if !(effective || product && propagated) {
        return true;
    }
    trusted(subject, product) || mask & if product { WRITE } else { REPLACE } == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    const USER: &str = "S-1-5-21-1-2-3-1001";
    #[test]
    fn ancestors_allow_sibling_creation_but_never_replacement() {
        for mask in [0x2, 0x4, 0x10, 0x100, 0x4000_0000, 0x8000_0000] {
            assert!(grant_allowed(USER, mask, 0, true, false));
        }
        for mask in [0x10000, 0x40, 0x40000, 0x80000, 0x1000_0000] {
            assert!(!grant_allowed(USER, mask, 0, true, false));
            assert!(grant_allowed(SYSTEM, mask, 0, true, false));
        }
        assert!(grant_allowed("S-1-3-0", 0x1000_0000, 0x0b, true, false));
        assert!(!trusted(USER, false));
        assert!(trusted(INSTALLER, false));
        assert!(!trusted(INSTALLER, true));
    }
    #[test]
    fn product_rejects_effective_and_inherited_writers() {
        for mask in [
            0x2,
            0x4,
            0x10,
            0x100,
            0x10000,
            0x40,
            0x40000,
            0x80000,
            0x4000_0000,
            0x1000_0000,
        ] {
            assert!(!grant_allowed(USER, mask, 0, false, true));
            assert!(!grant_allowed(USER, mask, 0x0b, true, true));
            assert!(grant_allowed(ADMINISTRATORS, mask, 0x0b, true, true));
        }
        assert!(grant_allowed(USER, 0x8000_0000, 0, false, true));
        assert!(grant_allowed(USER, 0x2, 0x08, false, true));
    }
}
