//! Payments — the money side of an [`Order`](crate::resources::orders::Order).
//!
//! The critical distinction in this module is **authorized vs captured**: see
//! [`PaymentsClient::capture`].

use serde::{Deserialize, Serialize};

use crate::client::RazorpayClient;
use crate::error::RazorpayError;
use crate::resources::common::{Notes, NotesUpdate, PaymentMethod};
use crate::resources::refunds::{CreateRefundParams, Refund};
use crate::util::pagination::{Collection, ListOptions};

/// Lifecycle state of a [`Payment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    /// Created but not yet acted on.
    Created,
    /// Funds are held but **not** settled to you — see [`PaymentsClient::capture`].
    Authorized,
    /// Funds captured; this is the only status that means you have been paid.
    Captured,
    /// Fully or partially refunded.
    Refunded,
    /// The payment failed.
    Failed,
    /// A status this crate does not model yet.
    #[serde(other)]
    Unknown,
}

/// How much of a payment has been refunded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefundStatus {
    /// No refund issued.
    Null,
    /// Some of the amount refunded.
    Partial,
    /// The whole amount refunded.
    Full,
    /// A status this crate does not model yet.
    #[serde(other)]
    Unknown,
}

/// A payment returned by the API.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Payment {
    /// Unique identifier, e.g. `pay_29QQoUBi66xm2f`.
    pub id: String,
    /// Always `"payment"`.
    #[serde(default)]
    pub entity: String,
    /// Amount in the smallest currency unit (paise for INR).
    pub amount: i64,
    /// ISO 4217 currency code.
    pub currency: String,
    /// Current lifecycle state.
    pub status: PaymentStatus,
    /// The order this payment was made against, when it came through an order.
    #[serde(default)]
    pub order_id: Option<String>,
    /// Invoice this payment settles, if any.
    #[serde(default)]
    pub invoice_id: Option<String>,
    /// Whether this payment is on an international card.
    #[serde(default)]
    pub international: bool,
    /// How the customer paid.
    #[serde(default)]
    pub method: Option<PaymentMethod>,
    /// Amount refunded so far, in the smallest unit.
    #[serde(default)]
    pub amount_refunded: i64,
    /// Whether the payment is partly or wholly refunded.
    #[serde(default)]
    pub refund_status: Option<RefundStatus>,
    /// Whether the payment has been captured.
    #[serde(default)]
    pub captured: bool,
    /// Free-text description shown to the customer.
    #[serde(default)]
    pub description: Option<String>,
    /// Card id, when [`method`](Self::method) is [`PaymentMethod::Card`].
    #[serde(default)]
    pub card_id: Option<String>,
    /// Bank code, for netbanking payments.
    #[serde(default)]
    pub bank: Option<String>,
    /// Wallet name, for wallet payments.
    #[serde(default)]
    pub wallet: Option<String>,
    /// VPA, for UPI payments.
    #[serde(default)]
    pub vpa: Option<String>,
    /// Customer email, when collected.
    #[serde(default)]
    pub email: Option<String>,
    /// Customer contact number, when collected.
    #[serde(default)]
    pub contact: Option<String>,
    /// Customer id, when the payment is tied to a saved customer.
    #[serde(default)]
    pub customer_id: Option<String>,
    /// Token id, for payments on a saved instrument.
    #[serde(default)]
    pub token_id: Option<String>,
    /// Razorpay's fee for this payment, in the smallest unit.
    #[serde(default)]
    pub fee: Option<i64>,
    /// Tax charged on the fee, in the smallest unit.
    #[serde(default)]
    pub tax: Option<i64>,
    /// Gateway error code, when the payment failed.
    #[serde(default)]
    pub error_code: Option<String>,
    /// Human-readable failure description.
    #[serde(default)]
    pub error_description: Option<String>,
    /// Failure reason, e.g. `payment_failed`.
    #[serde(default)]
    pub error_reason: Option<String>,
    /// Which side caused the failure, e.g. `customer`.
    #[serde(default)]
    pub error_source: Option<String>,
    /// Which step failed, e.g. `payment_authentication`.
    #[serde(default)]
    pub error_step: Option<String>,
    /// Your metadata.
    #[serde(default)]
    pub notes: Notes,
    /// Creation time as a Unix timestamp in seconds.
    pub created_at: i64,
}

impl Payment {
    /// Whether this payment represents money you will actually receive.
    ///
    /// An [`Authorized`](PaymentStatus::Authorized) payment is *not* settled —
    /// this returns `false` for it.
    pub fn is_captured(&self) -> bool {
        matches!(self.status, PaymentStatus::Captured)
    }
}

/// Body for [`PaymentsClient::capture`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CaptureParams {
    /// Amount to capture, in the smallest unit. Must equal the authorized amount.
    pub amount: i64,
    /// ISO 4217 currency code.
    pub currency: String,
}

/// Payment endpoints. Obtain one from [`RazorpayClient::payments`].
pub struct PaymentsClient<'a> {
    pub(crate) client: &'a RazorpayClient,
}

impl<'a> PaymentsClient<'a> {
    /// Fetch one payment by id — `GET /v1/payments/{id}`.
    pub async fn fetch(&self, id: &str) -> Result<Payment, RazorpayError> {
        self.client.get::<(), _>(&format!("payments/{id}"), None).await
    }

    /// List payments — `GET /v1/payments`.
    pub async fn all(&self, options: ListOptions) -> Result<Collection<Payment>, RazorpayError> {
        self.client.get("payments", Some(&options)).await
    }

    /// Capture an authorized payment — `POST /v1/payments/{id}/capture`.
    ///
    /// # An authorized payment is not money in your account
    ///
    /// When a payment is [`Authorized`](PaymentStatus::Authorized), the funds are
    /// only *held* on the customer's instrument. Razorpay **auto-refunds
    /// uncaptured payments** after a configurable window — typically days. A store
    /// that never calls this appears to work in testing and then silently refunds
    /// every order.
    ///
    /// You only need this if the order was created with
    /// [`manual_capture`](crate::resources::orders::CreateOrderParams::manual_capture)
    /// or your account defaults to manual capture; otherwise Razorpay captures for
    /// you.
    ///
    /// `amount` must match the authorized amount exactly, or Razorpay rejects it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use razorpay_sdk::{RazorpayClient, RazorpayError};
    /// # async fn demo(client: RazorpayClient) -> Result<(), RazorpayError> {
    /// let payment = client.payments().fetch("pay_123").await?;
    /// if !payment.is_captured() {
    ///     client.payments().capture("pay_123", payment.amount, &payment.currency).await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn capture(
        &self,
        id: &str,
        amount: i64,
        currency: &str,
    ) -> Result<Payment, RazorpayError> {
        let body = CaptureParams {
            amount,
            currency: currency.to_string(),
        };
        self.client
            .post(&format!("payments/{id}/capture"), Some(&body))
            .await
    }

    /// Replace a payment's notes — `PATCH /v1/payments/{id}`.
    pub async fn edit(&self, id: &str, notes: Notes) -> Result<Payment, RazorpayError> {
        self.client
            .patch(&format!("payments/{id}"), Some(&NotesUpdate::new(notes)))
            .await
    }

    /// Refund a payment — `POST /v1/payments/{id}/refund`.
    ///
    /// Omitting the amount on [`CreateRefundParams`] refunds the payment in full.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use razorpay_sdk::{RazorpayClient, RazorpayError};
    /// # use razorpay_sdk::resources::CreateRefundParams;
    /// # async fn demo(client: RazorpayClient) -> Result<(), RazorpayError> {
    /// // Full refund.
    /// client.payments().refund("pay_123", CreateRefundParams::full()).await?;
    ///
    /// // Partial refund of ₹100.
    /// client.payments().refund("pay_123", CreateRefundParams::partial(10_000)).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn refund(
        &self,
        id: &str,
        params: CreateRefundParams,
    ) -> Result<Refund, RazorpayError> {
        self.client
            .post(&format!("payments/{id}/refund"), Some(&params))
            .await
    }

    /// List refunds issued against a payment — `GET /v1/payments/{id}/refunds`.
    pub async fn refunds(
        &self,
        id: &str,
        options: ListOptions,
    ) -> Result<Collection<Refund>, RazorpayError> {
        self.client
            .get(&format!("payments/{id}/refunds"), Some(&options))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorized_is_not_captured() {
        let payment: Payment = serde_json::from_str(
            r#"{"id":"pay_1","amount":100,"currency":"INR","status":"authorized","created_at":1}"#,
        )
        .unwrap();
        assert!(!payment.is_captured());
    }

    #[test]
    fn unknown_method_and_status_decode_to_fallback() {
        let payment: Payment = serde_json::from_str(
            r#"{"id":"pay_1","amount":1,"currency":"INR","status":"new_status",
                "method":"crypto","created_at":1}"#,
        )
        .unwrap();
        assert_eq!(payment.status, PaymentStatus::Unknown);
        assert_eq!(payment.method, Some(PaymentMethod::Unknown));
    }

    #[test]
    fn sparse_payment_decodes() {
        // Failed payments omit most fields; decoding must not require them.
        let payment: Payment = serde_json::from_str(
            r#"{"id":"pay_1","amount":1,"currency":"INR","status":"failed","created_at":1}"#,
        )
        .unwrap();
        assert_eq!(payment.amount_refunded, 0);
        assert!(payment.notes.is_empty());
    }
}
