use std::sync::Mutex;

use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};
use crate::models::{AiProviderSessionStatusDto, SetAiSessionCredentialPayload};

#[derive(Default)]
struct CredentialSlots {
    novelai: Option<Zeroizing<String>>,
    gemini: Option<Zeroizing<String>>,
}

#[derive(Default)]
pub struct AiSessionCredentialStore {
    slots: Mutex<CredentialSlots>,
}

impl AiSessionCredentialStore {
    pub fn status(&self) -> AppResult<AiProviderSessionStatusDto> {
        let slots = self.slots.lock().map_err(|_| AppError::lock_failed())?;
        Ok(status_for(&slots))
    }

    pub fn set(
        &self,
        payload: SetAiSessionCredentialPayload,
    ) -> AppResult<AiProviderSessionStatusDto> {
        let SetAiSessionCredentialPayload {
            provider,
            credential,
        } = payload;
        let credential = Zeroizing::new(credential);
        validate_credential(&provider, credential.as_str())?;
        let mut slots = self.slots.lock().map_err(|_| AppError::lock_failed())?;
        match provider.as_str() {
            "novelai" => slots.novelai = Some(credential),
            "gemini" => slots.gemini = Some(credential),
            _ => return Err(invalid_provider()),
        }
        Ok(status_for(&slots))
    }

    pub fn clear(&self, provider: &str) -> AppResult<AiProviderSessionStatusDto> {
        let mut slots = self.slots.lock().map_err(|_| AppError::lock_failed())?;
        match provider {
            "novelai" => slots.novelai = None,
            "gemini" => slots.gemini = None,
            _ => return Err(invalid_provider()),
        }
        Ok(status_for(&slots))
    }

    pub(crate) fn credential(&self, provider: &str) -> AppResult<Zeroizing<String>> {
        let slots = self.slots.lock().map_err(|_| AppError::lock_failed())?;
        let credential = match provider {
            "novelai" => slots.novelai.as_deref(),
            "gemini" => slots.gemini.as_deref(),
            _ => return Err(invalid_provider()),
        }
        .ok_or_else(|| {
            AppError::new(
                "ai_credential_missing",
                "이 공급자의 세션 API 키가 없습니다. 키를 입력한 뒤 다시 시도해 주세요.",
            )
        })?;
        Ok(Zeroizing::new(credential.to_string()))
    }
}

fn status_for(slots: &CredentialSlots) -> AiProviderSessionStatusDto {
    AiProviderSessionStatusDto {
        novel_ai_configured: slots.novelai.is_some(),
        gemini_configured: slots.gemini.is_some(),
    }
}

fn validate_credential(provider: &str, credential: &str) -> AppResult<()> {
    if !(16..=512).contains(&credential.len())
        || !credential.is_ascii()
        || credential.chars().any(char::is_whitespace)
        || credential.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "ai_credential_invalid",
            "API 키 형식이 올바르지 않습니다. 앞뒤 공백 없이 다시 입력해 주세요.",
        ));
    }
    match provider {
        "novelai" if !credential.starts_with("pst-") => Err(AppError::new(
            "ai_credential_invalid",
            "NovelAI에는 공식 계정 화면에서 만든 pst- 형식의 Persistent API Token을 입력해 주세요.",
        )),
        "novelai" | "gemini" => Ok(()),
        _ => Err(invalid_provider()),
    }
}

pub(super) fn invalid_provider() -> AppError {
    AppError::new(
        "ai_provider_invalid",
        "지원하지 않는 AI 공급자입니다. NovelAI 또는 Gemini를 선택해 주세요.",
    )
}

#[cfg(test)]
mod tests {
    use serde_json::to_string;

    use super::*;

    #[test]
    fn credentials_are_status_only_and_never_serialized_back() {
        let store = AiSessionCredentialStore::default();
        let status = store
            .set(SetAiSessionCredentialPayload {
                provider: "novelai".to_string(),
                credential: "pst-example-secret-token".to_string(),
            })
            .unwrap();
        let serialized = to_string(&status).unwrap();
        assert_eq!(
            serialized,
            r#"{"novelAiConfigured":true,"geminiConfigured":false}"#
        );
        assert!(!serialized.contains("pst-example-secret-token"));

        let cleared = store.clear("novelai").unwrap();
        assert!(!cleared.novel_ai_configured);
    }

    #[test]
    fn novelai_requires_persistent_token_shape() {
        let store = AiSessionCredentialStore::default();
        let error = store
            .set(SetAiSessionCredentialPayload {
                provider: "novelai".to_string(),
                credential: "not-a-persistent-token".to_string(),
            })
            .unwrap_err();
        assert_eq!(error.code, "ai_credential_invalid");
        assert!(!to_string(&error)
            .unwrap()
            .contains("not-a-persistent-token"));
    }
}
