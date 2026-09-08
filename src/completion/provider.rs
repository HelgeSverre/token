//! Worker-side provider contract. Dropping a suggestion future cancels its work;
//! implementations must not leave detached generation tasks running locally.

use std::future::Future;
use std::pin::Pin;

use super::inline::{InlineRequest, RequestSnapshot};
use crate::config::ProviderConfig;
use crate::model::EditorId;

/// Provider-neutral input is separate from runtime routing/configuration.
#[derive(Debug, Clone)]
pub struct InlineJob {
    pub request: InlineRequest,
    pub provider: ProviderConfig,
    pub context: Option<super::postprocess::InlineContext>,
}

/// Lifetime of a debounce, request, or visible suggestion in one editor pane.
#[derive(Debug, Clone)]
pub struct InlineSession {
    pub snapshot: RequestSnapshot,
    pub editor_id: Option<EditorId>,
    pub provider: ProviderConfig,
    pub(crate) observation: Option<super::statistics::Observation>,
}

pub type SuggestionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>>;

/// Ordered raw alternatives. Non-HTTP providers use the same request,
/// cancellation, filtering and rendering path as HTTP providers.
pub trait InlineProvider: Send {
    fn suggest<'a>(&'a mut self, request: &'a InlineRequest) -> SuggestionFuture<'a>;
}

/// Safe for status-bar display: errors never echo source text, response bodies,
/// URL credentials, or query parameters returned by a backend.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("invalid inline provider configuration: {0}")]
    Configuration(&'static str),
    #[error("inline request timed out")]
    Timeout,
    #[error("cannot connect to inline backend (check its URL and TLS certificate)")]
    Connect,
    #[error("inline backend transport failed")]
    Transport,
    #[error("inline backend answered HTTP {0}")]
    Status(u16),
    #[error("Ollama model does not support suffix insertion; use a FIM model with a suffix-aware template")]
    UnsupportedSuffix,
    #[error("inline backend response exceeds the size limit")]
    ResponseTooLarge,
    #[error("invalid inline backend response")]
    InvalidResponse,
}

impl From<reqwest::Error> for ProviderError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Timeout
        } else if error.is_connect() {
            Self::Connect
        } else {
            Self::Transport
        }
    }
}
