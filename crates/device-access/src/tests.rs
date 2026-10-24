use super::*;
#[test]
fn rfc6238_sha1_vectors_truncated_to_six_digits() {
    let secret = b"12345678901234567890";
    for (time, expected) in [
        (59, "287082"),
        (1111111109, "081804"),
        (1111111111, "050471"),
        (1234567890, "005924"),
        (2000000000, "279037"),
        (20000000000, "353130"),
    ] {
        assert_eq!(otp(secret, time / 30), expected);
        assert_eq!(matching_step(secret, expected, time, -1), Some(time / 30));
        assert_eq!(matching_step(secret, expected, time, time / 30), None);
    }
}
#[test]
fn confirmation_consumes_highest_matching_step_and_rejects_bad_format() {
    let secret = b"12345678901234567890";
    let code = otp(secret, 1001);
    assert_eq!(matching_step(secret, &code, 30000, -1), Some(1001));
    assert_eq!(matching_step(secret, &code, 30000, 1001), None);
    for value in ["", "12345", "1234567", "１２３４５６", "12345 "] {
        assert_eq!(matching_step(secret, value, 30000, -1), None);
    }
}
#[test]
fn recovery_encoding_is_canonical_256_bits() {
    let code = URL_SAFE_NO_PAD.encode([255; 32]);
    assert!(valid_recovery(&code));
    assert!(!valid_recovery(&(code.clone() + "=")));
    assert!(!valid_recovery(&code[..42]));
    assert!(constant_equal(b"a", b"a"));
    assert!(!constant_equal(b"a", b"b"));
}

#[test]
fn cloud_sid_object_coordinate_is_canonical_but_does_not_supply_a_tenant() {
    let id = uuid::Uuid::parse_str("00112233-4455-6677-8899-aabbccddeeff").unwrap();
    let bytes = id.to_bytes_le();
    let words: Vec<_> = bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()).to_string())
        .collect();
    let sid = format!("S-1-12-1-{}", words.join("-"));
    assert_eq!(super::entra_object_id(&sid), Some(id));
    for bad in [
        "S-1-5-21-1-2-3-4",
        "S-1-12-1-1-2-3",
        "S-1-12-1-01-2-3-4",
        "S-1-12-1-0-0-0-0",
        "S-1-12-1-1-2-3-4294967296",
    ] {
        assert_eq!(super::entra_object_id(bad), None);
    }
}
