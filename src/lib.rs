//! An async, typed Rust client for the [Razorpay](https://razorpay.com) API.
//!
//! This crate is a typed wrapper over Razorpay's HTTP API: it turns
//! `POST /v1/orders` into [`client.orders().create(params)`](RazorpayClient::orders)
//! without losing type safety or error detail.
//!
//! # Quick start
//!
//! ```no_run
//! use razorpay_sdk::{RazorpayClient, RazorpayError};
//! use razorpay_sdk::resources::CreateOrderParams;
//!
//! # async fn demo() -> Result<(), RazorpayError> {
//! // Construct once at startup and share it — the client pools connections.
//! let client = RazorpayClient::new(
//!     std::env::var("RAZORPAY_KEY_ID").unwrap(),
//!     std::env::var("RAZORPAY_KEY_SECRET").unwrap(),
//! );
//!
//! // Amounts are in the smallest currency unit — 50_000 paise is ₹500.
//! let order = client
//!     .orders()
//!     .create(CreateOrderParams::new(50_000, "INR").receipt("rcpt#1"))
//!     .await?;
//!
//! println!("open Checkout with {}", order.id);
//! # Ok(())
//! # }
//! ```
//!
//! # The payment flow
//!
//! A complete integration has four steps, and **step 3 is not optional**:
//!
//! 1. **Create an order** server-side with [`orders().create()`](resources::orders::OrdersClient::create).
//! 2. **Open Checkout** in the browser with that order id. The customer pays.
//! 3. **Verify the signature** Checkout posts back, with
//!    [`verify_payment_signature`](signature::verify_payment_signature). The browser
//!    is attacker-controlled; without this, anyone can claim a payment succeeded.
//! 4. **Handle the webhook** for asynchronous updates, verifying it with
//!    [`WebhookEvent::parse_verified`](resources::webhooks::WebhookEvent::parse_verified).
//!
//! ```no_run
//! use razorpay_sdk::signature::verify_payment_signature;
//! # fn handle(order_id: &str, payment_id: &str, signature: &str, secret: &str)
//! # -> Result<(), razorpay_sdk::RazorpayError> {
//! // These three values arrive from the browser after Checkout completes.
//! verify_payment_signature(order_id, payment_id, signature, secret)?;
//! // Only now is it safe to mark the order as paid.
//! # Ok(())
//! # }
//! ```
//!
//! # Money is always an integer
//!
//! Every amount in this crate is an [`i64`] in the currency's **smallest unit** —
//! paise for INR, so ₹1 is `100`. Floating point is never used: `0.1 + 0.2 != 0.3`
//! in binary floating point, and on money that is a reconciliation bug users cannot
//! work around.
//!
//! # Forward compatibility
//!
//! Razorpay adds enum values and event types without a version bump. Every enum
//! here carries an `Unknown` fallback and every optional field is `#[serde(default)]`,
//! so a new status arriving in production degrades to `Unknown` instead of failing
//! to decode and breaking a working integration.
//!
//! # Errors
//!
//! Every call returns [`RazorpayError`]. The variants are deliberately distinct for
//! retry logic: [`Http`](RazorpayError::Http) means the request never landed and is
//! usually retryable, while [`Decode`](RazorpayError::Decode) means Razorpay
//! answered with something unexpected and never is. See [`RazorpayError::is_retryable`].
//!
//! # Available resources
//!
//! [Orders](resources::orders), [Payments](resources::payments),
//! [Refunds](resources::refunds), [Customers](resources::customers),
//! [Tokens](resources::tokens), [Cards](resources::cards), [Items](resources::items),
//! [Plans](resources::plans), [Subscriptions](resources::subscriptions),
//! [Invoices](resources::invoices), [Payment Links](resources::payment_links), and
//! [Webhooks](resources::webhooks).

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

mod client;
mod error;
pub mod resources;
mod util;

/// HMAC-SHA256 verification for payments and webhooks.
///
/// See [`verify_payment_signature`](crate::signature::verify_payment_signature)
/// for the Checkout callback and
/// [`verify_webhook_signature`](crate::signature::verify_webhook_signature) for
/// incoming webhooks.
pub mod signature {
    pub use crate::util::signature::{
        compute_signature, verify_payment_signature, verify_signature, verify_webhook_signature,
    };
}

pub use client::{AuthMethod, RazorpayClient};
pub use error::{ApiError, RazorpayError};
pub use util::pagination::{Collection, ListOptions};
