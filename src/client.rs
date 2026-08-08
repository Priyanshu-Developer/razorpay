use std::time::Duration;

use reqwest::{Method, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{ApiErrorEnvelope, RazorpayError};

const DEFAULT_BASE_URL: &str = "https://api.razorpay.com/v1/";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const USER_AGENT: &str = concat!("razorpay-api-rust/", env!("CARGO_PKG_VERSION"));

/// How the client authenticates.
///
/// An enum rather than two `Option` fields: two `Option`s make four states, of
/// which two ("both set" and "neither set") are invalid. This makes those
/// unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    /// HTTP Basic auth with an API key pair — the usual mode.
    BasicAuth {
        /// The key id from your Razorpay dashboard.
        api_key: String,
        /// The matching key secret.
        api_secret: String,
    },
    /// Bearer token auth, used for OAuth-issued access tokens.
    BearerToken {
        /// The access token.
        token: String,
    },
}

/// The entry point to the API.
///
/// Construct one at startup and share it. [`reqwest::Client`] holds a connection
/// pool internally, so building a client per request discards pooling and TLS
/// session reuse and will exhaust sockets under load. Cloning is cheap — the
/// underlying HTTP client is reference-counted.
///
/// # Examples
///
/// ```
/// use razorpay_api::RazorpayClient;
///
/// let client = RazorpayClient::new("rzp_test_key".into(), "secret".into());
/// assert_eq!(client.base_url().as_str(), "https://api.razorpay.com/v1/");
/// ```
#[derive(Debug, Clone)]
pub struct RazorpayClient {
    http_client: reqwest::Client,
    base_url: Url,
    auth_method: AuthMethod,
}

impl RazorpayClient {
    /// A client authenticating with an API key pair.
    ///
    /// Get these from the Razorpay dashboard. Keep the secret out of source
    /// control — read it from the environment or a secret manager.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use razorpay_api::RazorpayClient;
    ///
    /// let client = RazorpayClient::new(
    ///     std::env::var("RAZORPAY_KEY_ID").unwrap(),
    ///     std::env::var("RAZORPAY_KEY_SECRET").unwrap(),
    /// );
    /// ```
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self::with_auth(AuthMethod::BasicAuth { api_key, api_secret })
    }

    /// A client authenticating with an OAuth access token.
    pub fn new_with_bearer_token(token: String) -> Self {
        Self::with_auth(AuthMethod::BearerToken { token })
    }

    /// The base URL requests are sent to.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// The configured authentication method.
    ///
    /// Note this exposes the key secret; avoid logging the result.
    pub fn auth_method(&self) -> &AuthMethod {
        &self.auth_method
    }

    fn with_auth(auth_method: AuthMethod) -> Self {
        RazorpayClient {
            http_client: reqwest::Client::builder()
                .timeout(DEFAULT_TIMEOUT)
                .user_agent(USER_AGENT)
                .build()
                .expect("failed to build HTTP client"),
            base_url: DEFAULT_BASE_URL.parse().expect("Invalid base URL"),
            auth_method,
        }
    }

    /// Point the client at a different host.
    ///
    /// Exists so the crate is testable without network access: point it at a mock
    /// server and every request goes there instead. Keep the trailing slash —
    /// [`Url::join`] discards the last path segment without one.
    ///
    /// # Examples
    ///
    /// ```
    /// # use razorpay_api::RazorpayClient;
    /// let client = RazorpayClient::new("k".into(), "s".into())
    ///     .with_base_url("http://127.0.0.1:8080/v1/".parse().unwrap());
    /// assert_eq!(client.base_url().as_str(), "http://127.0.0.1:8080/v1/");
    /// ```
    pub fn with_base_url(mut self, base_url: Url) -> Self {
        self.base_url = base_url;
        self
    }

    /// Join a resource path onto the base URL.
    ///
    /// `Url::join` treats a leading `/` as absolute (discarding any base path), so
    /// paths are trimmed and the base is kept trailing-slashed at construction.
    fn url_for(&self, path: &str) -> Result<Url, RazorpayError> {
        self.base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| RazorpayError::InvalidUrl(e.to_string()))
    }

    /// The single place every API call funnels through: auth, query, body, send,
    /// status check, then parse.
    async fn request<Q, B, R>(
        &self,
        method: Method,
        path: &str,
        query: Option<&Q>,
        body: Option<&B>,
    ) -> Result<R, RazorpayError>
    where
        Q: Serialize + ?Sized,
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let mut req = self.http_client.request(method, self.url_for(path)?);

        req = match &self.auth_method {
            AuthMethod::BasicAuth { api_key, api_secret } => {
                req.basic_auth(api_key, Some(api_secret))
            }
            AuthMethod::BearerToken { token } => req.bearer_auth(token),
        };

        if let Some(query) = query {
            req = req.query(query);
        }
        if let Some(body) = body {
            req = req.json(body);
        }

        let response = req.send().await?;
        let status = response.status();
        let bytes = response.bytes().await?;

        // Razorpay sometimes answers 200 with an error envelope, so the body is
        // probed for one regardless of status rather than trusting the code alone.
        if let Ok(envelope) = serde_json::from_slice::<ApiErrorEnvelope>(&bytes) {
            return Err(envelope.into_error(status));
        }

        if !status.is_success() {
            // Non-2xx that didn't carry a parseable envelope — surface the raw body
            // instead of a misleading `Decode` error.
            return Err(RazorpayError::UnexpectedStatus {
                http_status: status,
                body: String::from_utf8_lossy(&bytes).into_owned(),
            });
        }

        // A 204 (and some DELETEs) return an empty body; `()` and `Option<T>` still
        // need valid JSON, so an empty body is normalized to `null`.
        let bytes: &[u8] = if bytes.is_empty() { b"null" } else { &bytes };
        Ok(serde_json::from_slice(bytes)?)
    }

    pub(crate) async fn get<Q, R>(&self, path: &str, query: Option<&Q>) -> Result<R, RazorpayError>
    where
        Q: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.request::<Q, (), R>(Method::GET, path, query, None).await
    }

    pub(crate) async fn post<B, R>(&self, path: &str, body: Option<&B>) -> Result<R, RazorpayError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.request::<(), B, R>(Method::POST, path, None, body).await
    }

    pub(crate) async fn patch<B, R>(&self, path: &str, body: Option<&B>) -> Result<R, RazorpayError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.request::<(), B, R>(Method::PATCH, path, None, body).await
    }

    pub(crate) async fn put<B, R>(&self, path: &str, body: Option<&B>) -> Result<R, RazorpayError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.request::<(), B, R>(Method::PUT, path, None, body).await
    }

    pub(crate) async fn delete<R>(&self, path: &str) -> Result<R, RazorpayError>
    where
        R: DeserializeOwned,
    {
        self.request::<(), (), R>(Method::DELETE, path, None, None).await
    }
}

/// Resource accessors.
///
/// Each returns a zero-cost borrowing view rather than an owned client, so
/// `client.orders().fetch(id)` allocates nothing and the whole API does not
/// collapse into a hundred flat methods on [`RazorpayClient`].
impl RazorpayClient {
    /// [Orders](crate::resources::orders) — create these before opening Checkout.
    pub fn orders(&self) -> crate::resources::orders::OrdersClient<'_> {
        crate::resources::orders::OrdersClient { client: self }
    }

    /// [Payments](crate::resources::payments) — fetch, capture, and refund.
    pub fn payments(&self) -> crate::resources::payments::PaymentsClient<'_> {
        crate::resources::payments::PaymentsClient { client: self }
    }

    /// [Refunds](crate::resources::refunds) — fetch and list issued refunds.
    pub fn refunds(&self) -> crate::resources::refunds::RefundsClient<'_> {
        crate::resources::refunds::RefundsClient { client: self }
    }

    /// [Customers](crate::resources::customers) — saved payer identities.
    pub fn customers(&self) -> crate::resources::customers::CustomersClient<'_> {
        crate::resources::customers::CustomersClient { client: self }
    }

    /// [Cards](crate::resources::cards) — read card details behind a payment.
    pub fn cards(&self) -> crate::resources::cards::CardsClient<'_> {
        crate::resources::cards::CardsClient { client: self }
    }

    /// [Items](crate::resources::items) — reusable priced line items.
    pub fn items(&self) -> crate::resources::items::ItemsClient<'_> {
        crate::resources::items::ItemsClient { client: self }
    }

    /// [Plans](crate::resources::plans) — billing cadences for subscriptions.
    pub fn plans(&self) -> crate::resources::plans::PlansClient<'_> {
        crate::resources::plans::PlansClient { client: self }
    }

    /// [Subscriptions](crate::resources::subscriptions) — recurring billing.
    pub fn subscriptions(&self) -> crate::resources::subscriptions::SubscriptionsClient<'_> {
        crate::resources::subscriptions::SubscriptionsClient { client: self }
    }

    /// [Invoices](crate::resources::invoices) — itemized requests for payment.
    pub fn invoices(&self) -> crate::resources::invoices::InvoicesClient<'_> {
        crate::resources::invoices::InvoicesClient { client: self }
    }

    /// [Payment links](crate::resources::payment_links) — shareable payment URLs.
    pub fn payment_links(&self) -> crate::resources::payment_links::PaymentLinksClient<'_> {
        crate::resources::payment_links::PaymentLinksClient { client: self }
    }
}

/// Public mirrors of the private wrappers, for integration tests only. Enabled by
/// the `test-util` feature so the real surface stays crate-private.
#[cfg(feature = "test-util")]
impl RazorpayClient {
    /// Issue a raw `GET`. Test-only; use the resource accessors instead.
    pub async fn get_public<Q, R>(&self, path: &str, query: Option<&Q>) -> Result<R, RazorpayError>
    where
        Q: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.get(path, query).await
    }

    /// Issue a raw `POST`. Test-only; use the resource accessors instead.
    pub async fn post_public<B, R>(&self, path: &str, body: Option<&B>) -> Result<R, RazorpayError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.post(path, body).await
    }

    /// Issue a raw `PATCH`. Test-only; use the resource accessors instead.
    pub async fn patch_public<B, R>(&self, path: &str, body: Option<&B>) -> Result<R, RazorpayError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.patch(path, body).await
    }

    /// Issue a raw `PUT`. Test-only; use the resource accessors instead.
    pub async fn put_public<B, R>(&self, path: &str, body: Option<&B>) -> Result<R, RazorpayError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.put(path, body).await
    }

    /// Issue a raw `DELETE`. Test-only; use the resource accessors instead.
    pub async fn delete_public<R>(&self, path: &str) -> Result<R, RazorpayError>
    where
        R: DeserializeOwned,
    {
        self.delete(path).await
    }
}

#[cfg(test)]
mod send_sync_check {
    use super::*;
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    const _: () = assert_send_sync::<RazorpayClient>();
    const _: () = assert_send_sync::<AuthMethod>();
}
