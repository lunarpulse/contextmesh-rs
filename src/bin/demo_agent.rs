//! The OA-05 JSON Lines demonstration provider.

use std::io::{BufWriter, Read as _, Write as _};

/// Maximum JSONL line bytes including the newline.
const LINE_LIMIT: usize = 2 * 1024 * 1024;

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = BufWriter::new(std::io::stdout().lock());
    let mut reader = stdin.lock().bytes();
    loop {
        let mut line = Vec::new();
        let mut oversized = false;
        let mut newline = false;
        loop {
            match reader.next() {
                None | Some(Err(_)) => break,
                Some(Ok(byte)) => {
                    if byte == b'\n' {
                        newline = true;
                        break;
                    }
                    if line.len() + 1 > LINE_LIMIT {
                        oversized = true;
                    } else {
                        line.push(byte);
                    }
                }
            }
        }
        if line.is_empty() && !newline {
            return;
        }
        let response = if oversized {
            failure_response(None, "limit_exceeded", "input line exceeds the JSONL limit")
        } else {
            handle(&line)
        };
        if stdout
            .write_all(response.as_bytes())
            .and_then(|_| stdout.write_all(b"\n"))
            .and_then(|_| stdout.flush())
            .is_err()
        {
            return;
        }
        if !newline {
            return;
        }
    }
}

fn handle(line: &[u8]) -> String {
    let parsed: Result<serde_json::Value, _> = serde_json::from_slice(line);
    let Some(object) = parsed.ok().and_then(|value| value.as_object().cloned()) else {
        return failure_response(None, "malformed_input", "input is not a JSON object");
    };
    let invocation = object
        .get("invocation_id")
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let fail = |code: &str, detail: &str| failure_response(invocation.clone(), code, detail);
    if object.len() != 7
        || object.get("protocol_version").and_then(|v| v.as_u64()) != Some(1)
        || !matches!(object.get("context"), Some(serde_json::Value::String(_)))
        || !matches!(
            object.get("selected_head"),
            Some(serde_json::Value::String(_))
        )
        || !matches!(
            object.get("request_event_id"),
            Some(serde_json::Value::String(_))
        )
        || !matches!(
            object.get("invocation_id"),
            Some(serde_json::Value::String(_))
        )
        || !matches!(object.get("ancestry"), Some(serde_json::Value::Array(_)))
    {
        return fail("malformed_input", "input field set is invalid");
    }
    let ancestry = object
        .get("ancestry")
        .and_then(|v| v.as_array())
        .expect("checked");
    if ancestry.len() > 1_024 {
        return fail("limit_exceeded", "ancestry exceeds its bound");
    }
    let input = object
        .get("input")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    success_response(
        object
            .get("invocation_id")
            .and_then(|v| v.as_str())
            .expect("checked"),
        &serde_json::json!({"demo": {"echo": input}}),
    )
}

fn success_response(invocation_id: &str, response: &serde_json::Value) -> String {
    serde_json::json!({
        "invocation_id": invocation_id,
        "ok": true,
        "protocol_version": 1,
        "response": response
    })
    .to_string()
}

fn failure_response(invocation_id: Option<String>, code: &str, detail: &str) -> String {
    serde_json::json!({
        "detail": sanitize(detail),
        "error_code": code,
        "invocation_id": invocation_id,
        "ok": false,
        "protocol_version": 1
    })
    .to_string()
}

fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_ascii_control())
        .take(1_024)
        .collect()
}
