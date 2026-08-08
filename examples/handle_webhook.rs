//! Verify and dispatch an incoming webhook.
//!
//! Runs offline — no API keys or network needed:
//!
//! ```sh
//! cargo run --example handle_webhook
//! ```

use razorpay_api::resources::WebhookEvent;
use razorpay_api::signature::compute_signature;

/// What a real handler looks like: raw body in, typed event out.
///
/// In an HTTP framework, `raw_body` is the unparsed request body and `signature`
/// is the `X-Razorpay-Signature` header.
fn handle(raw_body: &str, signature: &str, webhook_secret: &str) {
    // Verifies first and only parses if the signature checks out.
    let event = match WebhookEvent::parse_verified(raw_body, signature, webhook_secret) {
        Ok(event) => event,
        Err(e) => {
            // Respond 400 and do nothing else — this was not sent by Razorpay.
            println!("  rejected: {e}");
            return;
        }
    };

    match event {
        WebhookEvent::PaymentCaptured(p) => {
            let payment = &p.payment.entity;
            println!("  captured {} for {} paise", payment.id, payment.amount);
        }
        WebhookEvent::PaymentFailed(p) => {
            let payment = &p.payment.entity;
            println!(
                "  failed {}: {}",
                payment.id,
                payment.error_description.as_deref().unwrap_or("no reason given")
            );
        }
        WebhookEvent::OrderPaid(o) => {
            println!("  order {} paid in full", o.order.entity.id);
        }
        WebhookEvent::SubscriptionCharged(s) => {
            let sub = &s.subscription.entity;
            println!("  subscription {} charged ({} paid)", sub.id, sub.paid_count);
        }
        // Events are enabled in the Razorpay dashboard independently of this
        // crate's version, so an unmodelled event must not crash the handler.
        other => println!("  unhandled event: {other:?}"),
    }
}

fn main() {
    let webhook_secret = "my_webhook_secret";

    // Bodies exactly as Razorpay would send them.
    let captured = r#"{"event":"payment.captured","payload":{"payment":{"entity":{"id":"pay_29QQoUBi66xm2f","entity":"payment","amount":50000,"currency":"INR","status":"captured","order_id":"order_EKwxwAgItmmXdp","method":"card","captured":true,"created_at":1400826750}}}}"#;

    let failed = r#"{"event":"payment.failed","payload":{"payment":{"entity":{"id":"pay_failed01","entity":"payment","amount":50000,"currency":"INR","status":"failed","error_description":"Card declined by issuer","created_at":1400826750}}}}"#;

    let unknown = r#"{"event":"virtual_account.credited","payload":{"virtual_account":{"entity":{"id":"va_1"}}}}"#;

    for (label, body) in [
        ("payment.captured", captured),
        ("payment.failed", failed),
        ("an event this crate does not model", unknown),
    ] {
        println!("{label}:");
        // Razorpay computes this over the raw body; we do the same to demonstrate.
        let signature = compute_signature(body, webhook_secret);
        handle(body, &signature, webhook_secret);
    }

    println!("\nforged body (signature will not match):");
    handle(captured, "0000000000000000", webhook_secret);

    println!("\nre-serialized body — same data, different bytes:");
    // A reminder of why the raw body matters: reordering keys changes the digest.
    let reordered = r#"{"payload":{"payment":{"entity":{"id":"pay_29QQoUBi66xm2f"}}},"event":"payment.captured"}"#;
    handle(reordered, &compute_signature(captured, webhook_secret), webhook_secret);
}
