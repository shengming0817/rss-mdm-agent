use crate::ServiceError;
use serde::Serialize;
use std::io::Write;

struct Writer {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other("output budget"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(crate) fn encode(value: &impl Serialize, limit: usize) -> Result<Vec<u8>, ServiceError> {
    let mut writer = Writer {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| ServiceError::Limit)?;
    Ok(writer.bytes)
}
