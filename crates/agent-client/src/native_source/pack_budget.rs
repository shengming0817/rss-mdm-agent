//! Resource preflight only; Git remains the object/hash/delta validation authority.
//! ref: git/git v2.51.0 Documentation/gitformat-pack.adoc; flate2 1.1.10 bufread.rs.
use super::{Error, MAX_DOCUMENT_BYTES, MAX_PACK_BYTES};
use std::io::Read;

fn byte(input: &[u8], position: &mut usize) -> Result<u8, Error> {
    let value = *input.get(*position).ok_or(Error::Protocol)?;
    *position += 1;
    Ok(value)
}
fn size(input: &[u8], position: &mut usize) -> Result<u64, Error> {
    let mut result = 0u64;
    for shift in (0..=56).step_by(7) {
        let value = byte(input, position)?;
        result |= u64::from(value & 127) << shift;
        if value & 128 == 0 {
            return Ok(result);
        }
    }
    Err(Error::Capacity)
}
fn add(total: &mut u64, length: u64) -> Result<(), Error> {
    if length > MAX_DOCUMENT_BYTES as u64 {
        return Err(Error::Capacity);
    }
    *total = total.checked_add(length).ok_or(Error::Capacity)?;
    if *total > MAX_PACK_BYTES as u64 {
        return Err(Error::Capacity);
    }
    Ok(())
}
pub(super) fn check(pack: &[u8]) -> Result<(), Error> {
    if pack.len() < 32 || !matches!(u32::from_be_bytes(pack[4..8].try_into().unwrap()), 2 | 3) {
        return Err(Error::Protocol);
    }
    let count = u32::from_be_bytes(pack[8..12].try_into().unwrap());
    // Also bounds any delta dependency chain, including unreachable objects and REF deltas.
    if count == 0 || count > 512 {
        return Err(Error::Capacity);
    }
    let end = pack.len() - 20;
    let input = &pack[..end];
    let mut position = 12;
    let (mut inflated, mut resolved) = (0, 0);
    for _ in 0..count {
        let first = byte(input, &mut position)?;
        let kind = (first >> 4) & 7;
        let mut length = u64::from(first & 15);
        let mut next = first;
        let mut shift = 4;
        while next & 128 != 0 {
            if shift > 60 {
                return Err(Error::Capacity);
            }
            next = byte(input, &mut position)?;
            length = length
                .checked_add(
                    u64::from(next & 127)
                        .checked_mul(1u64.checked_shl(shift).ok_or(Error::Capacity)?)
                        .ok_or(Error::Capacity)?,
                )
                .ok_or(Error::Capacity)?;
            shift += 7;
        }
        add(&mut inflated, length)?;
        match kind {
            1..=4 => (),
            6 => {
                // Offset value and reference validity are checked by Git; framing is bounded here.
                for index in 0..10 {
                    let value = byte(input, &mut position)?;
                    if value & 128 == 0 {
                        break;
                    }
                    if index == 9 {
                        return Err(Error::Capacity);
                    }
                }
            }
            7 => {
                position = position
                    .checked_add(20)
                    .filter(|p| *p <= end)
                    .ok_or(Error::Protocol)?;
            }
            _ => return Err(Error::Protocol),
        }
        let mut decoder =
            flate2::bufread::ZlibDecoder::new(input.get(position..).ok_or(Error::Protocol)?);
        let mut prefix = Vec::with_capacity(18);
        let mut decoded = 0u64;
        let mut buffer = [0u8; 8192];
        loop {
            let read = decoder.read(&mut buffer).map_err(|_| Error::Protocol)?;
            if read == 0 {
                break;
            }
            decoded = decoded.checked_add(read as u64).ok_or(Error::Capacity)?;
            if decoded > length {
                return Err(Error::Capacity);
            }
            let keep = (18 - prefix.len()).min(read);
            prefix.extend_from_slice(&buffer[..keep]);
        }
        if decoded != length {
            return Err(Error::Protocol);
        }
        let consumed = usize::try_from(decoder.total_in()).map_err(|_| Error::Capacity)?;
        if consumed == 0 {
            return Err(Error::Protocol);
        }
        position = position
            .checked_add(consumed)
            .filter(|p| *p <= end)
            .ok_or(Error::Protocol)?;
        if kind >= 6 {
            let mut cursor = 0;
            if size(&prefix, &mut cursor)? > MAX_DOCUMENT_BYTES as u64 {
                return Err(Error::Capacity);
            }
            add(&mut resolved, size(&prefix, &mut cursor)?)?;
        } else {
            add(&mut resolved, length)?;
        }
    }
    if position != end {
        return Err(Error::Protocol);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn object(kind: u8, declared: u64, bytes: &[u8]) -> Vec<u8> {
        let mut length = declared >> 4;
        let mut result =
            vec![(kind << 4) | (declared as u8 & 15) | if length != 0 { 128 } else { 0 }];
        while length != 0 {
            let value = length as u8 & 127;
            length >>= 7;
            result.push(value | if length != 0 { 128 } else { 0 });
        }
        if kind == 7 {
            result.extend_from_slice(&[1; 20]);
        }
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(bytes).unwrap();
        result.extend(encoder.finish().unwrap());
        result
    }
    fn pack(objects: &[Vec<u8>]) -> Vec<u8> {
        [
            b"PACK".as_slice(),
            &2u32.to_be_bytes(),
            &(objects.len() as u32).to_be_bytes(),
            &objects.concat(),
            &[0; 20],
        ]
        .concat()
    }
    #[test]
    fn all_objects_are_budgeted_before_git_including_unreachable_and_delta_results() {
        assert!(matches!(
            check(&pack(&[object(3, MAX_DOCUMENT_BYTES as u64 + 1, b"small")])),
            Err(Error::Capacity)
        ));
        assert!(matches!(
            check(&pack(&[object(3, 1, &vec![0; 1024 * 1024])])),
            Err(Error::Capacity)
        ));
        let large = object(3, 6 * 1024 * 1024, &vec![0; 6 * 1024 * 1024]);
        assert!(matches!(
            check(&pack(&[large.clone(), large.clone(), large])),
            Err(Error::Capacity)
        ));
        // base=1, result=16MiB: the delta instructions themselves are tiny.
        assert!(matches!(
            check(&pack(&[object(7, 5, &[1, 128, 128, 128, 8])])),
            Err(Error::Capacity)
        ));
        assert!(matches!(
            check(&pack(&vec![object(3, 1, b"x"); 513])),
            Err(Error::Capacity)
        ));
        assert!(check(&pack(&[object(3, 3, b"abc")])).is_ok());
    }
}
