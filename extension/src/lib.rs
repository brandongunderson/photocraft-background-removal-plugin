//! Sandboxed Background Removal panel, built independently of the PhotoCraft host.
//! Request storage stays in a safe Rust buffer; only ABI export attributes need `unsafe`.
use serde_json::{Value, json};
use std::sync::Mutex;

const MANIFEST: &str = include_str!("../manifest.json");
static REQUEST: Mutex<Vec<u8>> = Mutex::new(Vec::new());

#[unsafe(no_mangle)]
pub extern "C" fn pce_api_version() -> u32 {
    1
}

fn packed(bytes: &[u8]) -> u64 {
    ((bytes.len() as u64) << 32) | bytes.as_ptr() as u64
}

#[unsafe(no_mangle)]
pub extern "C" fn pce_manifest() -> u64 {
    packed(MANIFEST.as_bytes())
}

#[unsafe(no_mangle)]
pub extern "C" fn pce_alloc(size: u32) -> *mut u8 {
    if size > 256 * 1024 {
        return std::ptr::null_mut();
    }
    let Ok(mut request) = REQUEST.lock() else {
        return std::ptr::null_mut();
    };
    request.resize(size as usize, 0);
    request.as_mut_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn pce_handle(ptr: *const u8, len: u32) -> u64 {
    let request = REQUEST
        .lock()
        .ok()
        .and_then(|request| {
            if request.as_ptr() != ptr {
                return None;
            }
            request
                .get(..len as usize)
                .and_then(|bytes| serde_json::from_slice(bytes).ok())
        })
        .unwrap_or(Value::Null);
    let response = handle(&request);
    let bytes = serde_json::to_vec(&response)
        .unwrap_or_else(|_| b"{\"message\":\"Could not encode response\"}".to_vec());
    packed(Box::leak(bytes.into_boxed_slice()))
}

fn handle(request: &Value) -> Value {
    if request.get("document").is_none_or(Value::is_null) {
        return json!({"message":"Open an image before using Background Removal."});
    }
    if request.get("event").and_then(Value::as_str) != Some("remove") {
        return json!({"message":"Unknown action"});
    }
    let values = request.get("values").unwrap_or(&Value::Null);
    let model = match values.get("model").and_then(Value::as_str) {
        Some("Standard · full precision") => "isnet",
        Some("Small · quantized") => "isnet_quint8",
        _ => "isnet_fp16",
    };
    let output = match values.get("output").and_then(Value::as_str) {
        Some("Keep background") => "background",
        Some("Subject mask") => "mask",
        _ => "foreground",
    };
    let runs = request
        .get("state")
        .and_then(|s| s.get("runs"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(1);
    json!({"state":{"runs":runs},"commands":[{"command":"plugin.processImage","params":{"processor":"native","options":{"model":model,"output":output},"preview":true}}]})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_exposes_general_processing_and_comparison_preview() {
        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(manifest["id"], "org.photocraft.background-removal");
        assert_eq!(
            manifest["permissions"]["commands"],
            json!(["plugin.processImage"])
        );
        assert_eq!(manifest["version"], "1.1.0");
        assert_eq!(manifest["panel"]["preview"], true);
        assert_eq!(
            manifest["panel"]["buttons"],
            json!([{"id":"remove","label":"Create result"}])
        );
        assert_eq!(
            manifest["panel"]["fields"][0]["default"],
            "Compact · half precision"
        );
        assert_eq!(
            handle(&json!({"event":"remove","document":{}}))["commands"][0]["params"]["options"]["model"],
            "isnet_fp16"
        );
    }

    #[test]
    fn all_models_and_outputs_map_to_owned_generic_worker_options() {
        for (label, model) in [
            ("Standard · full precision", "isnet"),
            ("Compact · half precision", "isnet_fp16"),
            ("Small · quantized", "isnet_quint8"),
        ] {
            for (choice, output) in [
                ("Keep subject", "foreground"),
                ("Keep background", "background"),
                ("Subject mask", "mask"),
            ] {
                let response = handle(
                    &json!({"event":"remove","document":{"width":25,"height":13},"state":{"runs":2},"values":{"model":label,"output":choice}}),
                );
                assert_eq!(response["state"]["runs"], 3);
                assert_eq!(
                    response["commands"],
                    json!([{"command":"plugin.processImage","params":{"processor":"native","options":{"model":model,"output":output},"preview":true}}])
                );
                let params = &response["commands"][0]["params"];
                assert!(params.get("id").is_none());
                assert!(params.get("width").is_none());
                assert!(params.get("height").is_none());
            }
        }
    }

    #[test]
    fn absent_document_and_unknown_event_do_not_run_inference() {
        assert!(
            handle(&json!({"event":"remove","document":null}))
                .get("commands")
                .is_none()
        );
        assert!(
            handle(&json!({"event":"unknown","document":{}}))
                .get("commands")
                .is_none()
        );
    }
}
