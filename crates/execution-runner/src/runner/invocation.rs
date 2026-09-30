use super::*;
/// Physical helper state only. Its dispatch already has a committed Begin in the system journal.
pub(crate) struct PhysicalInvocation {
    pub(crate) digest: Digest,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) facts: Arc<Mutex<Option<ProcessEvidence>>>,
}
impl PhysicalInvocation {
    pub(crate) fn start(
        plan: FrozenExecution,
        attempt: AttemptId,
        source: Artifacts,
        invocation: SoftwareInvocation,
        timeout_ms: u64,
        output_bytes: u64,
        first_start: Option<u64>,
    ) -> Result<Self, Error> {
        if timeout_ms == 0
            || timeout_ms > invocation.timeout_ms
            || output_bytes == 0
            || output_bytes > invocation.output_bytes
        {
            return Err(Error::Denied);
        }
        source.inspect_recipe(crate::materialize::Recipe::invocation(&invocation))?;
        let cancel = Arc::new(AtomicBool::new(false));
        let runner = Id::new("native-user-helper").expect("constant");
        let mut preparing = rejected(&plan, &attempt, &runner, ProcessEnd::Unknown);
        preparing.finished = false;
        preparing.quiescent = false;
        preparing.scope = ProcessScope::Preparing {};
        let facts = Arc::new(Mutex::new(Some(preparing)));
        let owner = Self {
            digest: plan.digest().clone(),
            cancel: cancel.clone(),
            facts: facts.clone(),
        };
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        std::thread::Builder::new()
            .name("rss-user-invocation".into())
            .spawn(move || {
                let result = (|| -> Result<(), Error> {
                    let prepared = source.prepare_recipe(
                        &plan,
                        &attempt,
                        crate::materialize::Recipe::invocation(&invocation),
                        &crate::software::PreparationControl {
                            deadline,
                            cancelled: cancel.clone(),
                        },
                    )?;
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|_| Error::Unavailable)?;
                    runtime.block_on(run(
                        prepared,
                        plan.clone(),
                        attempt.clone(),
                        runner.clone(),
                        (output_bytes, deadline),
                        cancel,
                        Captures {
                            start_before: first_start,
                            process: facts.clone(),
                        },
                        Some(invocation),
                    ));
                    Ok(())
                })();
                if let Err(error) = result {
                    publish(&facts, failed(&plan, &attempt, &runner, classify(error)));
                }
            })
            .map_err(|_| Error::Unavailable)?;
        Ok(owner)
    }
}
impl Drop for PhysicalInvocation {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
