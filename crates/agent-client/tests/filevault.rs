use agent_client::{filevault::ReadError, OpenMode};
use base64::{engine::general_purpose::STANDARD, Engine};
use execution_contract::FilevaultState as Status;
use rss_mdm_agent_wire::filevault as w;
use sha2::{Digest, Sha256};
use uuid::Uuid;
#[path = "../../../tests/agent-protocol/mod.rs"]
mod support;
use support::*;
#[tokio::test]
async fn independent_cms_upload_preserves_pending_across_ack_loss_restart_and_certificate_change() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    assert_eq!(
        client
            .drive_filevault(|| panic!("disabled must not read system material"))
            .await
            .unwrap(),
        Status::NotConfigured
    );
    let recipient = certificate(1);
    let cms = carrier(&recipient, 1);
    let config = configuration(&recipient);
    {
        let mut d = server.data.lock().unwrap();
        d.filevault.configuration = Some(config.clone());
        d.filevault.pending_probe = Some(root.path.join("communication.sqlite"));
        d.filevault.lose_ack = true;
    }
    client.refresh_filevault().unwrap();
    assert!(client.drive_filevault(|| Ok(cms.clone())).await.is_err());
    assert_eq!(client.filevault_status().unwrap(), Status::Pending);
    let original = server.data.lock().unwrap().filevault.uploads[0].clone();
    assert_eq!(
        client
            .drive_filevault(|| panic!("unknown pending must freeze the original carrier"))
            .await
            .unwrap(),
        Status::Pending
    );
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client.refresh_filevault().unwrap();
    server.time.set(31);
    server.data.lock().unwrap().filevault.deny_status = true;
    assert!(client
        .drive_filevault(|| panic!("a denied status query cannot replace unknown pending"))
        .await
        .is_err());
    server.time.set(61);
    server.data.lock().unwrap().filevault.deny_status = false;
    assert_eq!(
        client
            .drive_filevault(|| panic!("ACK recovery must not read a changed file"))
            .await
            .unwrap(),
        Status::EscrowedUnverified
    );
    assert_eq!(server.data.lock().unwrap().filevault.uploads.len(), 1);
    let newer = carrier(&recipient, 2);
    server.time.set(121);
    server.data.lock().unwrap().filevault.reject_upload = true;
    assert_eq!(
        client.drive_filevault(|| Ok(newer.clone())).await.unwrap(),
        Status::Rejected
    );
    let rejected = server.data.lock().unwrap().filevault.uploads[1].operation_id;
    server.time.set(181);
    assert_eq!(
        client.drive_filevault(|| Ok(newer.clone())).await.unwrap(),
        Status::Rejected
    );
    assert_eq!(server.data.lock().unwrap().filevault.uploads.len(), 2);
    {
        let mut d = server.data.lock().unwrap();
        d.filevault.configuration.as_mut().unwrap().revision += 1;
        d.filevault.reject_upload = false;
    }
    server.time.set(241);
    assert_eq!(
        client.drive_filevault(|| Ok(newer.clone())).await.unwrap(),
        Status::EscrowedUnverified
    );
    {
        let d = server.data.lock().unwrap();
        assert_ne!(d.filevault.uploads[2].operation_id, rejected);
        assert_ne!(d.filevault.uploads[2].operation_id, original.operation_id);
    }
    let rotated = certificate(2);
    {
        let mut d = server.data.lock().unwrap();
        let c = d.filevault.configuration.as_mut().unwrap();
        c.revision += 1;
        c.recipient_id = Some(format!("{:x}", Sha256::digest(&rotated)));
        c.recipient_certificate = Some(STANDARD.encode(&rotated));
    }
    server.time.set(301);
    assert_eq!(
        client.drive_filevault(|| Ok(newer)).await.unwrap(),
        Status::RecipientMismatch
    );
    assert_eq!(server.data.lock().unwrap().filevault.uploads.len(), 3);
    server.time.set(361);
    assert_eq!(
        client
            .drive_filevault(|| Err(ReadError::Missing))
            .await
            .unwrap(),
        Status::MissingMaterial
    );
    assert!(server.data.lock().unwrap().employee_calls.is_empty());
}

#[tokio::test]
async fn mismatched_receipt_cannot_rebind_a_frozen_operation_to_a_new_custody_epoch() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let cert = certificate(3);
    let bytes = carrier(&cert, 3);
    {
        let mut d = server.data.lock().unwrap();
        d.filevault.configuration = Some(configuration(&cert));
        d.filevault.wrong_ack = true;
    }
    assert!(matches!(
        client.drive_filevault(|| Ok(bytes.clone())).await,
        Err(agent_client::Error::Untrusted)
    ));
    let original = server.data.lock().unwrap().filevault.uploads[0].clone();
    {
        let mut d = server.data.lock().unwrap();
        let c = d.filevault.configuration.as_mut().unwrap();
        c.custody_epoch += 1;
        c.revision += 1;
    }
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    server.time.set(31);
    assert_eq!(
        client
            .drive_filevault(|| panic!("unknown original outcome must be recovered first"))
            .await
            .unwrap(),
        Status::EscrowedUnverified
    );
    assert_eq!(server.data.lock().unwrap().filevault.uploads.len(), 1);
    server.time.set(91);
    assert_eq!(
        client.drive_filevault(|| Ok(bytes)).await.unwrap(),
        Status::EscrowedUnverified
    );
    let d = server.data.lock().unwrap();
    assert_eq!(d.filevault.uploads.len(), 2);
    assert_eq!(
        d.filevault.uploads[1].custody_epoch,
        original.custody_epoch + 1
    );
    assert_ne!(d.filevault.uploads[1].operation_id, original.operation_id);
    assert!(d.filevault.receipts[&original.operation_id].matches(&original));
}

fn configuration(certificate: &[u8]) -> w::Configuration {
    w::Configuration {
        enabled: true,
        revision: 1,
        custody_id: Some(Uuid::new_v4()),
        custody_epoch: 1,
        recipient_id: Some(format!("{:x}", Sha256::digest(certificate))),
        recipient_certificate: Some(STANDARD.encode(certificate)),
        material_revision: 0,
    }
}

/// Test-only self-signed RSA recipient, reusing disposable fixture key material.
fn certificate(serial: u8) -> Vec<u8> {
    use der::{Decode, Encode};
    let leaf = base64::engine::general_purpose::STANDARD
        .decode(pki::chain(Uuid::from_u128(91)).remove(0))
        .unwrap();
    let mut cert = x509_cert::Certificate::from_der(&leaf).unwrap();
    cert.tbs_certificate.serial_number = x509_cert::serial_number::SerialNumber::from(serial);
    cert.tbs_certificate.issuer = cert.tbs_certificate.subject.clone();
    let signature = pki::key()
        .choose_scheme(&[rustls::SignatureScheme::RSA_PKCS1_SHA256])
        .unwrap()
        .sign(&cert.tbs_certificate.to_der().unwrap())
        .unwrap();
    cert.signature = der::asn1::BitString::from_bytes(&signature).unwrap();
    cert.to_der().unwrap()
}
/// Recipient-header fixture; deliberately never decrypts or claims a real recovery key.
fn carrier(certificate: &[u8], tag: u8) -> Vec<u8> {
    use cms::{
        cert::IssuerAndSerialNumber,
        content_info::{CmsVersion, ContentInfo},
        enveloped_data::*,
    };
    use der::{
        asn1::{ObjectIdentifier, OctetString, SetOfVec},
        Any, Decode, Encode,
    };
    let cert = x509_cert::Certificate::from_der(certificate).unwrap();
    let recipient = RecipientInfo::Ktri(KeyTransRecipientInfo {
        version: CmsVersion::V0,
        rid: RecipientIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: cert.tbs_certificate.issuer,
            serial_number: cert.tbs_certificate.serial_number,
        }),
        key_enc_alg: spki::AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1"),
            parameters: None,
        },
        enc_key: OctetString::new(vec![tag; 256]).unwrap(),
    });
    let envelope = EnvelopedData {
        version: CmsVersion::V0,
        originator_info: None,
        recip_infos: RecipientInfos(SetOfVec::try_from(vec![recipient]).unwrap()),
        encrypted_content: EncryptedContentInfo {
            content_type: ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.1"),
            content_enc_alg: spki::AlgorithmIdentifierOwned {
                oid: ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.1.42"),
                parameters: Some(
                    Any::from_der(&OctetString::new(vec![0; 16]).unwrap().to_der().unwrap())
                        .unwrap(),
                ),
            },
            encrypted_content: Some(OctetString::new(vec![tag; 32]).unwrap()),
        },
        unprotected_attrs: None,
    };
    ContentInfo {
        content_type: ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.3"),
        content: Any::from_der(&envelope.to_der().unwrap()).unwrap(),
    }
    .to_der()
    .unwrap()
}
