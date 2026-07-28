use std::io::Read;
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use reqwest::redirect::Policy;
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub(super) enum ProviderAuthorization<'a> {
    Bearer(&'a str),
    GoogleApiKey(&'a str),
}

pub(super) struct TransportResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

pub(super) trait AiImageTransport {
    fn post_json(
        &self,
        endpoint: &'static str,
        authorization: ProviderAuthorization<'_>,
        body: &[u8],
        max_response_bytes: usize,
    ) -> Result<TransportResponse, TransportFailure>;
}

pub(super) enum TransportFailure {
    Network,
    ResponseTooLarge,
}

pub(super) struct ReqwestTransport {
    client: Client,
}

impl ReqwestTransport {
    pub fn new() -> AppResult<Self> {
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT)
            .https_only(true)
            .no_proxy()
            .build()
            .map_err(|_| {
                AppError::new(
                    "ai_network_client",
                    "보안 HTTP 클라이언트를 준비하지 못했습니다.",
                )
            })?;
        Ok(Self { client })
    }
}

impl AiImageTransport for ReqwestTransport {
    fn post_json(
        &self,
        endpoint: &'static str,
        authorization: ProviderAuthorization<'_>,
        body: &[u8],
        max_response_bytes: usize,
    ) -> Result<TransportResponse, TransportFailure> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        match authorization {
            ProviderAuthorization::Bearer(token) => {
                let value = Zeroizing::new(format!("Bearer {token}"));
                let header =
                    HeaderValue::from_str(value.as_str()).map_err(|_| TransportFailure::Network)?;
                headers.insert(AUTHORIZATION, header);
            }
            ProviderAuthorization::GoogleApiKey(key) => {
                let name = HeaderName::from_static("x-goog-api-key");
                let header = HeaderValue::from_str(key).map_err(|_| TransportFailure::Network)?;
                headers.insert(name, header);
            }
        }

        let response = self
            .client
            .post(endpoint)
            .headers(headers)
            .body(body.to_vec())
            .send()
            .map_err(|_| TransportFailure::Network)?;
        let status = response.status().as_u16();
        if response
            .content_length()
            .is_some_and(|length| length > max_response_bytes as u64)
        {
            return Err(TransportFailure::ResponseTooLarge);
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let mut response_body = Vec::new();
        response
            .take(max_response_bytes as u64 + 1)
            .read_to_end(&mut response_body)
            .map_err(|_| TransportFailure::Network)?;
        if response_body.len() > max_response_bytes {
            return Err(TransportFailure::ResponseTooLarge);
        }
        Ok(TransportResponse {
            status,
            content_type,
            body: response_body,
        })
    }
}
