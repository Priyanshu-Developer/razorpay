# razorpay-api

An async, typed Rust client for the [Razorpay](https://razorpay.com) payments API.

This crate is a typed wrapper over Razorpay's HTTP API: it turns `POST /v1/orders`
into `client.orders().create(params).await?` without losing type safety or error
detail.

```toml
[dependencies]
razorpay-api = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

The package is `razorpay-api`; the import path is `razorpay_api` (Cargo converts
hyphens to underscores).

## Quick start

```rust
use razorpay_api::{RazorpayClient, RazorpayError};
use razorpay_api::resources::CreateOrderParams;

#[tokio::main]
async fn main() -> Result<(), RazorpayError> {
    // Construct once at startup and share it — the client pools connections.
    let client = RazorpayClient::new(
        std::env::var("RAZORPAY_KEY_ID").unwrap(),
        std::env::var("RAZORPAY_KEY_SECRET").unwrap(),
    );

    // Amounts are in the smallest currency unit — 50_000 paise is ₹500.
    let order = client
        .orders()
        .create(CreateOrderParams::new(50_000, "INR").receipt("rcpt#1"))
        .await?;

    println!("open Checkout with {}", order.id);
    Ok(())
}
```

## The payment flow

A complete integration is four steps, and **step 3 is not optional**:

1. **Create an order** server-side with `client.orders().create()`.
2. **Open Checkout** in the browser with that order id. The customer pays.
3. **Verify the signature** Checkout posts back. The browser is attacker-controlled
   — without this, anyone can POST a fabricated "payment succeeded" and receive
   goods for free.
4. **Handle webhooks** for asynchronous updates.

```rust
use razorpay_api::signature::verify_payment_signature;

// These three arrive from the browser after Checkout completes.
verify_payment_signature(&order_id, &payment_id, &signature, &key_secret)?;
// Only now is it safe to mark the order as paid.
```

## Webhooks: pass the raw body

The signature is computed over the **exact bytes** Razorpay sent. Deserializing and
re-serializing changes key order and whitespace, producing a different digest that
never matches. This is the single most common webhook integration bug.

```rust
use razorpay_api::resources::WebhookEvent;

// `raw_body` must be the unparsed request body, read as a string first.
let event = WebhookEvent::parse_verified(&raw_body, &signature_header, &webhook_secret)?;

match event {
    WebhookEvent::PaymentCaptured(p) => println!("captured {}", p.payment.entity.id),
    WebhookEvent::OrderPaid(o) => println!("order paid {}", o.order.entity.id),
    // Razorpay adds events without a version bump; this arm keeps your handler working.
    other => println!("unhandled: {other:?}"),
}
```

`parse_verified` verifies before parsing and cannot be used in the wrong order.

## Money is always an integer

Every amount is an `i64` in the currency's **smallest unit** — paise for INR, so ₹1
is `100`. Floating point is never used: `0.1 + 0.2 != 0.3` in binary floating point,
and on money that is a reconciliation bug you cannot work around.

## Authorized is not captured

An `Authorized` payment is **not** money in your account — the funds are only held.
Razorpay auto-refunds uncaptured payments after a window, so a store that never
calls `capture` appears to work in testing and silently refunds every order days
later.

```rust
let payment = client.payments().fetch("pay_123").await?;
if !payment.is_captured() {
    client.payments().capture("pay_123", payment.amount, &payment.currency).await?;
}
```

You only need this if the order used `manual_capture()` or your account defaults to
manual capture; otherwise Razorpay captures for you.

## Resources

| Resource | Accessor | Operations |
|---|---|---|
| Orders | `client.orders()` | create, fetch, all, edit, fetch_payments |
| Payments | `client.payments()` | fetch, all, capture, edit, refund, refunds |
| Refunds | `client.refunds()` | fetch, all, edit |
| Customers | `client.customers()` | create, fetch, all, edit, tokens, fetch_token, delete_token |
| Cards | `client.cards()` | fetch |
| Items | `client.items()` | create, fetch, all, edit, delete |
| Plans | `client.plans()` | create, fetch, all |
| Subscriptions | `client.subscriptions()` | create, fetch, all, cancel, pause, resume, edit, invoices |
| Invoices | `client.invoices()` | create, fetch, all, issue, cancel, edit, delete, notify_by |
| Payment links | `client.payment_links()` | create, fetch, all, cancel, notify_by |

Every list endpoint takes `ListOptions` and returns `Collection<T>`:

```rust
use razorpay_api::ListOptions;

let page = client.orders().all(ListOptions::new().count(25).skip(50)).await?;
for order in &page {
    println!("{} — {:?}", order.id, order.status);
}
```

## Errors

Every call returns `Result<T, RazorpayError>`. The variants are deliberately
distinct so retry logic can be written correctly:

| Variant | Meaning | Retryable |
|---|---|---|
| `Http` | Request never landed (DNS, TLS, timeout) | Usually |
| `Api` | Razorpay returned a structured error | Only on 5xx / 429 |
| `UnexpectedStatus` | Non-2xx with no error envelope | Only on 5xx |
| `Decode` | Response was not the expected shape | Never |
| `SignatureMismatch` | Signature did not verify | Never — treat as forgery |
| `InvalidUrl` | Path could not be joined onto the base URL | Never |

`RazorpayError::is_retryable()` encodes that table. Note it reports whether the
*server* might answer differently, not whether the call is safe to repeat —
retrying a `POST /orders` that succeeded but timed out creates a duplicate order.

The `Api` variant carries a boxed `ApiError` preserving all six fields Razorpay
documents (`code`, `description`, `field`, `source`, `step`, `reason`) plus the HTTP
status. Match on `code`, not on `description`, which Razorpay may reword:

```rust
match client.orders().create(params).await {
    Ok(order) => println!("{}", order.id),
    Err(e) if e.code() == Some("BAD_REQUEST_ERROR") => {
        let details = e.api_error().unwrap();
        eprintln!("{} failed validation: {}", details.field.as_deref().unwrap_or("?"), details.description);
    }
    Err(e) if e.is_retryable() => eprintln!("transient: {e}"),
    Err(e) => return Err(e),
}
```

Boxing keeps `RazorpayError` at 32 bytes, so the common success path does not carry
the weight of the error type.

## Forward compatibility

Razorpay adds enum values and event types without a version bump. Every enum here
carries an `Unknown` fallback and optional fields are defaulted, so a new status
arriving in production degrades to `Unknown` rather than breaking a working
integration with a decode error.

## Examples

Runnable examples in [`examples/`](examples/):

```sh
export RAZORPAY_KEY_ID=rzp_test_...
export RAZORPAY_KEY_SECRET=...

cargo run --example create_order
cargo run --example verify_payment     # offline, no keys needed
cargo run --example handle_webhook     # offline, no keys needed
cargo run --example subscription
```

## Testing

```sh
cargo test --all-features
```

Tests run offline against a [`wiremock`](https://docs.rs/wiremock) server with
fixtures copied from Razorpay's docs — no API keys, no network, deterministic. The
`base_url` is overridable via `with_base_url`, which is what makes that possible.

## TLS

Uses `rustls` with `default-features = false`, so there is no OpenSSL dependency and
no build failures in Docker, Alpine, or cross-compilation.

## Not yet implemented

Settlements, Transfers/Route, Virtual Accounts, QR codes, Disputes, Documents, and
OAuth token exchange. Documents needs multipart upload and OAuth targets a different
host with a different auth flow, so neither fits the current request path.

There is no `blocking` API and no built-in retry. For blocking use, wrap a call in
`tokio::runtime::Runtime::block_on`. Retry is omitted deliberately: safe retry of
writes needs idempotency keys, and getting it wrong charges customers twice.

## License

MIT OR Apache-2.0.
