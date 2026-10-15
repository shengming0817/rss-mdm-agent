//! Fixed tool requests are executed by the query supervisor, never by the confined provider.
use super::*;
impl Context<'_> {
    pub(super) fn run(
        &self,
        material: &SoftwareMaterial,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> Result<Vec<u8>> {
        self.check()?;
        if !env.is_empty() {
            return Err(Status::Denied.into());
        }
        let name = self
            .input
            .tools
            .iter()
            .find(|(_, value)| value.path == material.path)
            .map(|(name, _)| name.as_str())
            .ok_or(Status::Denied)?;
        let reply = super::super::macos_supervisor::tool(name, args, self.cap)
            .map_err(|_| Fault::Unclosed)?;
        // Tool raw bytes were already counted by their parent owner, including stderr.
        self.bytes
            .set(self.bytes.get().saturating_add(reply.observed));
        self.input
            .observed
            .fetch_add(reply.observed as u64, std::sync::atomic::Ordering::Relaxed);
        if self.bytes.get() > self.cap {
            return Err(Status::Truncated.into());
        }
        if reply.status != Status::Complete {
            return Err(reply.status.into());
        }
        self.check()?;
        Ok(reply.bytes)
    }
}
