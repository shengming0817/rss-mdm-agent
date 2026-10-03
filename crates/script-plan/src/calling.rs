use crate::{ScriptPlanError, ScriptProfile};
use execution_contract::{Id, LaunchArg, VersionedRef};

impl ScriptProfile {
    fn id(self) -> &'static str {
        match self {
            Self::PowerShell7 => "native-pwsh7-file",
            Self::PosixSh => "native-posix-sh-file",
            Self::Bash => "native-bash-file",
        }
    }
    fn prefix(self) -> &'static [&'static str] {
        match self {
            Self::PowerShell7 => &["-NoLogo", "-NoProfile", "-NonInteractive", "-File"],
            Self::PosixSh => &[],
            Self::Bash => &["--noprofile", "--norc"],
        }
    }
    /// Exact convention identity. It does not authorize an interpreter or platform.
    pub fn reference(self) -> VersionedRef {
        VersionedRef {
            id: Id::new(self.id()).expect("static script profile ID"),
            revision: Id::new("1").expect("static script profile revision"),
        }
    }
    /// Identify an exact supported convention; unknown revisions never fall back.
    pub fn from_reference(reference: &VersionedRef) -> Result<Self, ScriptPlanError> {
        if reference.revision.as_str() != "1" {
            return Err(ScriptPlanError::Profile);
        }
        [Self::PowerShell7, Self::PosixSh, Self::Bash]
            .into_iter()
            .find(|profile| profile.id() == reference.id.as_str())
            .ok_or(ScriptPlanError::Profile)
    }
    pub(crate) fn file_argv(self, arguments: Vec<String>) -> Vec<LaunchArg> {
        self.prefix()
            .iter()
            .map(|value| LaunchArg::Literal {
                value: (*value).into(),
            })
            .chain(Some(LaunchArg::ArtifactPath {}))
            .chain(
                arguments
                    .into_iter()
                    .map(|value| LaunchArg::Literal { value }),
            )
            .collect()
    }
    pub(crate) fn prefix_len(self) -> usize {
        self.prefix().len() + 1
    }
    /// File argv for a host-owned tool. The host still verifies its path and materials.
    /// No Task, software invocation or execution authority is manufactured here.
    pub fn materialized_file_argv(self, script_path: &str, arguments: &[String]) -> Vec<String> {
        self.prefix()
            .iter()
            .map(|value| (*value).to_owned())
            .chain(Some(script_path.to_owned()))
            .chain(arguments.iter().cloned())
            .collect()
    }
    /// Independently check the startup prefix and this attempt's verified material path.
    /// Trailing script arguments remain literal and are never parsed or rewritten.
    pub fn matches_materialized_file_argv(self, args: &[String], script_path: &str) -> bool {
        let prefix = self.prefix();
        args.len() >= self.prefix_len()
            && args
                .iter()
                .zip(prefix)
                .all(|(arg, expected)| arg == expected)
            && args[prefix.len()] == script_path
    }
}
