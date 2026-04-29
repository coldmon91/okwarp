pub use crate::aws_credentials::{AwsCredentials, AwsCredentialsState};
use serde::{Deserialize, Serialize};
use warp_multi_agent_api as api;
use warpui::{Entity, ModelContext, SingletonEntity};
use warpui_extras::secure_storage::{self, AppContextExt};

const SECURE_STORAGE_KEY: &str = "AiApiKeys";
pub const OPENAI_COMPATIBLE_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
pub const OPENAI_COMPATIBLE_DEFAULT_MODEL: &str = "gpt-4o-mini";

/// Emitted when user-provided API keys are updated in-memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeyManagerEvent {
    KeysUpdated,
}

/// User-provided API keys for AI providers.
///
/// These are used for "Bring Your Own API Key" functionality, allowing
/// users to use their own API keys instead of Warp's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenAICompatibleProviderConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl OpenAICompatibleProviderConfig {
    pub fn effective_base_url(&self) -> &str {
        let base_url = self.base_url.trim();
        if base_url.is_empty() {
            OPENAI_COMPATIBLE_DEFAULT_BASE_URL
        } else {
            base_url
        }
    }

    pub fn effective_model(&self) -> &str {
        let model = self.model.trim();
        if model.is_empty() {
            OPENAI_COMPATIBLE_DEFAULT_MODEL
        } else {
            model
        }
    }

    pub fn effective_api_key(&self) -> Option<&str> {
        self.api_key
            .as_deref()
            .map(str::trim)
            .filter(|api_key| !api_key.is_empty())
    }
}

impl Default for OpenAICompatibleProviderConfig {
    fn default() -> Self {
        Self {
            base_url: OPENAI_COMPATIBLE_DEFAULT_BASE_URL.to_string(),
            model: OPENAI_COMPATIBLE_DEFAULT_MODEL.to_string(),
            api_key: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiKeys {
    pub google: Option<String>,
    pub anthropic: Option<String>,
    pub openai: Option<String>,
    pub open_router: Option<String>,
    pub openai_compatible: OpenAICompatibleProviderConfig,
}

impl ApiKeys {
    pub fn has_any_key(&self) -> bool {
        self.openai.is_some()
            || self.anthropic.is_some()
            || self.google.is_some()
            || self.open_router.is_some()
            || self.openai_compatible.effective_api_key().is_some()
    }

    pub fn openai_compatible_provider_config(&self) -> OpenAICompatibleProviderConfig {
        let mut config = self.openai_compatible.clone();
        if config.effective_api_key().is_none() {
            config.api_key = self.openai.clone();
        }
        config
    }

    fn openai_key_for_request(&self) -> Option<String> {
        self.openai_compatible_provider_config()
            .effective_api_key()
            .map(ToOwned::to_owned)
    }

    fn normalize_after_load(&mut self) -> bool {
        let mut changed = false;

        if self.openai_compatible.base_url.trim().is_empty() {
            self.openai_compatible.base_url = OPENAI_COMPATIBLE_DEFAULT_BASE_URL.to_string();
            changed = true;
        }

        if self.openai_compatible.model.trim().is_empty() {
            self.openai_compatible.model = OPENAI_COMPATIBLE_DEFAULT_MODEL.to_string();
            changed = true;
        }

        if self.openai_compatible.effective_api_key().is_none() && self.openai.is_some() {
            self.openai_compatible.api_key = self.openai.clone();
            changed = true;
        }

        if self.openai.is_none() && self.openai_compatible.effective_api_key().is_some() {
            self.openai = self.openai_compatible.api_key.clone();
            changed = true;
        }

        changed
    }
}

/// Controls how AWS credentials are refreshed by [`ApiKeyManager`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AwsCredentialsRefreshStrategy {
    /// Load credentials from the local AWS credential chain (~/.aws). This is the default.
    #[default]
    LocalChain,
    /// Credentials are managed externally via OIDC/STS.
    /// The task ID is used to scope the STS AssumeRoleWithWebIdentity session.
    /// The role ARN is the IAM role to assume via STS.
    OidcManaged {
        task_id: Option<String>,
        role_arn: String,
    },
}

/// A structure that manages API keys for AI providers.
pub struct ApiKeyManager {
    keys: ApiKeys,
    pub(crate) aws_credentials_state: AwsCredentialsState,
    aws_credentials_refresh_strategy: AwsCredentialsRefreshStrategy,
}

impl ApiKeyManager {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let (keys, should_persist_keys) = Self::load_keys_from_secure_storage(ctx);
        let mut manager = Self {
            keys,
            aws_credentials_state: AwsCredentialsState::Missing,
            aws_credentials_refresh_strategy: AwsCredentialsRefreshStrategy::default(),
        };
        if should_persist_keys {
            manager.write_keys_to_secure_storage(ctx);
        }
        manager
    }

    pub fn keys(&self) -> &ApiKeys {
        &self.keys
    }

    pub fn set_google_key(&mut self, key: Option<String>, ctx: &mut ModelContext<Self>) {
        self.keys.google = key;
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn set_anthropic_key(&mut self, key: Option<String>, ctx: &mut ModelContext<Self>) {
        self.keys.anthropic = key;
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn set_openai_key(&mut self, key: Option<String>, ctx: &mut ModelContext<Self>) {
        self.keys.openai = key.clone();
        self.keys.openai_compatible.api_key = key;
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn set_openai_compatible_api_key(
        &mut self,
        key: Option<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        self.set_openai_key(key, ctx);
    }

    pub fn set_openai_compatible_base_url(
        &mut self,
        base_url: Option<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        self.keys.openai_compatible.base_url = base_url
            .map(|base_url| base_url.trim().to_string())
            .filter(|base_url| !base_url.is_empty())
            .unwrap_or_else(|| OPENAI_COMPATIBLE_DEFAULT_BASE_URL.to_string());
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn set_openai_compatible_model(
        &mut self,
        model: Option<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        self.keys.openai_compatible.model = model
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty())
            .unwrap_or_else(|| OPENAI_COMPATIBLE_DEFAULT_MODEL.to_string());
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn set_open_router_key(&mut self, key: Option<String>, ctx: &mut ModelContext<Self>) {
        self.keys.open_router = key;
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
        self.write_keys_to_secure_storage(ctx);
    }

    pub fn set_aws_credentials_state(
        &mut self,
        state: AwsCredentialsState,
        ctx: &mut ModelContext<Self>,
    ) {
        self.aws_credentials_state = state;
        ctx.emit(ApiKeyManagerEvent::KeysUpdated);
    }

    pub fn aws_credentials_state(&self) -> &AwsCredentialsState {
        &self.aws_credentials_state
    }

    pub fn aws_credentials_refresh_strategy(&self) -> AwsCredentialsRefreshStrategy {
        self.aws_credentials_refresh_strategy.clone()
    }

    pub fn set_aws_credentials_refresh_strategy(
        &mut self,
        strategy: AwsCredentialsRefreshStrategy,
    ) {
        self.aws_credentials_refresh_strategy = strategy;
    }

    pub fn api_keys_for_request(
        &self,
        include_byo_keys: bool,
        include_aws_bedrock_credentials: bool,
    ) -> Option<api::request::settings::ApiKeys> {
        let anthropic = include_byo_keys
            .then(|| self.keys.anthropic.clone())
            .flatten()
            .unwrap_or_default();
        let openai = include_byo_keys
            .then(|| self.keys.openai_key_for_request())
            .flatten()
            .unwrap_or_default();
        let google = include_byo_keys
            .then(|| self.keys.google.clone())
            .flatten()
            .unwrap_or_default();
        let open_router = include_byo_keys
            .then(|| self.keys.open_router.clone())
            .flatten()
            .unwrap_or_default();
        // Also include credentials when running with OIDC-managed Bedrock inference, regardless
        // of the per-user setting flag (which only applies to the local credential chain path).
        let include_aws = include_aws_bedrock_credentials
            || matches!(
                self.aws_credentials_refresh_strategy,
                AwsCredentialsRefreshStrategy::OidcManaged { .. }
            );
        let aws_credentials = include_aws
            .then(|| match self.aws_credentials_state {
                AwsCredentialsState::Loaded {
                    ref credentials, ..
                } => Some(credentials.clone().into()),
                _ => None,
            })
            .flatten();

        if anthropic.is_empty()
            && openai.is_empty()
            && google.is_empty()
            && open_router.is_empty()
            && aws_credentials.is_none()
        {
            None
        } else {
            Some(api::request::settings::ApiKeys {
                anthropic,
                openai,
                google,
                open_router,
                allow_use_of_warp_credits: false,
                aws_credentials,
            })
        }
    }

    fn load_keys_from_secure_storage(ctx: &mut ModelContext<Self>) -> (ApiKeys, bool) {
        let key_json = match ctx.secure_storage().read_value(SECURE_STORAGE_KEY) {
            Ok(json) => json,
            Err(e) => {
                if !matches!(e, secure_storage::Error::NotFound) {
                    log::error!("Failed to read API keys from secure storage: {e:#}");
                }
                return (ApiKeys::default(), false);
            }
        };

        let mut keys = match serde_json::from_str(&key_json) {
            Ok(keys) => keys,
            Err(e) => {
                log::error!("Failed to deserialize API keys: {e:#}");
                ApiKeys::default()
            }
        };
        let should_persist_keys = keys.normalize_after_load();

        (keys, should_persist_keys)
    }

    fn write_keys_to_secure_storage(&mut self, ctx: &mut ModelContext<Self>) {
        let keys = self.keys.clone();

        let json = match serde_json::to_string(&keys) {
            Ok(json) => json,
            Err(e) => {
                log::error!("Failed to serialize API keys: {e:#}");
                return;
            }
        };

        if let Err(e) = ctx.secure_storage().write_value(SECURE_STORAGE_KEY, &json) {
            log::error!("Failed to write API keys to secure storage: {e:#}");
        }
    }
}

impl Entity for ApiKeyManager {
    type Event = ApiKeyManagerEvent;
}

impl SingletonEntity for ApiKeyManager {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_compatible_config_uses_defaults() {
        let config = OpenAICompatibleProviderConfig::default();

        assert_eq!(
            config.effective_base_url(),
            OPENAI_COMPATIBLE_DEFAULT_BASE_URL
        );
        assert_eq!(config.effective_model(), OPENAI_COMPATIBLE_DEFAULT_MODEL);
        assert_eq!(config.effective_api_key(), None);
    }

    #[test]
    fn legacy_openai_key_migrates_to_openai_compatible_provider() {
        let mut keys = ApiKeys {
            openai: Some("sk-legacy".to_string()),
            ..Default::default()
        };

        assert!(keys.normalize_after_load());
        assert_eq!(
            keys.openai_compatible.effective_api_key(),
            Some("sk-legacy")
        );
        assert_eq!(keys.openai_key_for_request().as_deref(), Some("sk-legacy"));
    }

    #[test]
    fn openai_compatible_provider_key_mirrors_legacy_request_key() {
        let mut keys = ApiKeys {
            openai_compatible: OpenAICompatibleProviderConfig {
                api_key: Some("sk-provider".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(keys.normalize_after_load());
        assert_eq!(keys.openai.as_deref(), Some("sk-provider"));
        assert_eq!(
            keys.openai_key_for_request().as_deref(),
            Some("sk-provider")
        );
    }
}
