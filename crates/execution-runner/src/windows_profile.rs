// Windows admission policy shared by preparation and materialization.
// ref: tauri-apps/plugins-workspace 11211c3 plugins/shell/src/scope.rs
use execution_app::Error;
use execution_contract::VersionedRef;
use script_plan::ScriptProfile;
pub(crate) fn profile(profile: &VersionedRef) -> Result<(), Error> {
    if profile.revision.as_str() == "1"
        && (matches!(
            ScriptProfile::from_reference(profile),
            Ok(ScriptProfile::PowerShell7)
        ) || [
            "native-osquery-template",
            "native-software-worker",
            "native-local-software-worker",
            "native-settings-worker",
            "native-network-check-worker",
            "native-collection-worker",
            "native-credential-worker",
            "native-device-access-worker",
            "native-enrollment-worker",
        ]
        .contains(&profile.id.as_str()))
    {
        Ok(())
    } else {
        Err(Error::Unsupported)
    }
}

pub(crate) fn arguments(
    profile: &VersionedRef,
    args: &[String],
    script: &std::path::Path,
) -> Result<(), Error> {
    self::profile(profile)?;
    if profile.id.as_str() == crate::osquery::PROFILE {
        return if crate::osquery::prefix_matches(args) {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    let path = script.to_str().ok_or(Error::InvalidInput)?;
    if profile.id.as_str() == "native-collection-worker" {
        return if args == ["--collection-worker", path] {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    if profile.id.as_str() == "native-enrollment-worker" {
        return if args == ["--enrollment-worker", path] {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    if profile.id.as_str() == "native-device-access-worker" {
        return if args == ["--device-access-worker", path] {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    if profile.id.as_str() == "native-credential-worker" {
        return if args == ["--credential-worker", path] {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    if profile.id.as_str() == "native-network-check-worker" {
        return if args == ["--network-check-worker", path] {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    if profile.id.as_str() == "native-settings-worker" {
        return if args == ["--settings-worker", path] {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    if profile.id.as_str() == "native-local-software-worker" {
        return if args == ["--local-software-worker", path] {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    if profile.id.as_str() == "native-software-worker" {
        return if args == ["--software-worker", path] {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    self::profile(profile)?;
    let convention = ScriptProfile::from_reference(profile).map_err(|_| Error::Unsupported)?;
    if convention.matches_materialized_file_argv(args, path) {
        Ok(())
    } else {
        Err(Error::Denied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use execution_contract::Id;
    #[test]
    fn credential_admission_requires_exact_revision_flag_and_materialized_path() {
        for (profile_id, flag) in [
            ("native-credential-worker", "--credential-worker"),
            ("native-device-access-worker", "--device-access-worker"),
            ("native-enrollment-worker", "--enrollment-worker"),
        ] {
            let mut p = VersionedRef {
                id: Id::new(profile_id).unwrap(),
                revision: Id::new("1").unwrap(),
            };
            let path = std::path::Path::new(r"C:\sessions\input.json");
            let valid = vec![flag.into(), path.to_str().unwrap().into()];
            assert!(profile(&p).is_ok());
            assert!(arguments(&p, &valid, path).is_ok());
            for args in [
                vec![],
                vec![flag.into()],
                vec!["--software-worker".into(), valid[1].clone()],
                vec![valid[0].clone(), "other.json".into()],
                vec![valid[0].clone(), valid[1].clone(), "extra".into()],
            ] {
                assert!(arguments(&p, &args, path).is_err());
            }
            p.revision = Id::new("2").unwrap();
            assert!(profile(&p).is_err());
            assert!(arguments(&p, &valid, path).is_err());
            p.id = Id::new("unknown-worker").unwrap();
            assert!(arguments(&p, &valid, path).is_err());
        }
    }
}
