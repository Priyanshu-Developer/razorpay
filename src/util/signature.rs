//! HMAC-SHA256 signature verification for payments and webhooks.
//!
//! This is the crate's security boundary. Both functions compare digests in
//! constant time via [`subtle`], because a short-circuiting `==` on a signature
//! leaks the correct prefix to anyone who can measure response latency, turning an
//! infeasible search into a byte-at-a-time one.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::error::RazorpayError;

type HmacSha256 = Hmac<Sha256>;

/// Compute the hex-encoded HMAC-SHA256 of `message` under `secret`.
///
/// Exposed because a handful of Razorpay flows (Smart Collect, custom checkout
/// variants) sign payloads this crate does not model; you can build the message
/// yourself and compare with [`verify_signature`].
///
/// # Examples
///
/// ```
/// use razorpay_sdk::signature::compute_signature;
///
/// let sig = compute_signature("hello", "secret");
/// assert_eq!(sig, "88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b");
/// ```
pub fn compute_signature(message: &str, secret: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Verify that `signature` is the HMAC-SHA256 of `message` under `secret`.
///
/// Returns [`RazorpayError::SignatureMismatch`] on any mismatch, including a
/// malformed (non-hex or wrong-length) signature.
///
/// The comparison is constant-time.
///
/// # Examples
///
/// ```
/// use razorpay_sdk::signature::{compute_signature, verify_signature};
///
/// let expected = compute_signature("hello", "secret");
/// assert!(verify_signature("hello", &expected, "secret").is_ok());
/// assert!(verify_signature("hello", "deadbeef", "secret").is_err());
/// ```
#[must_use = "an ignored verification result is the same as no verification at all"]
pub fn verify_signature(message: &str, signature: &str, secret: &str) -> Result<(), RazorpayError> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(message.as_bytes());
    let expected = mac.finalize().into_bytes();

    // Decode rather than hex-encoding ours and comparing strings: a string compare
    // would also be case-sensitive, and decoding rejects malformed input up front.
    let provided = match hex::decode(signature) {
        Ok(bytes) => bytes,
        Err(_) => return Err(RazorpayError::SignatureMismatch),
    };

    if provided.len() != expected.len() {
        return Err(RazorpayError::SignatureMismatch);
    }

    if provided.ct_eq(&expected).into() {
        Ok(())
    } else {
        Err(RazorpayError::SignatureMismatch)
    }
}

/// Verify the signature Razorpay Checkout hands back after a payment.
///
/// The signature is HMAC-SHA256 of `"{order_id}|{payment_id}"` keyed by your **API
/// key secret**.
///
/// # Why this is mandatory
///
/// After Checkout completes, the *browser* posts `razorpay_order_id`,
/// `razorpay_payment_id`, and `razorpay_signature` to your server. The browser is
/// attacker-controlled — without this check, anyone can POST a fabricated "payment
/// succeeded" and receive goods for free. Call this before you fulfil an order.
///
/// # Examples
///
/// ```
/// use razorpay_sdk::signature::{compute_signature, verify_payment_signature};
///
/// let secret = "EnLs21M47BllR3X8PSFtjtbd";
/// let order_id = "order_IEIaMR65cU6MI1";
/// let payment_id = "pay_IEIazBq55mBSmS";
///
/// // In production this value arrives from Checkout as `razorpay_signature`.
/// let signature = compute_signature(&format!("{order_id}|{payment_id}"), secret);
///
/// verify_payment_signature(order_id, payment_id, &signature, secret)
///     .expect("genuine payment");
///
/// // A tampered payment id fails.
/// assert!(verify_payment_signature(order_id, "pay_forged", &signature, secret).is_err());
/// ```
#[must_use = "an ignored verification result is the same as no verification at all"]
pub fn verify_payment_signature(
    order_id: &str,
    payment_id: &str,
    signature: &str,
    key_secret: &str,
) -> Result<(), RazorpayError> {
    verify_signature(&format!("{order_id}|{payment_id}"), signature, key_secret)
}

/// Verify the `X-Razorpay-Signature` header on an incoming webhook.
///
/// The signature is HMAC-SHA256 of the **raw request body** keyed by the webhook
/// secret you configured in the Razorpay dashboard (which is *not* your API key
/// secret).
///
/// # `payload` must be the raw body
///
/// Pass the exact bytes Razorpay sent. Deserializing to a struct or
/// [`serde_json::Value`] and re-serializing changes key order and whitespace,
/// producing a different digest that will never match. This is the single most
/// common webhook integration bug — read the body as a string *first*, verify, and
/// only then parse.
///
/// # Examples
///
/// ```
/// use razorpay_sdk::signature::{compute_signature, verify_webhook_signature};
///
/// let secret = "webhook_secret";
/// let raw_body = r#"{"event":"payment.captured","payload":{}}"#;
/// let signature = compute_signature(raw_body, secret);
///
/// verify_webhook_signature(raw_body, &signature, secret).expect("genuine webhook");
///
/// // Re-serialized JSON has different bytes, so it fails — verify the raw body.
/// let reserialized = r#"{"payload":{},"event":"payment.captured"}"#;
/// assert!(verify_webhook_signature(reserialized, &signature, secret).is_err());
/// ```
#[must_use = "an ignored verification result is the same as no verification at all"]
pub fn verify_webhook_signature(
    payload: &str,
    signature: &str,
    webhook_secret: &str,
) -> Result<(), RazorpayError> {
    verify_signature(payload, signature, webhook_secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Published test vector: HMAC-SHA256("hello", "secret").
    const HELLO_SECRET: &str = "88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b";

    #[test]
    fn compute_matches_known_vector() {
        assert_eq!(compute_signature("hello", "secret"), HELLO_SECRET);
    }

    #[test]
    fn verify_accepts_correct_signature() {
        assert!(verify_signature("hello", HELLO_SECRET, "secret").is_ok());
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        assert!(matches!(
            verify_signature("hello", HELLO_SECRET, "wrong"),
            Err(RazorpayError::SignatureMismatch)
        ));
    }

    #[test]
    fn verify_rejects_tampered_message() {
        assert!(verify_signature("hello!", HELLO_SECRET, "secret").is_err());
    }

    #[test]
    fn verify_rejects_non_hex_signature() {
        assert!(verify_signature("hello", "zzzz", "secret").is_err());
    }

    #[test]
    fn verify_rejects_truncated_signature() {
        // A prefix of a valid signature must not pass.
        assert!(verify_signature("hello", &HELLO_SECRET[..32], "secret").is_err());
    }

    #[test]
    fn verify_is_case_insensitive_on_hex() {
        // Hex decoding normalizes case, so an uppercase signature still verifies.
        assert!(verify_signature("hello", &HELLO_SECRET.to_uppercase(), "secret").is_ok());
    }

    #[test]
    fn payment_signature_uses_pipe_separated_message() {
        let expected = compute_signature("order_1|pay_1", "sec");
        assert!(verify_payment_signature("order_1", "pay_1", &expected, "sec").is_ok());
        // Guard against a concatenation bug that would let ids be shuffled.
        assert!(verify_payment_signature("order_1|pay", "_1", &expected, "sec").is_err());
    }

    #[test]
    fn webhook_signature_is_over_exact_bytes() {
        let body = r#"{"event":"payment.captured"}"#;
        let sig = compute_signature(body, "whsec");
        assert!(verify_webhook_signature(body, &sig, "whsec").is_ok());
        // Whitespace differences change the digest — hence the raw-body requirement.
        assert!(verify_webhook_signature(r#"{"event": "payment.captured"}"#, &sig, "whsec").is_err());
    }
}
