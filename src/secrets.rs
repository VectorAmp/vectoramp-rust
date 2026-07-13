//! Organization-scoped provider secret helpers.

use serde_json::json;

use crate::client::Client;
use crate::errors::Result;
use crate::transport::Request;

/// Secret reference used by the API for organization OpenAI embedding keys.
pub const OPENAI_API_KEY_SECRET_REF: &str = "emb:openai:api_key";

/// Service for storing/updating provider secrets for the authenticated organization.
#[derive(Clone)]
pub struct OrgSecretService {
    client: Client,
}

impl OrgSecretService {
    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }

    /// Store or update an organization secret by name.
    pub async fn put<N: AsRef<str>, V: Into<String>>(&self, name: N, value: V) -> Result<()> {
        self.client
            .dispatcher()
            .empty(Request {
                method: "PUT".into(),
                path: format!("/org-secrets/{}", urlencoding::encode(name.as_ref())),
                body: Some(json!({ "value": value.into() })),
                ..Default::default()
            })
            .await
    }

    /// Returns `Ok(())` when an organization secret is present; a 404 is
    /// returned as an API error when it is missing.
    pub async fn has<N: AsRef<str>>(&self, name: N) -> Result<()> {
        self.client
            .dispatcher()
            .empty(Request {
                method: "GET".into(),
                path: format!("/org-secrets/{}", urlencoding::encode(name.as_ref())),
                ..Default::default()
            })
            .await
    }

    /// Store or update the organization OpenAI API key.
    pub async fn put_openai_api_key<S: Into<String>>(&self, api_key: S) -> Result<()> {
        self.put(OPENAI_API_KEY_SECRET_REF, api_key).await
    }

    /// Alias for [`OrgSecretService::put_openai_api_key`]; the API upserts.
    pub async fn update_openai_api_key<S: Into<String>>(&self, api_key: S) -> Result<()> {
        self.put_openai_api_key(api_key).await
    }

    /// Returns `Ok(())` when an organization OpenAI API key is present; a 404
    /// is returned as an API error when it is missing.
    pub async fn has_openai_api_key(&self) -> Result<()> {
        self.has(OPENAI_API_KEY_SECRET_REF).await
    }
}
