//! A complete integration scenario owns its process-local descriptor budget.
//! ref: Rust library/std/src/process.rs@1.96.0 (Child::try_wait, kill and wait).
use std::{
    io,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const CHILD_CASE: &str = "_AGENT_SCENARIO_CASE";
const ENTERED: &str = "agent-scenario-entered:";

/// Call before creating any scenario resources. The parent returns after the exact
/// child scenario finishes; the child continues through the original test body.
pub(crate) fn run_parent(case_id: &str) -> bool {
    let (_, case) = case_id.split_once("::").expect("qualified test name");
    if let Some(selected) = std::env::var_os(CHILD_CASE) {
        assert_eq!(selected, case, "isolated scenario marker mismatch");
        descriptor_budget().expect("isolated scenario descriptor budget");
        eprintln!("{ENTERED}{case}");
        return false;
    }
    run(command(case), case, Duration::from_secs(120)).unwrap();
    true
}

fn command(case: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", case, "--nocapture", "--include-ignored"])
        .env(CHILD_CASE, case)
        .stdin(Stdio::null());
    command
}

struct Owner(Child);
impl Drop for Owner {
    fn drop(&mut self) {
        // Child itself has no Drop cleanup. Reap even during parent unwinding.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn run(mut command: Command, case: &str, timeout: Duration) -> io::Result<()> {
    let root = crate::protocol_test_support::Root::new();
    let log_path = root.path.join("scenario.log");
    let log = platform_private_storage::create_new(&log_path)?;
    command.stdout(log.try_clone()?).stderr(log.try_clone()?);
    let mut owner = Owner(command.spawn()?);
    let pid = owner.0.id();
    // Do not retain the command's extra log handles while waiting.
    drop(command);
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = owner.0.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            owner.0.kill()?;
            owner.0.wait()?;
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // Windows creates a write-only, non-shared private file. Close the writer
    // after the child exits before reopening it for diagnostic reads.
    drop(log);
    let output = std::fs::read_to_string(log_path)?;
    // Exit success alone is insufficient: libtest also succeeds with zero matches.
    let expected = format!("{ENTERED}{case}");
    let entered = output.lines().any(|line| line == expected);
    if !status.is_some_and(|status| status.success()) || !entered {
        return Err(io::Error::other(format!(
            "isolated scenario {case}: pid={pid}, status={status:?}, entered={entered}\n{output}"
        )));
    }
    print!("{output}");
    Ok(())
}

fn descriptor_budget() -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: limit is a writable rlimit for this isolated child's process.
        if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if limit.rlim_cur < 256 {
            return Err(io::Error::other("scenario requires 256 descriptors"));
        }
        if limit.rlim_cur != 256 {
            limit.rlim_cur = 256;
            // SAFETY: only lowers this child's soft limit, retaining its hard limit.
            if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) } != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
            return Err(io::Error::last_os_error());
        }
        assert_eq!(limit.rlim_cur, 256);
        eprintln!("agent-scenario-nofile:{}", limit.rlim_cur);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const CASE: &str = "scenario_process::tests::child_fixture";
    const PROBE: &str = "_AGENT_SCENARIO_PROBE";

    #[test]
    #[ignore = "subprocess-only scenario owner fixture"]
    fn child_fixture() {
        if run_parent(concat!(module_path!(), "::child_fixture")) {
            return;
        }
        match std::env::var(PROBE).as_deref() {
            Ok("fail") => panic!("scenario failure must propagate"),
            Ok("wait") => std::thread::sleep(Duration::from_secs(30)),
            _ => {}
        }
    }

    #[test]
    fn exact_ignored_scenario_executes_and_failure_propagates() {
        run(command(CASE), CASE, Duration::from_secs(5)).unwrap();
        let mut failed = command(CASE);
        failed.env(PROBE, "fail");
        let error = run(failed, CASE, Duration::from_secs(5)).unwrap_err();
        assert!(error
            .to_string()
            .contains("scenario failure must propagate"));
    }

    #[test]
    fn zero_matches_and_mismatched_markers_cannot_pass() {
        let missing = "scenario_process::tests::missing_scenario";
        assert!(run(command(missing), missing, Duration::from_secs(5)).is_err());
        let mut mismatched = command(CASE);
        mismatched.env(CHILD_CASE, "different-scenario");
        let error = run(mismatched, CASE, Duration::from_secs(5)).unwrap_err();
        assert!(error
            .to_string()
            .contains("isolated scenario marker mismatch"));
    }

    #[test]
    fn timeout_reaps_the_child() {
        let mut waiting = command(CASE);
        waiting.env(PROBE, "wait");
        let error = run(waiting, CASE, Duration::from_secs(1)).unwrap_err();
        let error = error.to_string();
        assert!(error.contains("status=None, entered=true"));
        #[cfg(unix)]
        assert_reaped(
            error
                .split("pid=")
                .nth(1)
                .unwrap()
                .split(',')
                .next()
                .unwrap()
                .parse()
                .unwrap(),
        );
    }

    #[cfg(unix)]
    fn assert_reaped(pid: u32) {
        // SAFETY: signal zero only queries existence of the child's process.
        assert_eq!(unsafe { libc::kill(pid as libc::pid_t, 0) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
    }

    #[test]
    #[cfg(unix)]
    fn parent_unwind_reaps_the_child() {
        let mut waiting = command(CASE);
        waiting
            .env(PROBE, "wait")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let owner = Owner(waiting.spawn().unwrap());
        let pid = owner.0.id();
        let panic = std::panic::catch_unwind(move || {
            let _owner = owner;
            panic!("parent failure");
        });
        assert!(panic.is_err());
        assert_reaped(pid);
    }
}
