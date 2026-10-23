//! Product-owned read recipes; no sampling cadence or remote Policy is created here.
use super::*;
use rss_mdm_agent_wire::CollectionScope;

impl Data {
    pub fn install_user(&mut self, subject: &str, session: uuid::Uuid) -> Result<(), Error> {
        if crate::backend::plan::platform()? != Platform::Macos {
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
                budget: budget.clone(),
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
            self.pin(&d.descriptor.reference);
        }
        Ok(())
    }
    pub fn install_local(&mut self) -> Result<(), Error> {
        use w::NativeQueryDefinition as Q;
        let mut recipes = Vec::new();
        match crate::backend::plan::platform()? {
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
                use w::CimType::*;
                for (title, class, fields) in [
                    (
                        "system",
                        "Win32_ComputerSystem",
                        vec![
                            ("Model", String),
                            ("Manufacturer", String),
                            ("TotalPhysicalMemory", Uint64),
                        ],
                    ),
                    (
                        "OS",
                        "Win32_OperatingSystem",
                        vec![
                            ("Caption", String),
                            ("Version", String),
                            ("BuildNumber", String),
                        ],
                    ),
                    (
                        "firmware",
                        "Win32_BIOS",
                        vec![("SerialNumber", String), ("SMBIOSBIOSVersion", String)],
                    ),
                    (
                        "disks",
                        "Win32_DiskDrive",
                        vec![("DeviceID", String), ("Model", String), ("Size", Uint64)],
                    ),
                ] {
                    let list = class == "Win32_DiskDrive";
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
                output_bytes: 2 * 1024 * 1024,
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
                budget: budget.clone(),
            };
            let plan = w::NativeCollectionPlan::new(w::NativePlanSpec {
                version: 1,
                scope: CollectionScope::Device,
                queries: [("read".into(), query)].into(),
                budget,
            })?;
            let read = ReadPlan {
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
            self.pin(&definition.descriptor.reference);
        }
        Ok(())
    }
}
