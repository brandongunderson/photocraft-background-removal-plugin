#![deny(unsafe_code)]
use photocraft_plugin_sdk::api::{
    ApiError, CommandInvocation, ErrorCode, EventRequest, PanelResponse, Result,
};
use photocraft_plugin_sdk::serde_json::{Value, json};

pub mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
#[cfg(test)]
const MANIFEST: &str = bindings::MANIFEST_JSON;

fn reply(request: &EventRequest, message: &str) -> Result<PanelResponse> {
    Ok(PanelResponse {
        state: request.state.clone(),
        message: message.into(),
        ..PanelResponse::default()
    })
}

fn effect(state: Value, message: &str, command: &str, params: Value) -> Result<PanelResponse> {
    Ok(PanelResponse {
        state,
        message: message.into(),
        commands: vec![CommandInvocation {
            command: command.into(),
            params: params.as_object().cloned().ok_or_else(|| {
                ApiError::new(ErrorCode::InvalidJson, "Invalid processing options.")
            })?,
        }],
        ..PanelResponse::default()
    })
}

fn handle_event(request: &EventRequest) -> Result<PanelResponse> {
    if !request.document.is_object() {
        return reply(request, "Open an image before using Background Removal.");
    }
    if request.event.as_str() != "remove" {
        return reply(request, "Unknown action");
    }
    let values = &request.values;
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
        .state
        .get("runs")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(1);
    let mut state = request.state.as_object().cloned().unwrap_or_default();
    state.insert("runs".into(), json!(runs));
    effect(
        json!(state),
        "Creating a background-removal preview…",
        "plugin.processImage",
        json!({"processor":"native","options":{"model":model,"output":output},"preview":true}),
    )
}

photocraft_plugin_sdk::export_plugin!(bindings::MANIFEST_JSON, handle_event);

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_plugin_sdk::serde_json;

    // Existing feature cases also pass through the PhotoCraft Plugin API request/response boundary.
    fn handle(value: &Value) -> Value {
        let mut input = value.clone();
        if let Some(object) = input.as_object_mut() {
            object.entry("apiVersion").or_insert(json!(1));
            object
                .entry("host")
                .or_insert(json!({"version":1,"capabilities":[]}));
        }
        let input = serde_json::to_vec(&input).unwrap();
        let mut guest = photocraft_plugin_sdk::GuestBuffers::new();
        let buffer = guest.allocate(input.len()).unwrap();
        buffer.copy_from_slice(&input);
        let pointer = buffer.as_ptr();
        match guest.respond(pointer, input.len(), handle_event) {
            Ok(bytes) => serde_json::from_slice(bytes).unwrap(),
            Err(error) => {
                json!({"state": value.get("state").unwrap_or(&Value::Null), "message":error.message,"commands":[]})
            }
        }
    }

    #[test]
    fn metadata_is_additive_and_unknown_versions_are_rejected() {
        assert_eq!(photocraft_guest_api_version(), 1);
        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(manifest["apiVersion"], 1);
        assert_eq!(
            manifest["capabilities"]["required"],
            json!([{"id":"host.commands.invoke","minVersion":1}])
        );
        let base = json!({"apiVersion":1,"host":{"version":1,"capabilities":[]},
            "event":"init","values":{},"document":null,"state":{"saved":7},
            "compatibility":{"panelApi":1,"hostApi":1,"negotiated":{"available":[{"id":"host.commands.invoke","version":1}],"unavailableOptional":[]}},
            "futureHostMetadata":{"added":true}});
        EventRequest::parse(&serde_json::to_vec(&base).unwrap()).unwrap();
        let response = handle(&base);
        assert_eq!(response["state"], base["state"]);
        assert_eq!(response["commands"], json!([]));
        for event in [
            "init",
            "change",
            "document_changed",
            "host_response",
            "unrecognized",
        ] {
            let mut input = base.clone();
            input["event"] = json!(event);
            assert_eq!(handle(&input)["commands"], json!([]));
        }
        for version in [0, 3] {
            let mut invalid = base.clone();
            invalid["apiVersion"] = json!(version);
            assert!(EventRequest::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
        }
        let missing_metadata = json!({"event":"init","values":{},"state":{}});
        assert!(EventRequest::parse(&serde_json::to_vec(&missing_metadata).unwrap()).is_err());
    }

    #[test]
    fn manifest_exposes_general_processing_and_comparison_preview() {
        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(manifest["id"], "org.photocraft.background-removal");
        assert_eq!(
            manifest["permissions"]["commands"],
            json!(["plugin.processImage"])
        );
        assert_eq!(manifest["version"], "1.2.1");
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
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            handle(&json!({"event":"unknown","document":{}}))
                .get("commands")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}
