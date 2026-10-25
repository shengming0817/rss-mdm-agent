use super::*;
use ring::{
    rand::SystemRandom,
    signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING},
};
#[test]
fn request_and_uv_proofs_verify_original_bytes_and_fail_closed() {
    let rng = SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
    let key =
        EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng).unwrap();
    let mut blob = Vec::from(0x31534345u32.to_le_bytes());
    blob.extend(32u32.to_le_bytes());
    blob.extend(&key.public_key().as_ref()[1..]);
    let message = b"original OS encoded request";
    let proof = key.sign(&rng, message).unwrap();
    signature::verify(&blob, message, proof.as_ref()).unwrap();
    assert_eq!(
        signature::verify(&blob, b"another RP/request", proof.as_ref()),
        Err(Error::Authentication)
    );
    blob[8] ^= 1;
    assert_eq!(
        signature::verify(&blob, message, proof.as_ref()),
        Err(Error::Authentication)
    );
    assert_eq!(
        signature::verify(&[], message, proof.as_ref()),
        Err(Error::Authentication)
    );
}
#[test]
fn cancelled_completed_and_shutdown_contexts_never_reauthorize() {
    let operations = Operations::default();
    let first = operations
        .begin([1; 16], OperationKind::Create, b"one")
        .unwrap();
    assert!(first.matches(OperationKind::Create, b"one"));
    assert!(!first.matches(OperationKind::Assert, b"one"));
    assert!(matches!(
        operations.begin([2; 16], OperationKind::Assert, b"two"),
        Err(Error::Busy)
    ));
    assert_eq!(operations.cancel([9; 16]), Err(Error::NotFound));
    operations.cancel([1; 16]).unwrap();
    assert_eq!(first.check(), Err(Error::Cancelled));
    assert_eq!(operations.finish(&first), Err(Error::Cancelled));
    assert!(operations
        .begin([1; 16], OperationKind::Create, b"three")
        .is_err());
    assert!(operations
        .begin([3; 16], OperationKind::Create, b"one")
        .is_err());
    let next = operations
        .begin([4; 16], OperationKind::Assert, b"four")
        .unwrap();
    operations.shutdown();
    assert_eq!(operations.finish(&next), Err(Error::Cancelled));
    assert!(operations.is_idle());
    assert!(operations
        .begin([5; 16], OperationKind::Assert, b"five")
        .is_err());
}
#[test]
fn bounded_ctap_decoder_rejects_duplicate_keys_trailing_bytes_and_deep_values() {
    let input = [0xa2, 0x01, 0x01, 0x01, 0x02];
    assert!(protocol::Request::decode(OperationKind::Assert, &input).is_err());
    let mut deep = vec![0x81; 18];
    deep.push(0xf6);
    assert!(protocol::Request::decode(OperationKind::Create, &deep).is_err());
    assert!(protocol::Request::decode(OperationKind::Assert, &[0xa0, 0x00]).is_err());
    assert!(protocol::Request::decode(OperationKind::Create, &[0xbf, 0xff]).is_err());
}
#[test]
fn ctap_map_round_trips_without_inventing_an_origin() {
    use passkey_types::ctap2::get_assertion;
    let input = get_assertion::Request {
        rp_id: "example.com".into(),
        client_data_hash: vec![7; 32].into(),
        allow_list: None,
        extensions: None,
        options: get_assertion::Options { up: true, uv: true },
        pin_auth: None,
        pin_protocol: None,
    };
    let bytes = protocol::encode(&input).unwrap();
    let decoded = protocol::Request::decode(OperationKind::Assert, &bytes).unwrap();
    assert_eq!(decoded.rp(), "example.com");
    assert!(matches!(decoded, protocol::Request::Assert(_)));
    assert!(protocol::Request::decode(OperationKind::Create, &bytes).is_err());
}
