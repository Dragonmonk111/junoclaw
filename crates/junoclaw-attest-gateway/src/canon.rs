//! Deterministic JSON canonicalization ("JCS-lite").
//!
//! Sorts object keys recursively, emits arrays in order, compact
//! separators, strings via serde_json escaping, numbers via serde_json's
//! Display. Divergence from full RFC 8785: number formatting follows
//! serde_json (e.g. `1.0` stays `1.0`); customers needing strict JCS
//! should submit `payload_hash` computed their side.
//!
//! Whatever shape we pick, the hash only has to be deterministic for the
//! same logical payload — the gateway is the one hashing, so consistency
//! is what matters, not cross-implementation agreement.

use serde_json::Value;

pub fn canonicalize(v: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_value(v, &mut out);
    out
}

fn write_value(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Value::Number(n) => out.extend_from_slice(n.to_string().as_bytes()),
        Value::String(s) => {
            // serde_json string escaping is deterministic for a given input.
            let enc = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
            out.extend_from_slice(enc.as_bytes());
        }
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_value(item, out);
            }
            out.push(b']');
        }
        Value::Object(map) => {
            out.push(b'{');
            for (i, (k, val)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_value(&Value::String(k.clone()), out);
                out.push(b':');
                write_value(val, out);
            }
            out.push(b'}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sorts_keys_and_is_stable() {
        let a = json!({"b": 1, "a": {"d": [3,2], "c": "x"}});
        let canon = canonicalize(&a);
        assert_eq!(canon, br#"{"a":{"c":"x","d":[3,2]},"b":1}"#.to_vec());
        // stability
        assert_eq!(canon, canonicalize(&a));
    }

    #[test]
    fn same_logical_object_same_bytes() {
        let a = json!({"x":1,"y":2});
        let b = json!({"y":2,"x":1});
        assert_eq!(canonicalize(&a), canonicalize(&b));
    }
}
