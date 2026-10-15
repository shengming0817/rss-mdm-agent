//! Synthetic contract-consumption samples, not Windows provider/COM or asset-mapping proof.
//! ref: microsoft/wmi server23h2/root/standardcimv2/MSFT_NetFirewallProfile.go
//! ref: microsoft/wmi server23h2/root/microsoft/windows/deviceguard/Win32_DeviceGuard.go
use super::*;
use w::CimType::{Boolean, String as Text, Uint16, Uint32};

fn wmi_query(
    namespace: &str,
    class: &str,
    properties: &[(&str, w::CimType, bool)],
    keys: &[&str],
) -> w::NativeQuery {
    w::NativeQuery {
        definition: w::NativeQueryDefinition::WindowsWmi {
            namespace: namespace.into(),
            class: class.into(),
            properties: properties
                .iter()
                .map(|(name, cim_type, array)| {
                    (
                        (*name).into(),
                        w::WmiProperty {
                            cim_type: *cim_type,
                            array: *array,
                        },
                    )
                })
                .collect(),
            filter: None,
        },
        cardinality: if keys.is_empty() {
            w::NativeCardinality::ExactlyOne
        } else {
            w::NativeCardinality::List
        },
        instance_keys: keys.iter().map(|k| (*k).into()).collect(),
        budget: w::NativeBudget {
            max_rows: 4,
            ..query().budget
        },
        version: 1,
    }
}

fn sample(q: &w::NativeQuery, input: &[Row]) -> w::NativeQueryResult {
    let p = w::NativeCollectionPlan::new(w::NativePlanSpec {
        version: 1,
        scope: rss_mdm_agent_wire::CollectionScope::Device,
        queries: [("sample".into(), q.clone())].into(),
        budget: w::NativeBudget {
            output_bytes: 8192,
            ..q.budget
        },
    })
    .unwrap();
    let time = now().unwrap();
    let mut result = collect(&p, time + 30, (time as u64 + 30) * 1000, |_, q, _| {
        let mut rows = Rows::new(q, time);
        let end = input.iter().cloned().try_for_each(|row| rows.push(row));
        Ok(rows.finish(end))
    })
    .unwrap();
    let result = result.queries.remove("sample").unwrap();
    assert_eq!(result.query_digest, q.digest());
    assert_eq!(result.collected_at, time);
    assert_eq!(result.row_count as usize, result.rows.len());
    result
}

fn row(value: serde_json::Value) -> Row {
    serde_json::from_value(value).unwrap()
}

#[test]
fn bitlocker_keeps_conversion_protection_and_algorithm_per_volume() {
    // https://learn.microsoft.com/en-us/windows/win32/secprov/win32-encryptablevolume
    let q = wmi_query(
        r"root\cimv2\Security\MicrosoftVolumeEncryption",
        "Win32_EncryptableVolume",
        &[
            ("DeviceID", Text, false),
            ("ConversionStatus", Uint32, false),
            ("ProtectionStatus", Uint32, false),
            ("EncryptionMethod", Uint32, false),
        ],
        &["DeviceID"],
    );
    assert_eq!(
        select(&q.definition).unwrap(),
        "SELECT ConversionStatus,DeviceID,EncryptionMethod,ProtectionStatus FROM Win32_EncryptableVolume"
    );
    let input = vec![
        row(
            json!({"DeviceID": "volume-a", "ConversionStatus": cim_value(Uint32, CimValue::I32(1)).unwrap(),
            "ProtectionStatus": cim_value(Uint32, CimValue::I32(0)).unwrap(), "EncryptionMethod": 7}),
        ),
        row(
            json!({"DeviceID": "volume-b", "ConversionStatus": 4, "ProtectionStatus": 2, "EncryptionMethod": 65535}),
        ),
        row(json!({"DeviceID": "volume-c"})),
    ];
    let result = sample(&q, &input);
    assert!(result.complete_for(&q, now().unwrap()));
    assert_eq!(result.rows, input); // Missing properties stay absent; no lock/progress is invented.
    assert_eq!(result.rows[0]["ConversionStatus"], json!(1));
    assert_eq!(result.rows[0]["ProtectionStatus"], json!(0));
}

#[test]
fn firewall_profiles_keep_uint16_states_instead_of_health_booleans() {
    let q = wmi_query(
        r"root\StandardCimv2",
        "MSFT_NetFirewallProfile",
        &[("Name", Text, false), ("Enabled", Uint16, false)],
        &["Name"],
    );
    assert_eq!(
        select(&q.definition).unwrap(),
        "SELECT Enabled,Name FROM MSFT_NetFirewallProfile"
    );
    let input: Vec<_> = [("Domain", 1), ("Private", 2), ("Public", 65535)]
        .into_iter()
        .map(|(name, state)| {
            row(json!({"Name": name,
            "Enabled": cim_value(Uint16, CimValue::I32(state)).unwrap()}))
        })
        .collect();
    let result = sample(&q, &input);
    assert!(result.complete_for(&q, now().unwrap()));
    assert_eq!(result.rows, input);
    assert_eq!(result.rows[0]["Enabled"], json!(1));
    assert_eq!(result.rows[1]["Enabled"], json!(2));
    assert_eq!(result.rows[2]["Enabled"], json!(65535));
    let invalid = sample(&q, &[row(json!({"Name": "Domain", "Enabled": true}))]);
    assert_eq!(invalid.status, Status::Invalid);
    assert!(!invalid.enumeration_complete);
}

#[test]
fn device_guard_distinguishes_configured_running_empty_and_missing_arrays() {
    let q = wmi_query(
        r"root\Microsoft\Windows\DeviceGuard",
        "Win32_DeviceGuard",
        &[
            ("VirtualizationBasedSecurityStatus", Uint32, false),
            ("SecurityServicesConfigured", Uint32, true),
            ("SecurityServicesRunning", Uint32, true),
        ],
        &[],
    );
    assert_eq!(
        select(&q.definition).unwrap(),
        "SELECT SecurityServicesConfigured,SecurityServicesRunning,VirtualizationBasedSecurityStatus FROM Win32_DeviceGuard"
    );
    // The arrays are already decoded samples: this does not exercise Windows SAFEARRAY.
    for input in [
        json!({"VirtualizationBasedSecurityStatus": 2, "SecurityServicesConfigured": [1, 2], "SecurityServicesRunning": [2, 99]}),
        json!({"VirtualizationBasedSecurityStatus": 99, "SecurityServicesConfigured": [1, 2], "SecurityServicesRunning": []}),
        json!({"SecurityServicesConfigured": [1, 2]}),
    ] {
        let input = vec![row(input)];
        let result = sample(&q, &input);
        assert!(result.complete_for(&q, now().unwrap()));
        assert_eq!(result.rows, input);
    }
    let invalid = sample(&q, &[row(json!({"SecurityServicesRunning": true}))]);
    assert_eq!(invalid.status, Status::Invalid);
    assert!(!invalid.enumeration_complete);
}

#[test]
fn local_accounts_filter_locally_and_preserve_sid_and_disabled_state() {
    // https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-useraccount
    let mut q = wmi_query(
        r"root\cimv2",
        "Win32_UserAccount",
        &[
            ("SID", Text, false),
            ("Name", Text, false),
            ("LocalAccount", Boolean, false),
            ("Disabled", Boolean, false),
        ],
        &["SID"],
    );
    let w::NativeQueryDefinition::WindowsWmi { filter, .. } = &mut q.definition else {
        unreachable!()
    };
    *filter = Some(w::WmiFilter::Compare {
        property: "LocalAccount".into(),
        operator: w::NativeComparison::Eq,
        value: json!(true),
    });
    assert_eq!(
        select(&q.definition).unwrap(),
        "SELECT Disabled,LocalAccount,Name,SID FROM Win32_UserAccount WHERE (LocalAccount = TRUE)"
    );
    let input = vec![
        row(
            json!({"SID": "S-1-5-21-100-200-300-1001", "Name": "operator", "LocalAccount": true,
            "Disabled": cim_value(Boolean, CimValue::Bool(-1)).unwrap()}),
        ),
        row(
            json!({"SID": "S-1-5-21-100-200-300-1002", "Name": "operator", "LocalAccount": true,
            "Disabled": cim_value(Boolean, CimValue::Bool(0)).unwrap()}),
        ),
    ];
    let result = sample(&q, &input);
    assert!(result.complete_for(&q, now().unwrap()));
    assert_eq!(result.rows, input); // No administrator/derived enabled field is added.
    assert_eq!(result.rows[0]["Disabled"], json!(true));
    assert_eq!(result.rows[1]["Disabled"], json!(false));
    let duplicate = sample(&q, &[input[0].clone(), input[0].clone()]);
    assert_eq!(duplicate.status, Status::Invalid);
    assert!(!duplicate.enumeration_complete);
}
