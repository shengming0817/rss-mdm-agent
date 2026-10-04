//! Native lifetime evidence is independent of installation outcome diagnostics.
//! ref: Microsoft StartServiceW / QueryServiceStatusEx (winsvc.h).
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkerEvidence {
    version: u8,
    pub process_id: Option<u32>,
    pub start_error: Option<u32>,
}
impl WorkerEvidence {
    pub fn process(process_id: u32) -> Self {
        Self {
            version: 1,
            process_id: Some(process_id),
            start_error: None,
        }
    }
    pub fn not_started(error: u32) -> Option<Self> {
        // Only documented failures that prevent process creation. In particular,
        // timeout (1053) and failure to create the service thread (1054) are unknown.
        matches!(error, 2 | 3 | 5 | 193 | 1055 | 1058 | 1068 | 1069 | 1075).then_some(Self {
            version: 1,
            process_id: None,
            start_error: Some(error),
        })
    }
    pub fn valid(&self) -> bool {
        self.version == 1
            && match (self.process_id, self.start_error) {
                (Some(pid), None) => pid != 0,
                (None, Some(error)) => Self::not_started(error).is_some(),
                _ => false,
            }
    }
    pub fn reclaimable(&self, stopped: bool, process_exited: bool) -> bool {
        self.valid() && stopped && (self.start_error.is_some() || process_exited)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_exit_authority_survives_missing_business_outcomes() {
        let crash = WorkerEvidence::process(42);
        assert!(crash.reclaimable(true, true));
        assert!(!crash.reclaimable(true, false));
        assert!(!crash.reclaimable(false, true));
        assert!(WorkerEvidence::not_started(2)
            .unwrap()
            .reclaimable(true, false));
        assert!(WorkerEvidence::not_started(193)
            .unwrap()
            .reclaimable(true, false));
        let logon_failure = WorkerEvidence::not_started(1069).unwrap();
        assert!(logon_failure.reclaimable(true, false));
        assert!(!logon_failure.reclaimable(false, false));
        for ambiguous in [0, 1053, 1054, 1056, 12345] {
            assert!(WorkerEvidence::not_started(ambiguous).is_none());
        }
        for invalid in [
            r#"{"version":1,"process_id":null,"start_error":null}"#,
            r#"{"version":1,"process_id":42,"start_error":1069}"#,
            r#"{"version":1,"process_id":0,"start_error":null}"#,
            r#"{"version":1,"process_id":null,"start_error":1053}"#,
            r#"{"version":2,"process_id":42,"start_error":null}"#,
        ] {
            let evidence: WorkerEvidence = serde_json::from_str(invalid).unwrap();
            assert!(!evidence.reclaimable(true, true));
        }
        assert!(serde_json::from_str::<WorkerEvidence>(
            r#"{"version":1,"process_id":42,"start_error":null,"terminal":true}"#
        )
        .is_err());
    }
}
