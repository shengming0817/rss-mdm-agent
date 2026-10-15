//! Query isolation reuses the product Job owner; no task retry or persistent state.
use super::*;
use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub(super) fn query(
    path: &Path,
    input: &Input,
    id: &str,
    q: &w::NativeQuery,
    remaining: Duration,
) -> Result<w::NativeQueryResult, Error> {
    let began = Instant::now();
    let allowance = remaining
        .min(Duration::from_secs(u64::from(q.budget.timeout_seconds)))
        .min(Duration::from_secs(
            u64::try_from(input.task.deadline.saturating_sub(now()?)).unwrap_or(0),
        ));
    if allowance <= Duration::from_millis(100) {
        return Ok(failure(q, w::NativeQueryStatus::Timeout, now()?));
    }
    let image = std::env::current_exe()?;
    let _lease = execution_runner::staging::verify_retained(&image, &input.image_sha256)
        .map_err(crate::error::app_error)?;
    let mut command = Command::new(image);
    command
        .arg("--collection-query")
        .arg(path)
        .arg(id)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut owner = native_process::OwnedHost::prepare(&mut command)?;
    let mut child = command.spawn()?;
    if owner.attach(child.id()).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Error::Unavailable);
    }
    let stdout = child.stdout.take().ok_or(Error::Unavailable)?;
    let mut stderr = child.stderr.take().ok_or(Error::Unavailable)?;
    let diagnostics = std::thread::spawn(move || {
        let mut retained = Vec::new();
        let mut buffer = [0u8; 256];
        loop {
            let n = stderr.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            let keep = n.min(512usize.saturating_sub(retained.len()));
            retained.extend_from_slice(&buffer[..keep]);
        }
        Ok::<_, std::io::Error>(retained)
    });
    let cap = u64::from(q.budget.output_bytes);
    let reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        stdout.take(cap + 1).read_to_end(&mut b).map(|_| b)
    });
    let mut timed_out = false;
    let exit = loop {
        if let Some(exit) = child.try_wait()? {
            break Some(exit);
        }
        // Reserve a small portion of the *same* budget for process/pipe cleanup.
        if began.elapsed() >= allowance.saturating_sub(Duration::from_millis(100)) {
            timed_out = true;
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    owner.terminate()?;
    while !owner.empty() {
        if began.elapsed() >= allowance {
            return Err(Error::Unavailable);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    child.wait()?;
    let bytes = reader.join().map_err(|_| Error::Unavailable)??;
    let diagnostics = diagnostics.join().map_err(|_| Error::Unavailable)??;
    let control: CollectionCompletion =
        serde_json::from_slice(&diagnostics).map_err(|_| Error::Unavailable)?;
    input.observed.fetch_add(
        control.native_output_bytes,
        std::sync::atomic::Ordering::Relaxed,
    );
    let material = serde_json::to_vec(input).map_err(|_| Error::Protocol)?;
    if !control.closed
        || control.input_sha256 != hex(&material)
        || control.output_sha256 != hex(&bytes)
    {
        return Err(Error::Unavailable);
    }
    let time = now()?;
    if timed_out {
        return Ok(failure(q, w::NativeQueryStatus::Timeout, time));
    }
    if bytes.len() > cap as usize {
        return Ok(failure(q, w::NativeQueryStatus::Truncated, time));
    }
    if exit.is_none_or(|v| !v.success()) {
        return Ok(failure(q, w::NativeQueryStatus::Invalid, time));
    }
    Ok(decode_query(q, &bytes, time))
}
