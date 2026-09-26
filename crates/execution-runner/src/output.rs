use execution_contract::{OutputFormat, OutputQuality, OutputSpec, TextEncoding};
pub(crate) fn decode(bytes: &[u8], encoding: TextEncoding) -> Option<String> {
    match encoding {
        TextEncoding::Utf8 => std::str::from_utf8(bytes).ok().map(str::to_owned),
        TextEncoding::Utf16Le if bytes.len().is_multiple_of(2) => String::from_utf16(
            &bytes
                .chunks_exact(2)
                .map(|v| u16::from_le_bytes([v[0], v[1]]))
                .collect::<Vec<_>>(),
        )
        .ok(),
        _ => None,
    }
}
pub(crate) fn quality(stdout: &[u8], stderr: &[u8], spec: OutputSpec) -> OutputQuality {
    let Some(text) = decode(stdout, spec.stdout) else {
        return OutputQuality::Failed;
    };
    if decode(stderr, spec.stderr).is_none() {
        return OutputQuality::Failed;
    }
    match spec.format {
        OutputFormat::Text {} => OutputQuality::Complete,
        OutputFormat::Json { max_rows } => match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(serde_json::Value::Object(_)) if max_rows > 0 => OutputQuality::Complete,
            Ok(serde_json::Value::Array(rows))
                if !rows.is_empty()
                    && rows.len() <= usize::from(max_rows)
                    && rows.iter().all(serde_json::Value::is_object) =>
            {
                OutputQuality::Complete
            }
            _ => OutputQuality::Failed,
        },
    }
}
