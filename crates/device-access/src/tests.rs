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
