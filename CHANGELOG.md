# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] — 2026-08-08

Initial release. Covers the core payment lifecycle and recurring billing.

### Added

**Core**

- `RazorpayClient` with connection pooling, a 30s default timeout, and a
  `razorpay-sdk-rust/{version}` user agent.
- `AuthMethod` supporting API key pairs (HTTP Basic) and OAuth bearer tokens.
- `with_base_url` for pointing the client at a mock server, which is what makes the
  crate testable without network access.
- `RazorpayError` with distinct `Http`, `Api`, `UnexpectedStatus`, `Decode`,
  `SignatureMismatch`, and `InvalidUrl` variants. The `Api` variant carries a boxed
  `ApiError` preserving all six fields Razorpay documents plus the HTTP status;
  boxing keeps the enum at 32 bytes rather than 152.
- `RazorpayError::is_retryable`, `status`, `code`, and `api_error` accessors.
- `ListOptions` and `Collection<T>` shared by every list endpoint.

**Signatures**

- `verify_payment_signature` for the Checkout callback.
- `verify_webhook_signature` for incoming webhooks.
- `verify_signature` and `compute_signature` for flows this crate does not model.
- All comparisons are constant-time via `subtle`, and all return `Result` marked
  `#[must_use]` so an ignored check is a compiler warning.

**Resources**

- Orders: create, fetch, all, edit, fetch_payments.
- Payments: fetch, all, capture, edit, refund, refunds.
- Refunds: fetch, all, edit.
- Customers: create, fetch, all, edit, tokens, fetch_token, delete_token.
- Cards: fetch.
- Items: create, fetch, all, edit, delete.
- Plans: create, fetch, all.
- Subscriptions: create, fetch, all, cancel, pause, resume, edit, invoices.
- Invoices: create, fetch, all, issue, cancel, edit, delete, notify_by.
- Payment links: create, fetch, all, cancel, notify_by.

**Webhooks**

- `WebhookEvent` covering 16 payment, order, refund, subscription, and invoice
  events, with an `Unknown` fallback for everything else.
- `WebhookEvent::parse_verified`, which verifies the signature before parsing and
  so cannot be used in the wrong order.

### Design notes

- **Money is `i64`** in the currency's smallest unit throughout. Floating point is
  never used for amounts.
- **Every enum carries an `Unknown` fallback** and optional fields are defaulted, so
  new values Razorpay ships without a version bump degrade gracefully instead of
  breaking a working integration with a decode error.
- **`rustls` with `default-features = false`**, so there is no OpenSSL dependency
  and no build failures in Docker, Alpine, or cross-compilation.
- **No runtime is imposed**: `tokio` is a dev-dependency only.
- **No `chrono`**: timestamps are passed through as `i64` Unix seconds, avoiding a
  transitive dependency and the `chrono`/`time` split.

### Not included

- Settlements, Transfers/Route, Virtual Accounts, QR codes, and Disputes.
- Documents (needs multipart upload) and OAuth token exchange (different host and
  auth flow) — neither fits the current JSON request path.
- A `blocking` API. Wrap a call in `tokio::runtime::Runtime::block_on` instead.
- Automatic retries. Safe retry of writes requires idempotency keys; without them,
  retrying a timed-out `POST /orders` creates a duplicate order.

[Unreleased]: https://github.com/Priyanshu-Developer/razorpay-sdk/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Priyanshu-Developer/razorpay-sdk/releases/tag/v0.1.0
