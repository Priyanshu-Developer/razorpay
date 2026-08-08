//! Typed webhook events.
//!
//! # Always verify before parsing
//!
//! Parsing tells you what an event *says*; it does not tell you Razorpay sent it.
//! Verify the `X-Razorpay-Signature` header against the **raw request body** with
//! [`crate::signature::verify_webhook_signature`] first,
//! then parse that same string. See [`WebhookEvent::parse_verified`], which does
//! both in the right order.

use serde::Deserialize;

use crate::error::RazorpayError;
use crate::resources::invoices::Invoice;
use crate::resources::orders::Order;
use crate::resources::payments::Payment;
use crate::resources::refunds::Refund;
use crate::resources::subscriptions::Subscription;
use crate::signature::verify_webhook_signature;

/// Razorpay nests every entity one level deep, as `{"entity": {...}}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Wrapped<T> {
    /// The entity itself.
    pub entity: T,
}

/// Payload of a payment-related event.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PaymentPayload {
    /// The payment the event is about.
    pub payment: Wrapped<Payment>,
    /// The order it belongs to, when Razorpay includes it.
    #[serde(default)]
    pub order: Option<Wrapped<Order>>,
}

/// Payload of an order-related event.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OrderPayload {
    /// The order the event is about.
    pub order: Wrapped<Order>,
    /// The payment that paid it, when included.
    #[serde(default)]
    pub payment: Option<Wrapped<Payment>>,
}

/// Payload of a refund-related event.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RefundPayload {
    /// The refund the event is about.
    pub refund: Wrapped<Refund>,
    /// The payment being refunded, when included.
    #[serde(default)]
    pub payment: Option<Wrapped<Payment>>,
}

/// Payload of a subscription-related event.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SubscriptionPayload {
    /// The subscription the event is about.
    pub subscription: Wrapped<Subscription>,
}

/// Payload of an invoice-related event.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InvoicePayload {
    /// The invoice the event is about.
    pub invoice: Wrapped<Invoice>,
    /// The payment that settled it, when included.
    #[serde(default)]
    pub payment: Option<Wrapped<Payment>>,
}

/// A webhook event, dispatched on its `event` field.
///
/// [`Unknown`](WebhookEvent::Unknown) catches every event this crate does not
/// model. You enable events in the Razorpay dashboard independently of this
/// crate's version, so an unmodelled event must not break your handler.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "event", content = "payload", remote = "Self")]
pub enum WebhookEvent {
    /// A payment was authorized but not yet captured.
    #[serde(rename = "payment.authorized")]
    PaymentAuthorized(PaymentPayload),

    /// A payment was captured — the money is yours.
    #[serde(rename = "payment.captured")]
    PaymentCaptured(PaymentPayload),

    /// A payment attempt failed.
    #[serde(rename = "payment.failed")]
    PaymentFailed(PaymentPayload),

    /// An order was paid in full.
    #[serde(rename = "order.paid")]
    OrderPaid(OrderPayload),

    /// A refund was created.
    #[serde(rename = "refund.created")]
    RefundCreated(RefundPayload),

    /// A refund completed.
    #[serde(rename = "refund.processed")]
    RefundProcessed(RefundPayload),

    /// A refund failed; the money stayed with you.
    #[serde(rename = "refund.failed")]
    RefundFailed(RefundPayload),

    /// A subscription was authenticated by the customer.
    #[serde(rename = "subscription.authenticated")]
    SubscriptionAuthenticated(SubscriptionPayload),

    /// A subscription became active.
    #[serde(rename = "subscription.activated")]
    SubscriptionActivated(SubscriptionPayload),

    /// A subscription was charged for a cycle.
    #[serde(rename = "subscription.charged")]
    SubscriptionCharged(SubscriptionPayload),

    /// A subscription was cancelled.
    #[serde(rename = "subscription.cancelled")]
    SubscriptionCancelled(SubscriptionPayload),

    /// A subscription was paused.
    #[serde(rename = "subscription.paused")]
    SubscriptionPaused(SubscriptionPayload),

    /// A subscription ran out of retries.
    #[serde(rename = "subscription.halted")]
    SubscriptionHalted(SubscriptionPayload),

    /// A subscription finished its scheduled cycles.
    #[serde(rename = "subscription.completed")]
    SubscriptionCompleted(SubscriptionPayload),

    /// An invoice was paid.
    #[serde(rename = "invoice.paid")]
    InvoicePaid(InvoicePayload),

    /// An invoice went past its due date.
    #[serde(rename = "invoice.expired")]
    InvoiceExpired(InvoicePayload),

    /// An event this crate does not model, or one whose payload did not match the
    /// shape expected for its type.
    #[serde(other)]
    Unknown,
}

// `#[serde(other)]` only accepts a unit variant, and a unit variant cannot absorb
// the `payload` object that every real event carries — so the derived impl alone
// errors on unmodelled events, exactly the breakage `Unknown` exists to prevent.
// Deriving onto a remote shadow and falling back here keeps the variant list
// declarative while making the fallback total.
impl<'de> serde::Deserialize<'de> for WebhookEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(WebhookEvent::deserialize(value).unwrap_or(WebhookEvent::Unknown))
    }
}

impl WebhookEvent {
    /// Verify `raw_body` against `signature`, then parse it.
    ///
    /// This is the only entry point that cannot be used in the wrong order. It
    /// returns [`RazorpayError::SignatureMismatch`] if the body was not signed by
    /// `webhook_secret`, and never parses in that case.
    ///
    /// `raw_body` must be the **exact bytes** of the request. Do not parse and
    /// re-serialize before calling — that changes the digest and always fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use razorpay_sdk::resources::WebhookEvent;
    /// use razorpay_sdk::signature::compute_signature;
    ///
    /// let secret = "webhook_secret";
    /// let raw_body = r#"{"event":"payment.captured","payload":{"payment":{"entity":
    ///     {"id":"pay_1","amount":50000,"currency":"INR","status":"captured","created_at":1}}}}"#;
    /// let signature = compute_signature(raw_body, secret);
    ///
    /// match WebhookEvent::parse_verified(raw_body, &signature, secret)? {
    ///     WebhookEvent::PaymentCaptured(p) => {
    ///         assert_eq!(p.payment.entity.id, "pay_1");
    ///     }
    ///     other => panic!("unexpected event: {other:?}"),
    /// }
    ///
    /// // A forged body is rejected before parsing.
    /// assert!(WebhookEvent::parse_verified(raw_body, "00", secret).is_err());
    /// # Ok::<(), razorpay_sdk::RazorpayError>(())
    /// ```
    pub fn parse_verified(
        raw_body: &str,
        signature: &str,
        webhook_secret: &str,
    ) -> Result<Self, RazorpayError> {
        verify_webhook_signature(raw_body, signature, webhook_secret)?;
        Ok(serde_json::from_str(raw_body)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature::compute_signature;

    const CAPTURED: &str = r#"{"event":"payment.captured","payload":{"payment":{"entity":
        {"id":"pay_1","amount":50000,"currency":"INR","status":"captured","created_at":1}}}}"#;

    #[test]
    fn parses_payment_captured() {
        let event: WebhookEvent = serde_json::from_str(CAPTURED).unwrap();
        match event {
            WebhookEvent::PaymentCaptured(p) => {
                assert_eq!(p.payment.entity.id, "pay_1");
                assert!(p.payment.entity.is_captured());
            }
            other => panic!("expected PaymentCaptured, got {other:?}"),
        }
    }

    #[test]
    fn unmodelled_event_becomes_unknown_rather_than_erroring() {
        let json = r#"{"event":"virtual_account.credited","payload":{"anything":1}}"#;
        assert_eq!(serde_json::from_str::<WebhookEvent>(json).unwrap(), WebhookEvent::Unknown);
    }

    #[test]
    fn parse_verified_rejects_bad_signature_before_parsing() {
        let err = WebhookEvent::parse_verified(CAPTURED, "deadbeef", "whsec").unwrap_err();
        assert!(matches!(err, RazorpayError::SignatureMismatch));
    }

    #[test]
    fn parse_verified_accepts_good_signature() {
        let sig = compute_signature(CAPTURED, "whsec");
        let event = WebhookEvent::parse_verified(CAPTURED, &sig, "whsec").unwrap();
        assert!(matches!(event, WebhookEvent::PaymentCaptured(_)));
    }

    #[test]
    fn subscription_charged_parses() {
        let json = r#"{"event":"subscription.charged","payload":{"subscription":{"entity":
            {"id":"sub_1","plan_id":"plan_1","status":"active"}}}}"#;
        match serde_json::from_str::<WebhookEvent>(json).unwrap() {
            WebhookEvent::SubscriptionCharged(s) => assert_eq!(s.subscription.entity.id, "sub_1"),
            other => panic!("expected SubscriptionCharged, got {other:?}"),
        }
    }
}
