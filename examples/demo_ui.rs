//! Interactive dashboard exercising every resource in this crate, prefilled with
//! real test-mode data so every card works with a single click.
//!
//! Run with:
//!
//! ```sh
//! RAZORPAY_KEY_ID=rzp_test_xxx RAZORPAY_KEY_SECRET=xxx \
//!     cargo run --example demo_ui --features demo-ui
//! ```
//!
//! Then open <http://127.0.0.1:4000>. On load, the page calls `/api/seed`, which
//! creates a customer, item, order, plan, subscription, payment link, and invoice
//! against Razorpay's real test-mode API and returns their ids. Every card whose
//! path or body references one of those ids (fetch order, cancel subscription,
//! issue invoice, ...) is prefilled with the seeded value, so clicking "Send"
//! immediately succeeds instead of erroring on a placeholder id.
//!
//! # Completing a real checkout
//!
//! The page also has a "Live checkout" panel that opens Razorpay's actual
//! Checkout popup (`checkout.js`) against the seeded order, using
//! [test card 4111 1111 1111 1111](https://razorpay.com/docs/payments/payments/test-card-upi-details/).
//! Completing it there is the *only* way to produce a real payment id, even in
//! test mode — Razorpay has no server-side "just create a paid payment" endpoint.
//! When Checkout succeeds it hands the browser `razorpay_payment_id` and a
//! signature; the page posts both to `/api/verify_payment`, which calls this
//! crate's [`verify_payment_signature`](razorpay_api::signature::verify_payment_signature)
//! server-side (**never trust the browser's word that a payment succeeded**) and,
//! once verified, prefills the fetch/capture/edit/refund/list-refunds cards under
//! Payments with the now-real payment id. Card, token, and standalone refund
//! fields still need to be pasted in manually — they come from that payment's
//! `card_id`, a saved token, or the refund response, none of which this page can
//! produce on its own.
//!
//! This binary requires `RAZORPAY_KEY_ID`/`RAZORPAY_KEY_SECRET` for a Razorpay test
//! account — it talks to the real API, not a mock.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use razorpay_api::RazorpayClient;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// One clickable operation on the dashboard.
struct Operation {
    /// Stable id used in the URL, e.g. `orders.create`.
    id: &'static str,
    /// Resource group heading, e.g. `Orders`.
    resource: &'static str,
    /// Short label, e.g. `Create order`.
    title: &'static str,
    method: HttpMethod,
    /// Path template; `{param}` segments become text inputs.
    path: &'static str,
    /// How to prefill each `{param}` in `path`, in the same order the params
    /// appear.
    param_seeds: &'static [ParamSeed],
    /// JSON body template for POST/PATCH/PUT. `{{field}}` placeholders are
    /// substituted from seeded ids before being shown in the editor.
    body_template: Option<&'static str>,
    /// A one-line note shown on the card when it can't be fully seeded
    /// (e.g. needs a payment id from a completed Checkout).
    note: Option<&'static str>,
}

/// How a `{param}` text input should be prefilled.
#[derive(Clone, Copy)]
enum ParamSeed {
    /// Filled in from `/api/seed`'s response once it resolves, e.g. `order_id`.
    FromSeed(&'static str),
    /// Filled in once the Live checkout panel verifies a real payment.
    FromCheckout,
    /// A fixed literal value known ahead of time, e.g. `"sms"` for a `medium` param.
    Static(&'static str),
    /// Left blank — no server-side way to produce a valid value (see the card's
    /// `note`).
    Blank,
}

#[derive(Clone, Copy)]
enum HttpMethod {
    Get,
    Post,
    Patch,
    Put,
    Delete,
}

impl HttpMethod {
    fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
        }
    }
}

/// Amount (paise) for the order created by [`seed`] and paid by the Checkout
/// panel — kept as one constant so the two can never disagree.
const SEED_ORDER_AMOUNT_PAISE: i64 = 50_000;

const PAYMENT_NOTE: &str =
    "Prefilled automatically once you complete the Live checkout panel above. Until then, paste in a payment id manually.";

/// The full operation catalog, grouped by resource in the same order as
/// [`razorpay_api::resources`]'s module docs.
fn catalog() -> Vec<Operation> {
    use HttpMethod::*;
    vec![
        // Orders
        Operation { id: "orders.create", resource: "Orders", title: "Create order", method: Post, path: "orders", param_seeds: &[], body_template: Some(r#"{"amount": 50000, "currency": "INR", "receipt": "demo-rcpt-1"}"#), note: None },
        Operation { id: "orders.fetch", resource: "Orders", title: "Fetch order", method: Get, path: "orders/{id}", param_seeds: &[ParamSeed::FromSeed("order_id")], body_template: None, note: None },
        Operation { id: "orders.all", resource: "Orders", title: "List orders", method: Get, path: "orders", param_seeds: &[], body_template: None, note: None },
        Operation { id: "orders.payments", resource: "Orders", title: "List payments for order", method: Get, path: "orders/{id}/payments", param_seeds: &[ParamSeed::FromSeed("order_id")], body_template: None, note: None },
        Operation { id: "orders.edit", resource: "Orders", title: "Edit order notes", method: Patch, path: "orders/{id}", param_seeds: &[ParamSeed::FromSeed("order_id")], body_template: Some(r#"{"notes": {"demo": "updated"}}"#), note: None },
        // Payments
        Operation { id: "payments.fetch", resource: "Payments", title: "Fetch payment", method: Get, path: "payments/{id}", param_seeds: &[ParamSeed::FromCheckout], body_template: None, note: Some(PAYMENT_NOTE) },
        Operation { id: "payments.all", resource: "Payments", title: "List payments", method: Get, path: "payments", param_seeds: &[], body_template: None, note: None },
        Operation { id: "payments.capture", resource: "Payments", title: "Capture payment", method: Post, path: "payments/{id}/capture", param_seeds: &[ParamSeed::FromCheckout], body_template: Some(r#"{"amount": 50000, "currency": "INR"}"#), note: Some(PAYMENT_NOTE) },
        Operation { id: "payments.edit", resource: "Payments", title: "Edit payment notes", method: Patch, path: "payments/{id}", param_seeds: &[ParamSeed::FromCheckout], body_template: Some(r#"{"notes": {"demo": "updated"}}"#), note: Some(PAYMENT_NOTE) },
        Operation { id: "payments.refund", resource: "Payments", title: "Refund payment", method: Post, path: "payments/{id}/refund", param_seeds: &[ParamSeed::FromCheckout], body_template: Some(r#"{"amount": 10000}"#), note: Some(PAYMENT_NOTE) },
        Operation { id: "payments.refunds", resource: "Payments", title: "List refunds for payment", method: Get, path: "payments/{id}/refunds", param_seeds: &[ParamSeed::FromCheckout], body_template: None, note: Some(PAYMENT_NOTE) },
        // Refunds
        Operation { id: "refunds.fetch", resource: "Refunds", title: "Fetch refund", method: Get, path: "refunds/{id}", param_seeds: &[ParamSeed::Blank], body_template: None, note: Some("No seeded id: refunds only exist after refunding a real payment. Paste one in to try this.") },
        Operation { id: "refunds.all", resource: "Refunds", title: "List refunds", method: Get, path: "refunds", param_seeds: &[], body_template: None, note: None },
        Operation { id: "refunds.edit", resource: "Refunds", title: "Edit refund notes", method: Patch, path: "refunds/{id}", param_seeds: &[ParamSeed::Blank], body_template: Some(r#"{"notes": {"demo": "updated"}}"#), note: Some("No seeded id: refunds only exist after refunding a real payment. Paste one in to try this.") },
        // Customers
        Operation { id: "customers.create", resource: "Customers", title: "Create customer", method: Post, path: "customers", param_seeds: &[], body_template: Some(r#"{"name": "Demo Customer", "email": "demo@example.com", "contact": "9000000000", "fail_existing": 0}"#), note: None },
        Operation { id: "customers.fetch", resource: "Customers", title: "Fetch customer", method: Get, path: "customers/{id}", param_seeds: &[ParamSeed::FromSeed("customer_id")], body_template: None, note: None },
        Operation { id: "customers.all", resource: "Customers", title: "List customers", method: Get, path: "customers", param_seeds: &[], body_template: None, note: None },
        Operation { id: "customers.edit", resource: "Customers", title: "Edit customer", method: Put, path: "customers/{id}", param_seeds: &[ParamSeed::FromSeed("customer_id")], body_template: Some(r#"{"name": "Demo Customer Updated"}"#), note: None },
        Operation { id: "customers.tokens", resource: "Customers", title: "List saved tokens", method: Get, path: "customers/{id}/tokens", param_seeds: &[ParamSeed::FromSeed("customer_id")], body_template: None, note: None },
        Operation { id: "customers.fetch_token", resource: "Customers", title: "Fetch saved token", method: Get, path: "customers/{id}/tokens/{token_id}", param_seeds: &[ParamSeed::FromSeed("customer_id"), ParamSeed::Blank], body_template: None, note: Some("Token id needs a saved card from a completed Checkout. Paste one in to try this.") },
        Operation { id: "customers.delete_token", resource: "Customers", title: "Delete saved token", method: Delete, path: "customers/{id}/tokens/{token_id}", param_seeds: &[ParamSeed::FromSeed("customer_id"), ParamSeed::Blank], body_template: None, note: Some("Token id needs a saved card from a completed Checkout. Paste one in to try this.") },
        // Cards
        Operation { id: "cards.fetch", resource: "Cards", title: "Fetch card", method: Get, path: "cards/{id}", param_seeds: &[ParamSeed::Blank], body_template: None, note: Some("No seeded id: card ids come from a real payment's card_id. Paste one in to try this.") },
        // Items
        Operation { id: "items.create", resource: "Items", title: "Create item", method: Post, path: "items", param_seeds: &[], body_template: Some(r#"{"name": "Demo Book", "amount": 20000, "currency": "INR"}"#), note: None },
        Operation { id: "items.fetch", resource: "Items", title: "Fetch item", method: Get, path: "items/{id}", param_seeds: &[ParamSeed::FromSeed("item_id")], body_template: None, note: None },
        Operation { id: "items.all", resource: "Items", title: "List items", method: Get, path: "items", param_seeds: &[], body_template: None, note: None },
        Operation { id: "items.edit", resource: "Items", title: "Edit item", method: Patch, path: "items/{id}", param_seeds: &[ParamSeed::FromSeed("item_id")], body_template: Some(r#"{"active": true}"#), note: None },
        Operation { id: "items.delete", resource: "Items", title: "Delete item", method: Delete, path: "items/{id}", param_seeds: &[ParamSeed::FromSeed("item_id")], body_template: None, note: Some("Deletes the seeded demo item — re-run seed afterward if you want it back.") },
        // Plans
        Operation { id: "plans.create", resource: "Plans", title: "Create plan", method: Post, path: "plans", param_seeds: &[], body_template: Some(r#"{"period": "monthly", "interval": 1, "item": {"name": "Demo Pro Plan", "amount": 49900, "currency": "INR"}}"#), note: None },
        Operation { id: "plans.fetch", resource: "Plans", title: "Fetch plan", method: Get, path: "plans/{id}", param_seeds: &[ParamSeed::FromSeed("plan_id")], body_template: None, note: None },
        Operation { id: "plans.all", resource: "Plans", title: "List plans", method: Get, path: "plans", param_seeds: &[], body_template: None, note: None },
        // Subscriptions
        Operation { id: "subscriptions.create", resource: "Subscriptions", title: "Create subscription", method: Post, path: "subscriptions", param_seeds: &[], body_template: Some(r#"{"plan_id": "{{plan_id}}", "customer_id": "{{customer_id}}", "total_count": 12}"#), note: None },
        Operation { id: "subscriptions.fetch", resource: "Subscriptions", title: "Fetch subscription", method: Get, path: "subscriptions/{id}", param_seeds: &[ParamSeed::FromSeed("subscription_id")], body_template: None, note: None },
        Operation { id: "subscriptions.all", resource: "Subscriptions", title: "List subscriptions", method: Get, path: "subscriptions", param_seeds: &[], body_template: None, note: None },
        Operation { id: "subscriptions.cancel", resource: "Subscriptions", title: "Cancel subscription", method: Post, path: "subscriptions/{id}/cancel", param_seeds: &[ParamSeed::FromSeed("subscription_id")], body_template: Some(r#"{"cancel_at_cycle_end": 0}"#), note: None },
        Operation { id: "subscriptions.pause", resource: "Subscriptions", title: "Pause subscription", method: Post, path: "subscriptions/{id}/pause", param_seeds: &[ParamSeed::FromSeed("subscription_id")], body_template: Some(r#"{"pause_at": "now"}"#), note: Some("Requires the subscription to be active (customer authorized it) first; a freshly created one will 400.") },
        Operation { id: "subscriptions.resume", resource: "Subscriptions", title: "Resume subscription", method: Post, path: "subscriptions/{id}/resume", param_seeds: &[ParamSeed::FromSeed("subscription_id")], body_template: Some(r#"{"resume_at": "now"}"#), note: Some("Requires the subscription to be paused first.") },
        Operation { id: "subscriptions.edit", resource: "Subscriptions", title: "Edit subscription notes", method: Patch, path: "subscriptions/{id}", param_seeds: &[ParamSeed::FromSeed("subscription_id")], body_template: Some(r#"{"notes": {"demo": "updated"}}"#), note: None },
        Operation { id: "subscriptions.invoices", resource: "Subscriptions", title: "List invoices for subscription", method: Get, path: "invoices?subscription_id={id}", param_seeds: &[ParamSeed::FromSeed("subscription_id")], body_template: None, note: None },
        // Invoices
        Operation { id: "invoices.create", resource: "Invoices", title: "Create invoice", method: Post, path: "invoices", param_seeds: &[], body_template: Some(r#"{"type": "invoice", "customer_id": "{{customer_id}}", "line_items": [{"name": "Demo Book", "amount": 10000}]}"#), note: None },
        Operation { id: "invoices.fetch", resource: "Invoices", title: "Fetch invoice", method: Get, path: "invoices/{id}", param_seeds: &[ParamSeed::FromSeed("invoice_id")], body_template: None, note: None },
        Operation { id: "invoices.all", resource: "Invoices", title: "List invoices", method: Get, path: "invoices", param_seeds: &[], body_template: None, note: None },
        Operation { id: "invoices.issue", resource: "Invoices", title: "Issue draft invoice", method: Post, path: "invoices/{id}/issue", param_seeds: &[ParamSeed::FromSeed("invoice_id")], body_template: None, note: None },
        Operation { id: "invoices.cancel", resource: "Invoices", title: "Cancel invoice", method: Post, path: "invoices/{id}/cancel", param_seeds: &[ParamSeed::FromSeed("invoice_id")], body_template: None, note: Some("Only unpaid invoices can be cancelled.") },
        Operation { id: "invoices.edit", resource: "Invoices", title: "Edit draft invoice", method: Patch, path: "invoices/{id}", param_seeds: &[ParamSeed::FromSeed("invoice_id")], body_template: Some(r#"{"type": "invoice", "line_items": [{"name": "Demo Book", "amount": 12000}]}"#), note: Some("Only draft invoices can be edited — fails after Issue is clicked.") },
        Operation { id: "invoices.delete", resource: "Invoices", title: "Delete draft invoice", method: Delete, path: "invoices/{id}", param_seeds: &[ParamSeed::FromSeed("invoice_id")], body_template: None, note: Some("Only draft invoices can be deleted — fails after Issue is clicked.") },
        Operation { id: "invoices.notify_by", resource: "Invoices", title: "Send notification", method: Post, path: "invoices/{id}/notify_by/{medium}", param_seeds: &[ParamSeed::FromSeed("invoice_id"), ParamSeed::Static("sms")], body_template: None, note: None },
        // Payment links
        Operation { id: "payment_links.create", resource: "Payment Links", title: "Create payment link", method: Post, path: "payment_links", param_seeds: &[], body_template: Some(r#"{"amount": 50000, "currency": "INR", "description": "Demo invoice #42"}"#), note: None },
        Operation { id: "payment_links.fetch", resource: "Payment Links", title: "Fetch payment link", method: Get, path: "payment_links/{id}", param_seeds: &[ParamSeed::FromSeed("payment_link_id")], body_template: None, note: None },
        Operation { id: "payment_links.all", resource: "Payment Links", title: "List payment links", method: Get, path: "payment_links", param_seeds: &[], body_template: None, note: None },
        Operation { id: "payment_links.cancel", resource: "Payment Links", title: "Cancel payment link", method: Post, path: "payment_links/{id}/cancel", param_seeds: &[ParamSeed::FromSeed("payment_link_id")], body_template: None, note: None },
        Operation { id: "payment_links.notify_by", resource: "Payment Links", title: "Resend notification", method: Post, path: "payment_links/{id}/notify_by/{medium}", param_seeds: &[ParamSeed::FromSeed("payment_link_id"), ParamSeed::Static("sms")], body_template: None, note: None },
    ]
}

#[derive(Clone)]
struct AppState {
    client: Arc<RazorpayClient>,
    /// Kept separately from `client` because Checkout.js needs it in the
    /// browser — it's the public half of the credential pair, safe to send.
    /// The matching secret never leaves this process.
    key_id: Arc<str>,
}

#[tokio::main]
async fn main() {
    let key_id = std::env::var("RAZORPAY_KEY_ID")
        .expect("set RAZORPAY_KEY_ID to a Razorpay *test* key id (rzp_test_...)");
    let key_secret =
        std::env::var("RAZORPAY_KEY_SECRET").expect("set RAZORPAY_KEY_SECRET to the matching secret");

    let client = RazorpayClient::new(key_id.clone(), key_secret);
    let state = AppState { client: Arc::new(client), key_id: key_id.into() };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/seed", post(seed))
        .route("/api/call/{op_id}", post(call_operation))
        .route("/api/verify_payment", post(verify_payment))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4000")
        .await
        .expect("failed to bind 127.0.0.1:4000");
    println!("Razorpay API demo dashboard: http://127.0.0.1:4000");
    axum::serve(listener, app).await.expect("server error");
}

async fn index(State(state): State<AppState>) -> Html<String> {
    Html(render_index(&state.key_id))
}

/// Ids created by [`seed`], threaded into every card's prefilled fields.
#[derive(Default, Serialize)]
struct SeedIds {
    customer_id: Option<String>,
    item_id: Option<String>,
    order_id: Option<String>,
    plan_id: Option<String>,
    subscription_id: Option<String>,
    invoice_id: Option<String>,
    payment_link_id: Option<String>,
    /// Non-fatal problems hit while seeding (e.g. one step failed); surfaced to
    /// the user instead of silently leaving fields blank.
    warnings: Vec<String>,
}

/// Runs a real setup sequence against the configured Razorpay test account so
/// every other card on the dashboard has a genuine id to act on.
async fn seed(State(state): State<AppState>) -> impl IntoResponse {
    let client = &state.client;
    let mut ids = SeedIds::default();

    match client
        .post_public::<_, Value>(
            "customers",
            Some(&json!({
                "name": "Demo Customer",
                "email": "demo@example.com",
                "contact": "9000000000",
                "fail_existing": 0,
            })),
        )
        .await
    {
        Ok(v) => ids.customer_id = v.get("id").and_then(Value::as_str).map(str::to_owned),
        Err(e) => ids.warnings.push(format!("create customer: {e}")),
    }

    match client
        .post_public::<_, Value>(
            "items",
            Some(&json!({"name": "Demo Book", "amount": 20_000, "currency": "INR"})),
        )
        .await
    {
        Ok(v) => ids.item_id = v.get("id").and_then(Value::as_str).map(str::to_owned),
        Err(e) => ids.warnings.push(format!("create item: {e}")),
    }

    match client
        .post_public::<_, Value>(
            "orders",
            Some(&json!({"amount": SEED_ORDER_AMOUNT_PAISE, "currency": "INR", "receipt": "demo-seed-order"})),
        )
        .await
    {
        Ok(v) => ids.order_id = v.get("id").and_then(Value::as_str).map(str::to_owned),
        Err(e) => ids.warnings.push(format!("create order: {e}")),
    }

    match client
        .post_public::<_, Value>(
            "plans",
            Some(&json!({
                "period": "monthly",
                "interval": 1,
                "item": {"name": "Demo Pro Plan", "amount": 49_900, "currency": "INR"},
            })),
        )
        .await
    {
        Ok(v) => ids.plan_id = v.get("id").and_then(Value::as_str).map(str::to_owned),
        Err(e) => ids.warnings.push(format!("create plan: {e}")),
    }

    if let (Some(plan_id), Some(customer_id)) = (ids.plan_id.clone(), ids.customer_id.clone()) {
        match client
            .post_public::<_, Value>(
                "subscriptions",
                Some(&json!({"plan_id": plan_id, "customer_id": customer_id, "total_count": 12})),
            )
            .await
        {
            Ok(v) => ids.subscription_id = v.get("id").and_then(Value::as_str).map(str::to_owned),
            Err(e) => ids.warnings.push(format!("create subscription: {e}")),
        }
    } else {
        ids.warnings.push("create subscription: skipped, missing plan or customer".into());
    }

    if let Some(customer_id) = ids.customer_id.clone() {
        match client
            .post_public::<_, Value>(
                "invoices",
                Some(&json!({
                    "type": "invoice",
                    "customer_id": customer_id,
                    "line_items": [{"name": "Demo Book", "amount": 10_000}],
                })),
            )
            .await
        {
            Ok(v) => ids.invoice_id = v.get("id").and_then(Value::as_str).map(str::to_owned),
            Err(e) => ids.warnings.push(format!("create invoice: {e}")),
        }
    } else {
        ids.warnings.push("create invoice: skipped, missing customer".into());
    }

    match client
        .post_public::<_, Value>(
            "payment_links",
            Some(&json!({"amount": 50_000, "currency": "INR", "description": "Demo invoice #42"})),
        )
        .await
    {
        Ok(v) => ids.payment_link_id = v.get("id").and_then(Value::as_str).map(str::to_owned),
        Err(e) => ids.warnings.push(format!("create payment link: {e}")),
    }

    Json(ids)
}

#[derive(Deserialize)]
struct CallRequest {
    /// Values for each `{param}` placeholder in the operation's path template.
    #[serde(default)]
    params: std::collections::HashMap<String, String>,
    /// Raw JSON body text from the editor; empty/absent means no body.
    #[serde(default)]
    body: Option<String>,
}

async fn call_operation(
    State(state): State<AppState>,
    Path(op_id): Path<String>,
    Json(req): Json<CallRequest>,
) -> impl IntoResponse {
    let Some(op) = catalog().into_iter().find(|o| o.id == op_id) else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({"error": "unknown operation"})),
        );
    };

    let mut path = op.path.to_string();
    for (key, value) in &req.params {
        path = path.replace(&format!("{{{key}}}"), value);
    }

    let body: Option<Value> = match req.body.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(raw) => match serde_json::from_str::<Value>(raw) {
            Ok(v) => Some(v),
            Err(e) => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(json!({"error": format!("invalid JSON body: {e}")})),
                );
            }
        },
    };

    let result: Result<Value, razorpay_api::RazorpayError> = match op.method {
        HttpMethod::Get => state.client.get_public::<(), Value>(&path, None).await,
        HttpMethod::Post => state.client.post_public(&path, body.as_ref()).await,
        HttpMethod::Patch => state.client.patch_public(&path, body.as_ref()).await,
        HttpMethod::Put => state.client.put_public(&path, body.as_ref()).await,
        HttpMethod::Delete => state.client.delete_public::<Value>(&path).await,
    };

    match result {
        Ok(value) => (
            axum::http::StatusCode::OK,
            Json(json!({"ok": true, "response": value})),
        ),
        Err(err) => (
            axum::http::StatusCode::OK,
            Json(json!({
                "ok": false,
                "error": err.to_string(),
                "status": err.status().map(|s| s.as_u16()),
            })),
        ),
    }
}

#[derive(Deserialize)]
struct VerifyPaymentRequest {
    razorpay_order_id: String,
    razorpay_payment_id: String,
    razorpay_signature: String,
}

/// Verifies the signature Checkout handed the browser, then fetches the payment
/// so the dashboard can show it and prefill the payment/refund/card cards.
///
/// This is the one step in the whole demo that is not optional in a real
/// integration: the three `razorpay_*` fields arrive from the browser, which is
/// attacker-controlled, so trusting them without this check would let anyone
/// forge a "payment succeeded" callback.
async fn verify_payment(
    State(state): State<AppState>,
    Json(req): Json<VerifyPaymentRequest>,
) -> impl IntoResponse {
    let razorpay_api::AuthMethod::BasicAuth { api_secret, .. } = state.client.auth_method() else {
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "error": "demo server is not using key/secret auth"})),
        );
    };

    if let Err(e) = razorpay_api::signature::verify_payment_signature(
        &req.razorpay_order_id,
        &req.razorpay_payment_id,
        &req.razorpay_signature,
        api_secret,
    ) {
        return (
            axum::http::StatusCode::OK,
            Json(json!({"ok": false, "error": format!("signature verification failed: {e}")})),
        );
    }

    match state
        .client
        .get_public::<(), Value>(&format!("payments/{}", req.razorpay_payment_id), None)
        .await
    {
        Ok(payment) => (
            axum::http::StatusCode::OK,
            Json(json!({
                "ok": true,
                "payment_id": req.razorpay_payment_id,
                "order_id": req.razorpay_order_id,
                "payment": payment,
            })),
        ),
        Err(e) => (
            axum::http::StatusCode::OK,
            Json(json!({
                "ok": false,
                "error": format!("signature verified, but fetching the payment failed: {e}"),
            })),
        ),
    }
}

fn extract_params(path: &str) -> Vec<&str> {
    let mut params = Vec::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        if let Some(end) = after.find('}') {
            params.push(&after[..end]);
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    params
}

fn render_index(key_id: &str) -> String {
    let mut resources: Vec<&'static str> = Vec::new();
    let ops = catalog();
    for op in &ops {
        if !resources.contains(&op.resource) {
            resources.push(op.resource);
        }
    }

    let mut cards = String::new();
    for resource in &resources {
        cards.push_str(&format!(
            "<section class=\"resource\"><h2>{resource}</h2><div class=\"grid\">"
        ));
        for op in ops.iter().filter(|o| &o.resource == resource) {
            let params: Vec<&str> = extract_params(op.path);
            let param_inputs: String = params
                .iter()
                .zip(op.param_seeds.iter().chain(std::iter::repeat(&ParamSeed::Blank)))
                .map(|(p, seed)| {
                    let (seed_attr, value_attr) = match seed {
                        ParamSeed::FromSeed(k) => (format!(" data-seed=\"{k}\""), String::new()),
                        ParamSeed::FromCheckout => (" data-payment-id".to_string(), String::new()),
                        ParamSeed::Static(v) => (String::new(), format!(" value=\"{v}\"")),
                        ParamSeed::Blank => (String::new(), String::new()),
                    };
                    format!(
                        "<label>{p}<input type=\"text\" data-param=\"{p}\"{seed_attr}{value_attr} placeholder=\"{p}\"></label>"
                    )
                })
                .collect();
            let body_field = match op.body_template {
                Some(template) => format!(
                    "<label>JSON body<textarea data-body rows=\"3\">{template}</textarea></label>"
                ),
                None => String::new(),
            };
            let note_html = match op.note {
                Some(n) => format!("<p class=\"note\">{n}</p>"),
                None => String::new(),
            };
            cards.push_str(&format!(
                r#"<article class="card" data-op="{id}">
                    <header><span class="method method-{method_lower}">{method}</span><h3>{title}</h3></header>
                    <code class="path">{path}</code>
                    {note_html}
                    {param_inputs}
                    {body_field}
                    <button type="button" class="send">Send</button>
                    <pre class="response" hidden></pre>
                </article>"#,
                id = op.id,
                method = op.method.as_str(),
                method_lower = op.method.as_str().to_lowercase(),
                title = op.title,
                path = op.path,
            ));
        }
        cards.push_str("</div></section>");
    }

    let key_id_js = serde_json::to_string(key_id).expect("string always serializes");
    let seed_order_amount = SEED_ORDER_AMOUNT_PAISE;

    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>razorpay-api demo</title>
<style>
  :root {{ color-scheme: light dark; }}
  body {{ font-family: system-ui, sans-serif; margin: 0; padding: 2rem; background: Canvas; color: CanvasText; }}
  h1 {{ margin-top: 0; }}
  .hint {{ opacity: 0.75; margin-bottom: 1rem; max-width: 70ch; }}
  #seed-status {{ display: flex; align-items: center; gap: 0.6rem; padding: 0.6rem 0.9rem; border-radius: 8px; background: color-mix(in srgb, CanvasText 6%, transparent); margin-bottom: 2rem; font-size: 0.85rem; }}
  #seed-status.ok {{ border: 1px solid #16a34a; }}
  #seed-status.warn {{ border: 1px solid #d97706; }}
  #seed-status.error {{ border: 1px solid #dc2626; }}
  #seed-status button {{ margin-left: auto; padding: 0.3rem 0.7rem; border-radius: 6px; border: 1px solid color-mix(in srgb, CanvasText 25%, transparent); background: transparent; color: inherit; cursor: pointer; }}
  .resource h2 {{ border-bottom: 1px solid color-mix(in srgb, CanvasText 20%, transparent); padding-bottom: 0.25rem; }}
  .grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 1rem; margin-bottom: 2rem; }}
  .card {{ border: 1px solid color-mix(in srgb, CanvasText 20%, transparent); border-radius: 8px; padding: 0.75rem; display: flex; flex-direction: column; gap: 0.5rem; }}
  .card header {{ display: flex; align-items: center; gap: 0.5rem; }}
  .card h3 {{ margin: 0; font-size: 0.95rem; }}
  .method {{ font-size: 0.7rem; font-weight: 700; padding: 0.1rem 0.4rem; border-radius: 4px; color: white; }}
  .method-get {{ background: #2563eb; }}
  .method-post {{ background: #16a34a; }}
  .method-patch {{ background: #d97706; }}
  .method-put {{ background: #7c3aed; }}
  .method-delete {{ background: #dc2626; }}
  .path {{ font-size: 0.75rem; opacity: 0.7; }}
  .note {{ font-size: 0.72rem; opacity: 0.8; background: color-mix(in srgb, #d97706 12%, transparent); border-left: 3px solid #d97706; padding: 0.3rem 0.5rem; margin: 0; border-radius: 3px; }}
  label {{ display: flex; flex-direction: column; font-size: 0.75rem; gap: 0.15rem; }}
  input, textarea {{ font-family: ui-monospace, monospace; font-size: 0.8rem; padding: 0.3rem; border-radius: 4px; border: 1px solid color-mix(in srgb, CanvasText 25%, transparent); background: Field; color: FieldText; }}
  input[data-seeded="true"] {{ border-color: #16a34a; }}
  button.send {{ align-self: flex-start; padding: 0.35rem 0.9rem; border-radius: 6px; border: none; background: #2563eb; color: white; cursor: pointer; }}
  button.send:hover {{ background: #1d4ed8; }}
  .response {{ margin: 0; white-space: pre-wrap; word-break: break-word; font-size: 0.75rem; background: color-mix(in srgb, CanvasText 6%, transparent); padding: 0.5rem; border-radius: 4px; max-height: 240px; overflow: auto; }}
  .response.error {{ border: 1px solid #dc2626; }}
  #checkout-panel {{ border: 1px solid color-mix(in srgb, #16a34a 50%, transparent); border-radius: 8px; padding: 1rem 1.25rem; margin-bottom: 2rem; background: color-mix(in srgb, #16a34a 6%, transparent); }}
  #checkout-panel h2 {{ margin-top: 0; }}
  #checkout-panel p {{ max-width: 70ch; font-size: 0.9rem; }}
  #checkout-panel code {{ font-size: 0.85rem; }}
  #pay-now {{ padding: 0.5rem 1.1rem; border-radius: 6px; border: none; background: #16a34a; color: white; cursor: pointer; font-size: 0.9rem; }}
  #pay-now:hover {{ background: #15803d; }}
  #pay-now:disabled {{ opacity: 0.6; cursor: default; }}
  #checkout-result {{ margin-top: 0.75rem; white-space: pre-wrap; word-break: break-word; font-size: 0.75rem; }}
  #checkout-result.ok {{ color: #16a34a; }}
  #checkout-result.error {{ color: #dc2626; }}
</style>
</head>
<body>
<h1>razorpay-api demo dashboard</h1>
<p class="hint">Every card below calls the real Razorpay <strong>test-mode</strong> API through this crate. On load this page seeds a customer, item, order, plan, subscription, invoice, and payment link, then prefills every card that needs one of those ids (green border) — just click Send.</p>
<div id="seed-status">Seeding demo data...</div>

<section id="checkout-panel">
  <h2>Live checkout</h2>
  <p>
    Opens Razorpay's real Checkout popup against the seeded order. Use test card
    <code>4111 1111 1111 1111</code>, any future expiry, any CVV, and any name —
    or any <a href="https://razorpay.com/docs/payments/payments/test-card-upi-details/" target="_blank" rel="noopener">other test instrument</a>.
    On success the signature is verified <strong>server-side</strong> before the
    resulting payment id is trusted, exactly as a real integration must.
  </p>
  <button id="pay-now" type="button" disabled>Waiting for seeded order…</button>
  <pre id="checkout-result" hidden></pre>
</section>

{cards}
<script src="https://checkout.razorpay.com/v1/checkout.js"></script>
<script>
const RAZORPAY_KEY_ID = {key_id_js};
const SEED_URL = '/api/seed';
let seededOrderId = null;
let seededOrderAmount = null;

async function seedAndFill() {{
  const statusEl = document.getElementById('seed-status');
  let ids;
  try {{
    const res = await fetch(SEED_URL, {{ method: 'POST' }});
    ids = await res.json();
  }} catch (e) {{
    statusEl.textContent = 'Failed to seed demo data: ' + e;
    statusEl.className = 'error';
    return;
  }}

  document.querySelectorAll('[data-seed]').forEach(input => {{
    const key = input.dataset.seed;
    if (ids[key]) {{
      input.value = ids[key];
      input.dataset.seeded = 'true';
    }}
  }});
  document.querySelectorAll('textarea[data-body]').forEach(ta => {{
    ta.value = ta.value
      .replaceAll('{{{{customer_id}}}}', ids.customer_id || '')
      .replaceAll('{{{{plan_id}}}}', ids.plan_id || '');
  }});

  const warnings = ids.warnings || [];
  if (warnings.length === 0) {{
    statusEl.textContent = 'Demo data seeded: customer, item, order, plan, subscription, invoice, and payment link created. Prefilled fields are outlined green.';
    statusEl.className = 'ok';
  }} else {{
    statusEl.textContent = 'Seeded with ' + warnings.length + ' warning(s): ' + warnings.join(' | ');
    statusEl.className = 'warn';
  }}
  const retryBtn = document.createElement('button');
  retryBtn.textContent = 'Re-seed';
  retryBtn.addEventListener('click', () => {{
    statusEl.textContent = 'Seeding demo data...';
    statusEl.className = '';
    statusEl.appendChild(retryBtn);
    seedAndFill();
  }});
  statusEl.appendChild(retryBtn);

  const payBtn = document.getElementById('pay-now');
  if (ids.order_id) {{
    seededOrderId = ids.order_id;
    seededOrderAmount = {seed_order_amount};
    payBtn.disabled = false;
    payBtn.textContent = 'Pay ₹' + (seededOrderAmount / 100).toFixed(2) + ' with Checkout (order ' + seededOrderId + ')';
  }} else {{
    payBtn.disabled = true;
    payBtn.textContent = 'No seeded order — fix seeding above first';
  }}
}}

document.querySelectorAll('.card').forEach(card => {{
  const opId = card.dataset.op;
  const button = card.querySelector('button.send');
  const responseEl = card.querySelector('.response');
  button.addEventListener('click', async () => {{
    const params = {{}};
    card.querySelectorAll('[data-param]').forEach(input => {{
      params[input.dataset.param] = input.value;
    }});
    const bodyEl = card.querySelector('[data-body]');
    const body = bodyEl ? bodyEl.value : undefined;

    button.disabled = true;
    responseEl.hidden = false;
    responseEl.classList.remove('error');
    responseEl.textContent = 'Sending...';
    try {{
      const res = await fetch(`/api/call/${{opId}}`, {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify({{ params, body }}),
      }});
      const data = await res.json();
      if (data.ok === false) responseEl.classList.add('error');
      responseEl.textContent = JSON.stringify(data, null, 2);
    }} catch (e) {{
      responseEl.classList.add('error');
      responseEl.textContent = String(e);
    }} finally {{
      button.disabled = false;
    }}
  }});
}});

document.getElementById('pay-now').addEventListener('click', () => {{
  const resultEl = document.getElementById('checkout-result');
  resultEl.hidden = false;
  resultEl.className = '';
  resultEl.textContent = 'Opening Checkout...';

  const rzp = new Razorpay({{
    key: RAZORPAY_KEY_ID,
    amount: seededOrderAmount,
    currency: 'INR',
    name: 'razorpay-api demo',
    description: 'Test payment for the demo dashboard',
    order_id: seededOrderId,
    handler: async function (response) {{
      resultEl.textContent = 'Checkout succeeded in the browser. Verifying signature server-side...';
      try {{
        const res = await fetch('/api/verify_payment', {{
          method: 'POST',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify({{
            razorpay_order_id: response.razorpay_order_id,
            razorpay_payment_id: response.razorpay_payment_id,
            razorpay_signature: response.razorpay_signature,
          }}),
        }});
        const data = await res.json();
        if (data.ok) {{
          resultEl.className = 'ok';
          resultEl.textContent = 'Verified. Payment ' + data.payment_id + ' is genuine.\\n' + JSON.stringify(data.payment, null, 2);
          document.querySelectorAll('[data-payment-id]').forEach(input => {{
            input.value = data.payment_id;
            input.dataset.seeded = 'true';
          }});
        }} else {{
          resultEl.className = 'error';
          resultEl.textContent = 'Verification failed: ' + data.error;
        }}
      }} catch (e) {{
        resultEl.className = 'error';
        resultEl.textContent = 'Verification request failed: ' + e;
      }}
    }},
    modal: {{
      ondismiss: function () {{
        resultEl.className = '';
        resultEl.textContent = 'Checkout closed without completing payment.';
      }},
    }},
    theme: {{ color: '#16a34a' }},
  }});
  rzp.on('payment.failed', function (response) {{
    resultEl.className = 'error';
    resultEl.textContent = 'Payment failed: ' + JSON.stringify(response.error, null, 2);
  }});
  rzp.open();
}});

seedAndFill();
</script>
</body>
</html>"##
    )
}
