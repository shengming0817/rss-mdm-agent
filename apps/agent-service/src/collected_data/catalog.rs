//! Product-owned read recipes; no sampling cadence or remote Policy is created here.
use super::*;
use rss_mdm_agent_wire::CollectionScope;

impl Data {
    pub fn install_user(&mut self, subject: &str, session: uuid::Uuid) -> Result<(), Error> {
        if subject == "0" || crate::backend::plan::platform()? != Platform::Macos {
            return Ok(());
        }
        let mut recipes = vec![w::NativeQueryDefinition::MacosApplications {
            root: w::MacosApplicationRoot::UserApplications,
            max_depth: 4,
        }];
        for prefix in ["/opt/homebrew", "/usr/local"] {
            for package in [w::MacosBrewPackage::Formula, w::MacosBrewPackage::Cask] {
                recipes.push(w::NativeQueryDefinition::MacosBrew {
                    prefix: prefix.into(),
                    package,
                    taps: Default::default(),
                });
            }
        }
        for definition in recipes {
            let budget = w::NativeBudget {
                timeout_seconds: 60,
                output_bytes: 2 * 1024 * 1024,
                max_rows: 4096,
            };
            let query = w::NativeQuery {
                version: definition.engine().contract_version(),
                definition,
                cardinality: w::NativeCardinality::List,
                instance_keys: vec!["id".into()],
                budget,
            };
            let plan = w::NativeCollectionPlan::new(w::NativePlanSpec {
                version: 1,
                scope: CollectionScope::User {
                    identity: subject.into(),
                },
                queries: [("read".into(), query)].into(),
                budget,
            })?;
            let d = self.register(ReadPlan {
                source: crate::collection_worker::ReadSource::local(&plan)?,
                platform: plan.platform(),
                plan,
                deadline: 0,
                execution: w::NativeExecutionBinding {
                    identity: w::ExecutionIdentity::LoggedInUser,
                    context_revision: 1,
                    target: w::SoftwareExecutionTarget::User {
                        identity: subject.into(),
                        session_id: session,
                    },
                    generated_uid: crate::account_identity::generated_uid(subject)?,
                },
            })?;
            self.pin(&d.descriptor.reference, &d.read);
        }
        Ok(())
    }
    pub fn install_local(&mut self) -> Result<(), Error> {
        self.install_platform(crate::backend::plan::platform()?)
    }
    pub(super) fn install_platform(&mut self, platform: Platform) -> Result<(), Error> {
        use w::NativeQueryDefinition as Q;
        let mut recipes = Vec::new();
        match platform {
            Platform::Macos => {
                use w::MacosHardwareSection::*;
                for section in [
                    Identity,
                    Os,
                    Cpu,
                    Memory,
                    Firmware,
                    Disks,
                    ApfsContainers,
                    Volumes,
                    Network,
                    Battery,
                ] {
                    recipes.push((
                        format!("hardware.{section:?}"),
                        Q::MacosHardware { section },
                        section.cardinality(),
                        vec!["id"],
                    ));
                }
                for section in [
                    w::MacosSecuritySection::Filevault,
                    w::MacosSecuritySection::Firewall,
                ] {
                    let query = w::NativeSystemQuery::MacosSecurity { section };
                    let cardinality = query.cardinality();
                    recipes.push((
                        format!("security.{section:?}"),
                        Q::System { query },
                        cardinality,
                        vec![],
                    ));
                }
                for root in [
                    w::MacosApplicationRoot::LocalApplications,
                    w::MacosApplicationRoot::SystemApplications,
                ] {
                    recipes.push((
                        format!("applications.{root:?}"),
                        Q::MacosApplications { root, max_depth: 4 },
                        w::NativeCardinality::List,
                        vec!["id"],
                    ));
                }
                recipes.push((
                    "package receipts".into(),
                    Q::MacosPkgReceipts {},
                    w::NativeCardinality::List,
                    vec!["id"],
                ));
            }
            Platform::Windows => {
                recipes.extend([
                    (
                        "CPU".into(),
                        Q::WindowsCpu,
                        w::NativeCardinality::ExactlyOne,
                        vec![],
                    ),
                    (
                        "software installations".into(),
                        Q::WindowsSoftware,
                        w::NativeCardinality::List,
                        vec!["id"],
                    ),
                    (
                        "network interfaces".into(),
                        Q::WindowsNetwork,
                        w::NativeCardinality::List,
                        vec!["id"],
                    ),
                    (
                        "battery".into(),
                        Q::WindowsBattery,
                        w::NativeCardinality::ExactlyOne,
                        vec![],
                    ),
                ]);
                for (path, name) in [
                    (r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "EditionID"),
                    (
                        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
                        "PROCESSOR_ARCHITECTURE",
                    ),
                ] {
                    recipes.push((
                        name.into(),
                        Q::WindowsRegistry {
                            hive: w::RegistryHive::LocalMachine,
                            view: w::RegistryView::Registry64,
                            path: path.into(),
                            enumerate_subkeys: false,
                            values: [(
                                "value".into(),
                                w::RegistryValue {
                                    name: name.into(),
                                    value_type: w::RegistryType::Sz,
                                },
                            )]
                            .into(),
                        },
                        w::NativeCardinality::ExactlyOne,
                        vec![],
                    ));
                }
                for query in [
                    w::NativeSystemQuery::WindowsBitlocker,
                    w::NativeSystemQuery::WindowsFirewall,
                    w::NativeSystemQuery::WindowsDefender,
                ] {
                    let cardinality = query.cardinality();
                    recipes.push((
                        format!("security.{query:?}"),
                        Q::System { query },
                        cardinality,
                        vec!["id"],
                    ));
                }
                use w::CimType::*;
                for (title, class, fields) in [
                    (
                        "memory",
                        "Win32_ComputerSystem",
                        vec![("TotalPhysicalMemory", Uint64)],
                    ),
                    (
                        "hardware UUID",
                        "Win32_ComputerSystemProduct",
                        vec![("UUID", String)],
                    ),
                    ("serial", "Win32_BIOS", vec![("SerialNumber", String)]),
                    (
                        "system",
                        "Win32_ComputerSystem",
                        vec![("Model", String), ("Manufacturer", String)],
                    ),
                    (
                        "OS",
                        "Win32_OperatingSystem",
                        vec![
                            ("Caption", String),
                            ("Version", String),
                            ("BuildNumber", String),
                            ("SystemDrive", String),
                        ],
                    ),
                    (
                        "firmware",
                        "Win32_BIOS",
                        vec![("SMBIOSBIOSVersion", String)],
                    ),
                    (
                        "disks",
                        "Win32_DiskDrive",
                        vec![("DeviceID", String), ("Model", String), ("Size", Uint64)],
                    ),
                    (
                        "volumes",
                        "Win32_LogicalDisk",
                        vec![
                            ("DeviceID", String),
                            ("Size", Uint64),
                            ("FreeSpace", Uint64),
                            ("DriveType", Uint32),
                        ],
                    ),
                ] {
                    let list = matches!(class, "Win32_DiskDrive" | "Win32_LogicalDisk");
                    recipes.push((
                        title.into(),
                        Q::WindowsWmi {
                            namespace: r"root\cimv2".into(),
                            class: class.into(),
                            properties: fields
                                .into_iter()
                                .map(|(name, cim_type)| {
                                    (
                                        name.into(),
                                        w::WmiProperty {
                                            cim_type,
                                            array: false,
                                        },
                                    )
                                })
                                .collect(),
                            filter: None,
                        },
                        if list {
                            w::NativeCardinality::List
                        } else {
                            w::NativeCardinality::ExactlyOne
                        },
                        if list { vec!["DeviceID"] } else { vec![] },
                    ));
                }
            }
            Platform::Linux => return Ok(()),
        }
        for (title, definition, cardinality, keys) in recipes {
            let budget = w::NativeBudget {
                timeout_seconds: 60,
                output_bytes: if matches!(definition, w::NativeQueryDefinition::System { .. }) {
                    w::NATIVE_SYSTEM_MAX_BYTES
                } else {
                    2 * 1024 * 1024
                },
                max_rows: 4096,
            };
            let query = w::NativeQuery {
                version: definition.engine().contract_version(),
                definition,
                cardinality,
                instance_keys: if cardinality == w::NativeCardinality::List {
                    keys.into_iter().map(str::to_owned).collect()
                } else {
                    vec![]
                },
                budget,
            };
            let plan = w::NativeCollectionPlan::new(w::NativePlanSpec {
                version: 1,
                scope: CollectionScope::Device,
                queries: [("read".into(), query)].into(),
                budget,
            })?;
            let read = ReadPlan {
                source: crate::collection_worker::ReadSource::local(&plan)?,
                platform: plan.platform(),
                plan,
                deadline: 0,
                execution: w::NativeExecutionBinding {
                    identity: w::ExecutionIdentity::System,
                    context_revision: 1,
                    target: w::SoftwareExecutionTarget::Device,
                    generated_uid: None,
                },
            };
            let definition = self.register(read)?;
            let _ = title;
            self.pin(&definition.descriptor.reference, &definition.read);
        }
        Ok(())
    }
}
