//! Fixed trusted commands, with one live group owner and bounded pipe lifetime.
use super::*;
use std::{
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
impl Context<'_> {
    pub(super) fn run(
        &self,
        material: &SoftwareMaterial,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> Result<Vec<u8>> {
        self.check()?;
        let path = Path::new(&material.path);
        let _lease = execution_runner::staging::verify_retained(path, &material.artifact.sha256)
            .map_err(|_| Status::Denied)?;
        let mut command = Command::new(path);
        command
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("LC_ALL", "C")
            .envs(env.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut group = native_process::AttemptGroup::new()?;
        group.configure(&mut command);
        let mut child = command.spawn()?;
        let observed = Arc::new(AtomicUsize::new(0));
        let cap = self.cap.saturating_sub(self.bytes.get());
        let read = |mut pipe: Box<dyn Read + Send>, observed: Arc<AtomicUsize>| {
            std::thread::spawn(move || {
                let mut out = Vec::new();
                let mut buffer = [0u8; 8192];
                loop {
                    let n = pipe.read(&mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    let prior = observed.fetch_add(n, Ordering::Relaxed);
                    out.extend_from_slice(&buffer[..n.min(cap.saturating_sub(prior))]);
                }
                Ok::<_, std::io::Error>(out)
            })
        };
        let stdout = read(
            Box::new(child.stdout.take().ok_or(Fault::Unclosed)?),
            observed.clone(),
        );
        let stderr = read(
            Box::new(child.stderr.take().ok_or(Fault::Unclosed)?),
            observed.clone(),
        );
        let mut stopped = false;
        let exit = loop {
            if let Some(exit) = child.try_wait().map_err(|_| Fault::Unclosed)? {
                break exit;
            }
            if Instant::now() >= self.until || observed.load(Ordering::Relaxed) > cap {
                group.terminate();
                stopped = true;
                break child.wait().map_err(|_| Fault::Unclosed)?;
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        // A successful root exit cannot excuse a pipe still held by unaccounted work.
        while !stdout.is_finished() || !stderr.is_finished() {
            if Instant::now() >= self.until {
                group.terminate();
                return Err(Fault::Unclosed);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let output = stdout
            .join()
            .map_err(|_| Fault::Unclosed)?
            .map_err(|_| Fault::Unclosed)?;
        let _diagnostic = stderr
            .join()
            .map_err(|_| Fault::Unclosed)?
            .map_err(|_| Fault::Unclosed)?;
        if stopped {
            return Err(Fault::Unclosed);
        }
        self.consume(observed.load(Ordering::Relaxed))?;
        if !exit.success() {
            return Err(Status::Invalid.into());
        }
        Ok(output)
    }
}
