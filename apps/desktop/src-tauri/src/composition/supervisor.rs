//! Owns the replaceable AI Host process, incarnation and diagnostics.
use super::{
    credentials::MasterKey,
    execution::ExecutionHandle,
    host::{self, Fault, Phase, Process},
};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
enum Host {
    Closed,
    Stopped,
    Starting,
    Running(Arc<Process>),
    Failed(Fault),
}
struct HostState {
    generation: u64,
    owner: Host,
}
pub(super) struct HostSupervisor {
    state: Mutex<HostState>,
    root: PathBuf,
    artifact: PathBuf,
    source: ai_session_contract::HostStatusSource,
    version: ai_session_contract::HostStatusVersion,
    recent: Mutex<Vec<(u64, Fault, ai_session_contract::HostDiagnostic)>>,
}
fn configuration(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    use std::io::Write;
    let path = root.join("host.json");
    let value = json!({"version":1,"databasePath":root.join("ai.sqlite"),"nativeDirectory":root.join("native"),"workingDirectory":root.join("workspace")});
    let temporary = root.join(format!("host-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = platform_private_storage::create_new(&temporary)?;
    file.write_all(&serde_json::to_vec(&value)?)?;
    file.sync_all()?;
    drop(file);
    platform_private_storage::replace(&temporary, &path)?;
    Ok(path)
}
impl HostSupervisor {
    pub(super) fn new(
        root: &Path,
        artifact: &Path,
        source: ai_session_contract::HostStatusSource,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            state: Mutex::new(HostState {
                generation: 0,
                owner: Host::Stopped,
            }),
            root: root.into(),
            artifact: artifact.into(),
            source,
            version: env!("CARGO_PKG_VERSION")
                .try_into()
                .map_err(|_| "invalid application version")?,
            recent: Mutex::new(Vec::new()),
        })
    }
    pub(super) fn close(&self) {
        self.state.lock().unwrap().owner = Host::Closed;
    }
    pub fn closed(&self) -> bool {
        matches!(self.state.lock().unwrap().owner, Host::Closed)
    }
    pub(super) fn epoch(&self) -> u64 {
        self.state.lock().unwrap().generation
    }
    pub(super) fn process(&self) -> Option<Arc<Process>> {
        match &self.state.lock().unwrap().owner {
            Host::Running(process) => Some(process.clone()),
            _ => None,
        }
    }
    pub fn status(&self) -> ai_session_contract::HostStatus {
        let (generation, phase) = {
            let state = self.state.lock().unwrap();
            let phase = match &state.owner {
                Host::Closed | Host::Stopped => Phase::Stopped,
                Host::Starting => Phase::Starting,
                Host::Failed(fault) => Phase::Failed(*fault),
                Host::Running(process) => process.phase(),
            };
            (state.generation, phase)
        };
        let (phase, fault) = match phase {
            Phase::Starting => (ai_session_contract::HostStatusPhase::Starting, None),
            Phase::Ready => (ai_session_contract::HostStatusPhase::Ready, None),
            Phase::Stopping => (ai_session_contract::HostStatusPhase::Stopping, None),
            Phase::Stopped => (ai_session_contract::HostStatusPhase::Stopped, None),
            Phase::Failed(fault) => (ai_session_contract::HostStatusPhase::Failed, Some(fault)),
        };
        let mut recent = self.recent.lock().unwrap();
        if let Some(fault) = fault {
            if recent
                .last()
                .is_none_or(|(g, f, _)| *g != generation || *f != fault)
            {
                recent.push((generation, fault, fault.diagnostic(self.source)));
                if recent.len() > 64 {
                    recent.remove(0);
                }
            }
        }
        ai_session_contract::HostStatus {
            schema_version: ai_session_contract::HostStatusSchemaVersion::VALUE,
            kind: ai_session_contract::HostStatusKind::HostStatus,
            generation: ai_session_contract::Counter(generation as i64),
            phase,
            source: self.source,
            version: self.version.clone(),
            recent: recent.iter().map(|(_, _, d)| d.clone()).collect(),
            diagnostic: fault.and_then(|_| recent.last().map(|(_, _, d)| d.clone())),
        }
    }
    pub(super) async fn launch(
        &self,
        execution: &ExecutionHandle,
        master: Arc<MasterKey>,
    ) -> ai_session_contract::HostStatus {
        {
            let mut state = self.state.lock().unwrap();
            state.generation += 1;
            state.owner = Host::Starting;
        }
        let trusted_digest = match self.source {
            ai_session_contract::HostStatusSource::DevelopmentOverride => None,
            ai_session_contract::HostStatusSource::BundledResource => {
                match option_env!("RSS_BUNDLED_RUNTIME_SHA256") {
                    Some(digest) => Some(digest),
                    None => {
                        self.state.lock().unwrap().owner = Host::Failed(Fault::Invalid);
                        return self.status();
                    }
                }
            }
        };
        let next = match configuration(&self.root).map_err(|_| Fault::Configuration) {
            Ok(configuration) => match host::launch(
                &self.artifact,
                &configuration,
                execution,
                master,
                trusted_digest,
            )
            .await
            {
                Ok(process) => Host::Running(process),
                Err(fault) => Host::Failed(fault),
            },
            Err(_) => Host::Failed(Fault::Configuration),
        };
        self.state.lock().unwrap().owner = next;
        self.status()
    }
}
