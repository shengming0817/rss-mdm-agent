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
                if rows.len() <= usize::from(max_rows)
                    && rows.iter().all(serde_json::Value::is_object) =>
            {
                OutputQuality::Complete
            }
            Ok(serde_json::Value::Array(rows))
                if rows.len() > usize::from(max_rows)
                    && rows.iter().all(serde_json::Value::is_object) =>
            {
                OutputQuality::Truncated
            }
            _ => OutputQuality::Failed,
        },
    }
}

pub(crate) fn valid_encoding(bytes: &[u8], encoding: TextEncoding) -> bool {
    match encoding {
        TextEncoding::Utf8 => std::str::from_utf8(bytes).is_ok(),
        TextEncoding::Utf16Le => {
            bytes.len().is_multiple_of(2)
                && char::decode_utf16(
                    bytes
                        .chunks_exact(2)
                        .map(|b| u16::from_le_bytes([b[0], b[1]])),
                )
                .all(|c| c.is_ok())
        }
    }
}

#[cfg(test)]
mod collection_tests {
    use super::*;
    #[test]
    fn empty_json_list_is_complete_and_overflow_rows_are_explicitly_truncated() {
        let spec = OutputSpec {
            format: OutputFormat::Json { max_rows: 1 },
            stdout: TextEncoding::Utf8,
            stderr: TextEncoding::Utf8,
        };
        assert_eq!(quality(b"[]", b"", spec), OutputQuality::Complete);
        assert_eq!(quality(b"[{},{}]", b"", spec), OutputQuality::Truncated);
    }
}
