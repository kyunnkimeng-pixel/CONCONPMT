use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::error::{AppError, AppResult};

const MAX_BYTES: usize = 64 * 1024;
const MAX_DEPTH: usize = 8;
const MAX_NODES: usize = 1024;

pub(crate) fn canonicalize(kind: &str, raw: &str) -> AppResult<String> {
    if raw.len() > MAX_BYTES {
        return Err(snapshot_error("AI 실행 스냅샷은 64KiB를 넘을 수 없습니다."));
    }
    let value: Value = serde_json::from_str(raw)
        .map_err(|_| snapshot_error("AI 실행 스냅샷 JSON이 올바르지 않습니다."))?;
    validate_root(kind, &value)?;
    let mut nodes = 0;
    validate_value(kind, &value, 0, &mut nodes)?;
    let canonical = serde_json::to_string(&sort_json(&value))
        .map_err(|_| snapshot_error("AI 실행 스냅샷을 정규화할 수 없습니다."))?;
    if canonical.len() > MAX_BYTES {
        return Err(snapshot_error(
            "정규화된 AI 실행 스냅샷은 64KiB를 넘을 수 없습니다.",
        ));
    }
    Ok(canonical)
}

fn validate_root(kind: &str, value: &Value) -> AppResult<()> {
    if kind == "policy_refs" {
        return if value.is_array() {
            Ok(())
        } else {
            Err(snapshot_error("정책 참조 스냅샷은 배열이어야 합니다."))
        };
    }
    let object = value
        .as_object()
        .ok_or_else(|| snapshot_error("AI 실행 스냅샷은 객체여야 합니다."))?;
    let allowed: &[&str] = match kind {
        "capability" => &[
            "schema",
            "provider",
            "serviceSurface",
            "source",
            "supports",
            "limitations",
        ],
        "data_tier" => &["schema", "source", "tier"],
        "retention" => &["schema", "source", "retention"],
        "consent" => &[
            "schema",
            "source",
            "confirmed",
            "humanActionConfirmed",
            "rightsConfirmed",
            "costConfirmed",
            "requestContentConfirmed",
            "contractOverrideConfirmed",
            "adultConfirmed",
            "under18AudienceExcludedConfirmed",
            "professionalBusinessConfirmed",
            "supportedRegionConfirmed",
            "paidServiceConfirmed",
        ],
        "prompt_options" => &[
            "schema",
            "importedResultOnly",
            "operation",
            "provider",
            "model",
            "action",
            "prompt",
            "negativePrompt",
            "seed",
            "width",
            "height",
            "steps",
            "scale",
            "strength",
            "noise",
            "outputCount",
            "gridLayout",
        ],
        "provider_usage" => &[
            "schema",
            "provider",
            "candidateIndex",
            "seed",
            "input_tokens_by_modality",
            "output_tokens_by_modality",
            "total_thought_tokens",
            "total_tokens",
        ],
        _ => return Err(snapshot_error("알 수 없는 AI 스냅샷 종류입니다.")),
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(snapshot_error(
            "허용되지 않은 AI 스냅샷 필드가 포함되어 있습니다.",
        ));
    }
    Ok(())
}

fn validate_value(kind: &str, value: &Value, depth: usize, nodes: &mut usize) -> AppResult<()> {
    *nodes += 1;
    if depth > MAX_DEPTH || *nodes > MAX_NODES {
        return Err(snapshot_error("AI 실행 스냅샷 구조가 너무 복잡합니다."));
    }
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
        Value::String(value) => {
            let lower = value.to_ascii_lowercase();
            if value.len() > 4096
                || value.chars().any(char::is_control)
                || lower.starts_with("data:")
                || lower.contains("authorization:")
                || lower.contains("bearer ")
                || lower.contains("cookie:")
                || lower.contains("base64,")
            {
                return Err(snapshot_error(
                    "AI 실행 스냅샷에 비밀값 또는 바이너리 payload를 넣을 수 없습니다.",
                ));
            }
            Ok(())
        }
        Value::Array(values) => {
            for value in values {
                validate_value(kind, value, depth + 1, nodes)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for (key, value) in values {
                let lower = key.to_ascii_lowercase();
                let consent_request_confirmation =
                    kind == "consent" && key == "requestContentConfirmed";
                let provider_usage_metric = kind == "provider_usage"
                    && matches!(
                        key.as_str(),
                        "input_tokens_by_modality"
                            | "output_tokens_by_modality"
                            | "total_thought_tokens"
                            | "total_tokens"
                    );
                if !consent_request_confirmation
                    && !provider_usage_metric
                    && [
                        "authorization",
                        "token",
                        "cookie",
                        "secret",
                        "headers",
                        "request",
                        "response",
                        "bytes",
                        "base64",
                    ]
                    .iter()
                    .any(|forbidden| lower.contains(forbidden))
                {
                    return Err(snapshot_error(
                        "AI 실행 스냅샷에 비밀값 또는 전체 요청/응답 필드를 넣을 수 없습니다.",
                    ));
                }
                validate_value(kind, value, depth + 1, nodes)?;
            }
            Ok(())
        }
    }
}

pub(crate) fn canonical_value(value: &Value) -> String {
    serde_json::to_string(&sort_json(value)).unwrap_or_else(|_| "null".to_string())
}

fn sort_json(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(sort_json).collect()),
        Value::Object(values) => {
            let sorted = values
                .iter()
                .map(|(key, value)| (key.clone(), sort_json(value)))
                .collect::<BTreeMap<_, _>>();
            let mut object = Map::new();
            for (key, value) in sorted {
                object.insert(key, value);
            }
            Value::Object(object)
        }
        _ => value.clone(),
    }
}

fn snapshot_error(message: &str) -> AppError {
    AppError::new("ai_snapshot", message)
}

#[cfg(test)]
mod tests {
    use super::canonicalize;

    #[test]
    fn canonical_encoding_is_stable() {
        let canonical = canonicalize(
            "prompt_options",
            r#"{"width":1024,"schema":"pmtcon-ai-prompt-options-v1","prompt":"고양이"}"#,
        )
        .unwrap();
        assert_eq!(
            canonical,
            r#"{"prompt":"고양이","schema":"pmtcon-ai-prompt-options-v1","width":1024}"#
        );
    }

    #[test]
    fn secret_binary_and_unknown_fields_are_rejected() {
        assert!(canonicalize(
            "prompt_options",
            r#"{"schema":"pmtcon-ai-prompt-options-v1","token":"secret"}"#
        )
        .is_err());
        assert!(canonicalize(
            "prompt_options",
            r#"{"schema":"pmtcon-ai-prompt-options-v1","prompt":"data:image/png;base64,AAAA"}"#
        )
        .is_err());
        assert!(canonicalize(
            "prompt_options",
            r#"{"schema":"pmtcon-ai-prompt-options-v1","fullRequest":{}}"#
        )
        .is_err());
    }
}
