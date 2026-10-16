use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("rss-brew-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn write(&self, path: &str, data: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, data).unwrap();
    }
    fn scan(&self, kind: w::MacosBrewPackage, cap: usize) -> Result<w::NativeQueryResult> {
        self.scan_selected(kind, cap, BTreeSet::new())
    }
    fn scan_selected(
        &self,
        kind: w::MacosBrewPackage,
        cap: usize,
        taps: BTreeSet<String>,
    ) -> Result<w::NativeQueryResult> {
        let q = super::super::tests::query(w::NativeQueryDefinition::MacosBrew {
            prefix: "/opt/homebrew".into(),
            package: kind,
            taps: taps.clone(),
        });
        let input = super::super::tests::input(q.clone());
        let c = Context {
            input: &input,
            until: Instant::now() + Duration::from_secs(10),
            bytes: Cell::new(0),
            cap,
        };
        let mut scan = Scan {
            context: &c,
            root: absolute(&self.0)?,
            seen: Vec::new(),
        };
        let mut rows = Rows::new(&q, now().unwrap());
        let uid = unsafe { libc::geteuid() };
        installed(
            &mut scan,
            "/opt/homebrew",
            "501", // Synthetic row owner; filesystem fixture ownership is checked separately.
            uid,
            kind,
            &taps,
            &mut rows,
        )?;
        scan.verify()?;
        let result = rows.finish(Ok(()));
        assert!(result.complete_for(&q, now().unwrap()), "{result:?}");
        Ok(result)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn formula_receipts_retain_versions_and_never_execute_formula_code() {
    let f = Fixture::new();
    for version in ["1.2_1", "2.0"] {
        f.write(
            &format!("Cellar/example/{version}/INSTALL_RECEIPT.json"),
            r#"{"source":{"tap":"homebrew/core"},"shell":"touch sentinel"}"#,
        );
        f.write(
            &format!("Cellar/example/{version}/.brew/example.rb"),
            "raise 'must not execute'",
        );
    }
    let result = f.scan(w::MacosBrewPackage::Formula, 1_000_000).unwrap();
    assert_eq!(result.rows.len(), 2);
    assert_eq!(result.rows[0]["version"], "1.2_1");
    assert_eq!(result.rows[1]["version"], "2.0");
    assert_ne!(result.rows[0]["id"], result.rows[1]["id"]);
    assert_eq!(result.rows[0]["tap"], "homebrew/core");
    assert!(!f.0.join("sentinel").exists());
}
#[test]
fn cask_installed_receipt_and_marker_must_agree() {
    let f = Fixture::new();
    f.write(
        "Caskroom/example/.metadata/INSTALL_RECEIPT.json",
        r#"{"source":{"tap":"homebrew/cask","version":"2.0"}}"#,
    );
    let marker = "Caskroom/example/.metadata/2.0/20261015120000.000/Casks/example.json";
    f.write(marker, "{}");
    let result = f.scan(w::MacosBrewPackage::Cask, 1_000_000).unwrap();
    assert_eq!(result.rows[0]["version"], "2.0");
    f.write(marker, r#"{"unknown_layout":true}"#);
    assert!(matches!(
        f.scan(w::MacosBrewPackage::Cask, 1_000_000),
        Err(Fault::Query(Status::Unsupported))
    ));
    f.write(marker, r#"{"version":"999"}"#);
    assert!(f.scan(w::MacosBrewPackage::Cask, 1_000_000).is_err());
    std::fs::remove_file(f.0.join(marker)).unwrap();
    f.write(
        "Caskroom/example/.metadata/2.0/20261015120000.000/Casks/example.rb",
        "raise 'never load'",
    );
    assert!(matches!(
        f.scan(w::MacosBrewPackage::Cask, 1_000_000),
        Err(Fault::Query(Status::Unsupported))
    ));
}
#[test]
fn missing_corrupt_unknown_and_oversized_receipts_are_not_empty_success() {
    let f = Fixture::new();
    assert!(f.scan(w::MacosBrewPackage::Formula, 1_000_000).is_err());
    for bad in [
        "{}",
        "{",
        r#"{"source":{"tap":null}}"#,
        r#"{"source":{"tap":"../core"}}"#,
    ] {
        f.write("Cellar/example/1/INSTALL_RECEIPT.json", bad);
        assert!(f.scan(w::MacosBrewPackage::Formula, 1_000_000).is_err());
    }
    f.write(
        "Cellar/example/1/INSTALL_RECEIPT.json",
        &" ".repeat(MAX_RECEIPT + 1),
    );
    assert!(matches!(
        f.scan(w::MacosBrewPackage::Formula, 2_000_000),
        Err(Fault::Query(Status::Truncated))
    ));
    f.write(
        "Cellar/example/1/INSTALL_RECEIPT.json",
        r#"{"source":{"tap":"homebrew/core"}}"#,
    );
    assert!(matches!(
        f.scan(w::MacosBrewPackage::Formula, 10),
        Err(Fault::Query(Status::Truncated))
    ));
}
#[test]
fn symlink_receipts_and_ancestors_are_rejected() {
    let f = Fixture::new();
    f.write(
        "outside/INSTALL_RECEIPT.json",
        r#"{"source":{"tap":"homebrew/core"}}"#,
    );
    std::fs::create_dir_all(f.0.join("Cellar/example/1")).unwrap();
    std::os::unix::fs::symlink(
        f.0.join("outside/INSTALL_RECEIPT.json"),
        f.0.join("Cellar/example/1/INSTALL_RECEIPT.json"),
    )
    .unwrap();
    assert!(f.scan(w::MacosBrewPackage::Formula, 1_000_000).is_err());
    std::fs::remove_dir_all(f.0.join("Cellar/example")).unwrap();
    std::os::unix::fs::symlink(f.0.join("outside"), f.0.join("Cellar/example")).unwrap();
    assert!(f.scan(w::MacosBrewPackage::Formula, 1_000_000).is_err());
}
#[test]
fn mutation_journal_detects_replacement_after_read() {
    let f = Fixture::new();
    f.write("receipt.json", "{}");
    let q = super::super::tests::query(w::NativeQueryDefinition::MacosBrew {
        prefix: "/opt/homebrew".into(),
        package: w::MacosBrewPackage::Formula,
        taps: BTreeSet::new(),
    });
    let input = super::super::tests::input(q);
    let c = Context {
        input: &input,
        until: Instant::now() + Duration::from_secs(10),
        bytes: Cell::new(0),
        cap: 1_000_000,
    };
    let mut scan = Scan {
        context: &c,
        root: absolute(&f.0).unwrap(),
        seen: Vec::new(),
    };
    scan.json(&["receipt.json".into()]).unwrap();
    f.write("receipt.json", "{\"changed\":true}");
    assert!(matches!(scan.verify(), Err(Fault::Query(Status::Invalid))));
    let mut scan = Scan {
        context: &c,
        root: absolute(&f.0).unwrap(),
        seen: Vec::new(),
    };
    scan.json(&["receipt.json".into()]).unwrap();
    f.write("replacement", "{\"changed\":true}");
    std::fs::rename(f.0.join("replacement"), f.0.join("receipt.json")).unwrap();
    assert!(matches!(scan.verify(), Err(Fault::Query(Status::Invalid))));
}

#[test]
fn existing_empty_roots_are_complete_but_missing_roots_are_not() {
    let f = Fixture::new();
    for (root, kind) in [
        ("Cellar", w::MacosBrewPackage::Formula),
        ("Caskroom", w::MacosBrewPackage::Cask),
    ] {
        assert!(matches!(
            f.scan(kind, 1_000_000),
            Err(Fault::Query(Status::Missing))
        ));
        std::fs::create_dir(f.0.join(root)).unwrap();
        assert!(f.scan(kind, 1_000_000).unwrap().rows.is_empty());
    }
}
#[test]
fn tap_selection_is_exact_and_filters_before_emitting_rows() {
    let f = Fixture::new();
    for (name, tap) in [
        ("core", "homebrew/core"),
        ("custom", "other/core"),
        ("similar", "homebrew/core-extra"),
    ] {
        f.write(
            &format!("Cellar/{name}/1/INSTALL_RECEIPT.json"),
            &serde_json::json!({"source":{"tap":tap}}).to_string(),
        );
    }
    let result = f
        .scan_selected(
            w::MacosBrewPackage::Formula,
            1_000_000,
            BTreeSet::from(["homebrew/core".into()]),
        )
        .unwrap();
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0]["native_id"], "homebrew/core/core");
    assert!(f
        .scan_selected(
            w::MacosBrewPackage::Formula,
            1_000_000,
            BTreeSet::from(["missing/tap".into()])
        )
        .unwrap()
        .rows
        .is_empty());
}
#[test]
fn malformed_tap_cannot_be_filtered_into_complete_empty_inventory() {
    let f = Fixture::new();
    f.write(
        "Cellar/example/1/INSTALL_RECEIPT.json",
        r#"{"source":{"tap":"homebrew/ bad"}}"#,
    );
    assert!(matches!(
        f.scan_selected(
            w::MacosBrewPackage::Formula,
            1_000_000,
            ["homebrew/core".into()].into()
        ),
        Err(Fault::Query(Status::Invalid))
    ));
}
#[test]
fn installation_identity_is_stable_and_isolates_all_provenance_dimensions() {
    let make = |prefix: &str, owner: &str, kind, version: &str, tap: &str| {
        let q = super::super::tests::query(w::NativeQueryDefinition::MacosBrew {
            prefix: prefix.into(),
            package: kind,
            taps: BTreeSet::new(),
        });
        let mut rows = Rows::new(&q, now().unwrap());
        emit(
            &serde_json::json!({"source":{"tap":tap}}),
            prefix,
            owner,
            "example",
            version,
            kind,
            &BTreeSet::new(),
            &mut rows,
        )
        .unwrap();
        let result = rows.finish(Ok(()));
        assert!(result.complete_for(&q, now().unwrap()));
        result.rows[0]["id"].as_str().unwrap().to_owned()
    };
    let formula = w::MacosBrewPackage::Formula;
    let first = make("/opt/homebrew", "501", formula, "1", "homebrew/core");
    assert_eq!(
        first,
        make("/opt/homebrew", "501", formula, "1", "homebrew/core")
    );
    let ids = BTreeSet::from([
        first,
        make("/usr/local", "501", formula, "1", "homebrew/core"),
        make("/opt/homebrew", "502", formula, "1", "homebrew/core"),
        make(
            "/opt/homebrew",
            "501",
            w::MacosBrewPackage::Cask,
            "1",
            "homebrew/core",
        ),
        make("/opt/homebrew", "501", formula, "2", "homebrew/core"),
        make("/opt/homebrew", "501", formula, "1", "other/core"),
    ]);
    assert_eq!(ids.len(), 6);
}
#[test]
fn production_collect_enforces_current_nonroot_user_without_tools() {
    let f = Fixture::new();
    f.write(
        "Cellar/example/1/INSTALL_RECEIPT.json",
        r#"{"source":{"tap":"homebrew/core"}}"#,
    );
    let prefix = f.0.to_str().unwrap();
    let q = super::super::tests::query(w::NativeQueryDefinition::MacosBrew {
        prefix: prefix.into(),
        package: w::MacosBrewPackage::Formula,
        taps: BTreeSet::new(),
    });
    let mut input = super::super::tests::input(q.clone());
    assert!(input.tools.is_empty());
    let uid = unsafe { libc::geteuid() };
    let attempt = |input: &Input| {
        let c = Context {
            input,
            until: Instant::now() + Duration::from_secs(10),
            bytes: Cell::new(0),
            cap: 1_000_000,
        };
        let mut rows = Rows::new(&q, now().unwrap());
        collect(
            &c,
            prefix,
            w::MacosBrewPackage::Formula,
            &BTreeSet::new(),
            &mut rows,
        )?;
        Ok::<_, Fault>(rows.finish(Ok(())))
    };
    // Never impersonate a nonroot OS user when the test process itself is root.
    if uid != 0 {
        let result = attempt(&input).unwrap();
        assert!(result.complete_for(&q, now().unwrap()));
        assert_eq!(result.rows[0]["owner"], uid.to_string());
    } else {
        assert!(matches!(attempt(&input), Err(Fault::Query(Status::Denied))));
    }
    for identity in ["0".to_owned(), uid.saturating_add(1).to_string()] {
        input.task.execution.target = w::SoftwareExecutionTarget::User {
            identity,
            session_id: uuid::Uuid::new_v4(),
        };
        assert!(matches!(attempt(&input), Err(Fault::Query(Status::Denied))));
    }
    input.task.execution.target = w::SoftwareExecutionTarget::Device;
    assert!(matches!(attempt(&input), Err(Fault::Query(Status::Denied))));
}
