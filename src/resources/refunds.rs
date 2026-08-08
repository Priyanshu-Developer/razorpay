//! Refunds — returning money for a captured payment.

use serde::{Deserialize, Serialize};

use crate::client::RazorpayClient;
use crate::error::RazorpayError;
use crate::resources::common::{Notes, NotesUpdate};
use crate::util::pagination::{Collection, ListOptions};

/// How quickly a refund is processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefundSpeed {
    /// Processed at the normal rate, at no extra cost.
    Normal,
    /// Processed immediately where the instrument supports it; carries a fee.
    Optimum,
    /// A speed this crate does not model yet.
    #[serde(other)]
    Unknown,
}

/// Lifecycle state of a [`Refund`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefundState {
    /// Queued but not yet sent to the bank.
    Pending,
    /// The refund completed.
    Processed,
    /// The refund failed and the money stayed with you.
    Failed,
    /// A state this crate does not model yet.
    #[serde(other)]
    Unknown,
}

/// A refund returned by the API.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Refund {
    /// Unique identifier, e.g. `rfnd_FgRAHdNOM4ZVbO`.
    pub id: String,
    /// Always `"refund"`.
    #[serde(default)]
    pub entity: String,
    /// Amount refunded, in the smallest currency unit.
    pub amount: i64,
    /// ISO 4217 currency code.
    pub currency: String,
    /// The payment this refund is against.
    pub payment_id: String,
    /// Current state.
    #[serde(default)]
    pub status: Option<RefundState>,
    /// Speed requested at creation.
    #[serde(default)]
    pub speed_requested: Option<RefundSpeed>,
    /// Speed actually used.
    #[serde(default)]
    pub speed_processed: Option<RefundSpeed>,
    /// Your reference for this refund.
    #[serde(default)]
    pub receipt: Option<String>,
    /// Reference number to share with the customer for tracing.
    #[serde(default)]
    pub acquirer_data: Option<serde_json::Value>,
    /// The batch this refund belonged to, for bulk refunds.
    #[serde(default)]
    pub batch_id: Option<String>,
    /// Your metadata.
    #[serde(default)]
    pub notes: Notes,
    /// Creation time as a Unix timestamp in seconds.
    pub created_at: i64,
}

/// Parameters for creating a refund.
///
/// Construct with [`full`](Self::full) or [`partial`](Self::partial) — the
/// difference is only whether `amount` is sent.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CreateRefundParams {
    /// Amount to refund in the smallest unit. Omitted means refund everything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<i64>,
    /// Requested processing speed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<RefundSpeed>,
    /// Your reference for this refund.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<String>,
    /// Metadata to attach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Notes>,
}

impl CreateRefundParams {
    /// Refund the payment's entire remaining amount.
    ///
    /// ```
    /// use razorpay_sdk::resources::CreateRefundParams;
    /// assert!(CreateRefundParams::full().amount.is_none());
    /// ```
    pub fn full() -> Self {
        Self::default()
    }

    /// Refund only `amount`, in the smallest currency unit.
    ///
    /// ```
    /// use razorpay_sdk::resources::CreateRefundParams;
    /// assert_eq!(CreateRefundParams::partial(10_000).amount, Some(10_000));
    /// ```
    pub fn partial(amount: i64) -> Self {
        Self {
            amount: Some(amount),
            ..Self::default()
        }
    }

    /// Request a specific processing speed.
    pub fn speed(mut self, speed: RefundSpeed) -> Self {
        self.speed = Some(speed);
        self
    }

    /// Attach your own reference.
    pub fn receipt(mut self, receipt: impl Into<String>) -> Self {
        self.receipt = Some(receipt.into());
        self
    }

    /// Attach metadata.
    pub fn notes(mut self, notes: Notes) -> Self {
        self.notes = Some(notes);
        self
    }
}

/// Refund endpoints. Obtain one from [`RazorpayClient::refunds`].
///
/// Creating a refund lives on the payment it belongs to:
/// [`PaymentsClient::refund`](crate::resources::payments::PaymentsClient::refund).
pub struct RefundsClient<'a> {
    pub(crate) client: &'a RazorpayClient,
}

impl<'a> RefundsClient<'a> {
    /// Fetch one refund by id — `GET /v1/refunds/{id}`.
    pub async fn fetch(&self, id: &str) -> Result<Refund, RazorpayError> {
        self.client.get::<(), _>(&format!("refunds/{id}"), None).await
    }

    /// List all refunds across payments — `GET /v1/refunds`.
    pub async fn all(&self, options: ListOptions) -> Result<Collection<Refund>, RazorpayError> {
        self.client.get("refunds", Some(&options)).await
    }

    /// Replace a refund's notes — `PATCH /v1/refunds/{id}`.
    pub async fn edit(&self, id: &str, notes: Notes) -> Result<Refund, RazorpayError> {
        self.client
            .patch(&format!("refunds/{id}"), Some(&NotesUpdate::new(notes)))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_refund_omits_amount() {
        assert_eq!(serde_json::to_string(&CreateRefundParams::full()).unwrap(), "{}");
    }

    #[test]
    fn partial_refund_sends_amount() {
        let json = serde_json::to_string(&CreateRefundParams::partial(500)).unwrap();
        assert_eq!(json, r#"{"amount":500}"#);
    }

    #[test]
    fn speed_serializes_snake_case() {
        let json =
            serde_json::to_string(&CreateRefundParams::full().speed(RefundSpeed::Optimum)).unwrap();
        assert_eq!(json, r#"{"speed":"optimum"}"#);
    }
}
