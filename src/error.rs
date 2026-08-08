use reqwest::StatusCode;
use serde::Deserialize;

/// Everything that can go wrong in this crate.
///
/// The variants are kept distinct so retry logic can be written correctly — see
/// [`is_retryable`](RazorpayError::is_retryable).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RazorpayError {
    /// The request never completed — DNS, TLS, connection, or timeout failure.
    ///
    /// Usually transient, so generally safe to retry for idempotent calls.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// Razorpay returned a structured error.
    ///
    /// Boxed because it is by far the largest variant; inline it would put ~150
    /// bytes on every `Result` the crate returns, including the overwhelmingly
    /// common success path.
    #[error("api error [{code}]: {description}", code = .0.code, description = .0.description)]
    Api(Box<ApiError>),

    /// A non-2xx response that carried no parseable error envelope.
    ///
    /// Typically a gateway or proxy failure returning HTML rather than JSON. The
    /// raw body is preserved instead of being reported as a misleading decode error.
    #[error("unexpected http status {http_status}: {body}")]
    UnexpectedStatus {
        /// The HTTP status returned.
        http_status: StatusCode,
        /// The raw response body.
        body: String,
    },

    /// A request path could not be joined onto the base URL.
    #[error("invalid url: {0}")]
    InvalidUrl(String),

    /// A payment or webhook signature did not match.
    ///
    /// Treat this as a forgery attempt: do not fulfil the order.
    #[error("signature verification failed")]
    SignatureMismatch,

    /// The response was not the shape this crate expected.
    ///
    /// Means an API change or a bug in our types — never retryable, since the same
    /// request will produce the same undecodable response.
    #[error("failed to decode response: {0}")]
    Decode(#[from] serde_json::Error),
}

impl RazorpayError {
    /// Whether retrying the same request could plausibly succeed.
    ///
    /// True for transport failures and 5xx/429 responses; false for client errors,
    /// signature mismatches, and decode failures, which will fail identically on a
    /// retry.
    ///
    /// # Retrying is not automatically safe
    ///
    /// This reports whether the *server* might answer differently, not whether the
    /// call is safe to repeat. Retrying a `POST /orders` that actually succeeded
    /// but timed out creates a **duplicate order**. Only retry non-idempotent
    /// writes if you can deduplicate them yourself.
    ///
    /// # Examples
    ///
    /// ```
    /// # use razorpay_api::RazorpayError;
    /// # fn demo(err: RazorpayError) {
    /// if err.is_retryable() {
    ///     // back off and try again
    /// }
    /// # }
    /// ```
    pub fn is_retryable(&self) -> bool {
        match self {
            RazorpayError::Http(e) => e.is_timeout() || e.is_connect() || e.is_request(),
            RazorpayError::Api(e) => {
                e.http_status.is_server_error() || e.http_status == StatusCode::TOO_MANY_REQUESTS
            }
            RazorpayError::UnexpectedStatus { http_status, .. } => {
                http_status.is_server_error() || *http_status == StatusCode::TOO_MANY_REQUESTS
            }
            RazorpayError::InvalidUrl(_)
            | RazorpayError::SignatureMismatch
            | RazorpayError::Decode(_) => false,
        }
    }

    /// The HTTP status, when the error carries one.
    pub fn status(&self) -> Option<StatusCode> {
        match self {
            RazorpayError::Api(e) => Some(e.http_status),
            RazorpayError::UnexpectedStatus { http_status, .. } => Some(*http_status),
            RazorpayError::Http(e) => e.status(),
            _ => None,
        }
    }

    /// Razorpay's machine-readable error code, when this is an
    /// [`Api`](RazorpayError::Api) error.
    ///
    /// # Examples
    ///
    /// ```
    /// # use razorpay_api::RazorpayError;
    /// # fn demo(err: RazorpayError) {
    /// // Match on the code, never on the human-readable description.
    /// if err.code() == Some("BAD_REQUEST_ERROR") {
    ///     // handle a malformed request
    /// }
    /// # }
    /// ```
    pub fn code(&self) -> Option<&str> {
        match self {
            RazorpayError::Api(e) => Some(&e.code),
            _ => None,
        }
    }

    /// The structured error body, when this is an [`Api`](RazorpayError::Api) error.
    pub fn api_error(&self) -> Option<&ApiError> {
        match self {
            RazorpayError::Api(e) => Some(e),
            _ => None,
        }
    }
}

/// A structured error returned by Razorpay.
///
/// All six documented fields are preserved: collapsing them into a single string
/// loses `reason` and `step`, which are exactly the fields that explain *why* a
/// payment declined.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiError {
    /// Machine-readable code, e.g. `BAD_REQUEST_ERROR`.
    ///
    /// Match on this rather than [`description`](Self::description), which
    /// Razorpay may reword without notice.
    pub code: String,

    /// Human-readable message. Do not match on this text.
    #[serde(default)]
    pub description: String,

    /// Which request field was at fault, when applicable.
    #[serde(default)]
    pub field: Option<String>,

    /// Which side caused the failure, e.g. `business`, `customer`.
    ///
    /// Razorpay names this `source` on the wire; it is renamed here to avoid
    /// colliding with [`std::error::Error::source`].
    #[serde(default, rename = "source")]
    pub error_source: Option<String>,

    /// Which step of the flow failed, e.g. `payment_initiation`.
    #[serde(default)]
    pub step: Option<String>,

    /// Failure reason, e.g. `input_validation_failed`.
    #[serde(default)]
    pub reason: Option<String>,

    /// The HTTP status carried alongside the error.
    ///
    /// Needed for retry decisions: `code` is often the generic `BAD_REQUEST_ERROR`
    /// for both a 400 and a 404. Defaulted because it comes from the response, not
    /// the JSON body.
    #[serde(default = "ok_status", skip_deserializing)]
    pub http_status: StatusCode,
}

fn ok_status() -> StatusCode {
    StatusCode::OK
}

/// Razorpay's error envelope: `{"error": {"code", "description", ...}}`.
#[derive(Debug, Deserialize)]
pub(crate) struct ApiErrorEnvelope {
    pub error: ApiError,
}

impl ApiErrorEnvelope {
    pub(crate) fn into_error(self, http_status: StatusCode) -> RazorpayError {
        let mut error = self.error;
        error.http_status = http_status;
        RazorpayError::Api(Box::new(error))
    }
}