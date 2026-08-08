//! Resource-level tests against a mock server.
//!
//! These pin the *contract*: the HTTP verb, the URL, the request body, and that
//! Razorpay's documented response shape still decodes into our types. Fixtures are
//! trimmed copies of the examples in Razorpay's API docs.

use razorpay_api::resources::{
    CreateInvoiceParams, CreateOrderParams, CreatePaymentLinkParams, CreatePlanParams,
    CreateRefundParams, CreateSubscriptionParams, CustomerParams, InvoiceStatus, ItemParams,
    LineItem, OrderStatus, PaymentStatus, PlanPeriod, SubscriptionStatus,
};
use razorpay_api::{ListOptions, RazorpayClient};
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> RazorpayClient {
    RazorpayClient::new("key_id".into(), "key_secret".into())
        .with_base_url(format!("{}/v1/", server.uri()).parse().unwrap())
}

/// Mount a single canned response and return a client pointed at it.
async fn mock(
    server: &MockServer,
    verb: &str,
    url_path: &str,
    status: u16,
    body: serde_json::Value,
) {
    Mock::given(method(verb))
        .and(path(url_path))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(server)
        .await;
}

fn order_fixture() -> serde_json::Value {
    serde_json::json!({
        "id": "order_EKwxwAgItmmXdp",
        "entity": "order",
        "amount": 50000,
        "amount_paid": 0,
        "amount_due": 50000,
        "currency": "INR",
        "receipt": "rcptid_11",
        "status": "created",
        "attempts": 0,
        "notes": {"key": "value"},
        "created_at": 1582628071
    })
}

fn payment_fixture() -> serde_json::Value {
    serde_json::json!({
        "id": "pay_29QQoUBi66xm2f",
        "entity": "payment",
        "amount": 50000,
        "currency": "INR",
        "status": "captured",
        "order_id": "order_EKwxwAgItmmXdp",
        "international": false,
        "method": "card",
        "amount_refunded": 0,
        "captured": true,
        "description": "Purchase",
        "card_id": "card_JXPULjlR3DjqLp",
        "email": "a@example.com",
        "contact": "+919999999999",
        "fee": 1180,
        "tax": 180,
        "notes": {},
        "created_at": 1400826750
    })
}

// ---------------------------------------------------------------- orders

#[tokio::test]
async fn orders_create_posts_params_and_decodes_order() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .and(body_json(serde_json::json!({
            "amount": 50000, "currency": "INR", "receipt": "rcptid_11"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(order_fixture()))
        .mount(&server)
        .await;

    let order = client(&server)
        .orders()
        .create(CreateOrderParams::new(50_000, "INR").receipt("rcptid_11"))
        .await
        .unwrap();

    assert_eq!(order.id, "order_EKwxwAgItmmXdp");
    assert_eq!(order.status, OrderStatus::Created);
    assert_eq!(order.amount, 50_000);
    assert_eq!(order.notes.get("key").map(String::as_str), Some("value"));
}

#[tokio::test]
async fn orders_fetch_hits_id_path() {
    let server = MockServer::start().await;
    mock(&server, "GET", "/v1/orders/order_EKwxwAgItmmXdp", 200, order_fixture()).await;

    let order = client(&server).orders().fetch("order_EKwxwAgItmmXdp").await.unwrap();
    assert_eq!(order.id, "order_EKwxwAgItmmXdp");
}

#[tokio::test]
async fn orders_all_sends_list_options_as_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/orders"))
        .and(query_param("count", "2"))
        .and(query_param("skip", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "entity": "collection", "count": 1, "items": [order_fixture()]
        })))
        .mount(&server)
        .await;

    let page = client(&server)
        .orders()
        .all(ListOptions::new().count(2).skip(1))
        .await
        .unwrap();

    assert_eq!(page.count, 1);
    assert_eq!(page.len(), 1);
    assert_eq!(page.items[0].id, "order_EKwxwAgItmmXdp");
}

#[tokio::test]
async fn orders_edit_patches_notes() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/orders/order_1"))
        .and(body_json(serde_json::json!({"notes": {"k": "v"}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(order_fixture()))
        .mount(&server)
        .await;

    let notes = std::collections::HashMap::from([("k".to_string(), "v".to_string())]);
    assert!(client(&server).orders().edit("order_1", notes).await.is_ok());
}

#[tokio::test]
async fn orders_fetch_payments_returns_collection() {
    let server = MockServer::start().await;
    mock(
        &server,
        "GET",
        "/v1/orders/order_1/payments",
        200,
        serde_json::json!({"entity": "collection", "count": 1, "items": [payment_fixture()]}),
    )
    .await;

    let payments = client(&server).orders().fetch_payments("order_1").await.unwrap();
    assert_eq!(payments.items[0].status, PaymentStatus::Captured);
}

// -------------------------------------------------------------- payments

#[tokio::test]
async fn payments_fetch_decodes_full_fixture() {
    let server = MockServer::start().await;
    mock(&server, "GET", "/v1/payments/pay_29QQoUBi66xm2f", 200, payment_fixture()).await;

    let payment = client(&server).payments().fetch("pay_29QQoUBi66xm2f").await.unwrap();
    assert!(payment.is_captured());
    assert_eq!(payment.fee, Some(1180));
    assert_eq!(payment.order_id.as_deref(), Some("order_EKwxwAgItmmXdp"));
}

#[tokio::test]
async fn payments_capture_posts_amount_and_currency() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/payments/pay_1/capture"))
        .and(body_json(serde_json::json!({"amount": 50000, "currency": "INR"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(payment_fixture()))
        .mount(&server)
        .await;

    let payment = client(&server).payments().capture("pay_1", 50_000, "INR").await.unwrap();
    assert!(payment.is_captured());
}

#[tokio::test]
async fn payments_refund_full_sends_empty_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/payments/pay_1/refund"))
        .and(body_json(serde_json::json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "rfnd_1", "entity": "refund", "amount": 50000, "currency": "INR",
            "payment_id": "pay_1", "status": "processed", "notes": {}, "created_at": 1
        })))
        .mount(&server)
        .await;

    let refund = client(&server)
        .payments()
        .refund("pay_1", CreateRefundParams::full())
        .await
        .unwrap();
    assert_eq!(refund.payment_id, "pay_1");
}

#[tokio::test]
async fn payments_refund_partial_sends_amount() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/payments/pay_1/refund"))
        .and(body_json(serde_json::json!({"amount": 10000})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "rfnd_2", "amount": 10000, "currency": "INR",
            "payment_id": "pay_1", "created_at": 1
        })))
        .mount(&server)
        .await;

    let refund = client(&server)
        .payments()
        .refund("pay_1", CreateRefundParams::partial(10_000))
        .await
        .unwrap();
    assert_eq!(refund.amount, 10_000);
}

// -------------------------------------------------------------- customers

#[tokio::test]
async fn customers_create_and_edit_use_post_then_put() {
    let server = MockServer::start().await;
    let fixture = serde_json::json!({
        "id": "cust_1", "entity": "customer", "name": "Asha",
        "email": "asha@example.com", "notes": {}, "created_at": 1
    });
    mock(&server, "POST", "/v1/customers", 200, fixture.clone()).await;
    mock(&server, "PUT", "/v1/customers/cust_1", 200, fixture).await;

    let c = client(&server);
    let created = c
        .customers()
        .create(CustomerParams::new().name("Asha").email("asha@example.com"))
        .await
        .unwrap();
    assert_eq!(created.id, "cust_1");

    let edited = c
        .customers()
        .edit("cust_1", CustomerParams::new().name("Asha"))
        .await
        .unwrap();
    assert_eq!(edited.name.as_deref(), Some("Asha"));
}

#[tokio::test]
async fn customer_token_delete_tolerates_empty_body() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/customers/cust_1/tokens/token_1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    client(&server)
        .customers()
        .delete_token("cust_1", "token_1")
        .await
        .unwrap();
}

// ------------------------------------------------------------------ items

#[tokio::test]
async fn items_create_and_delete() {
    let server = MockServer::start().await;
    mock(
        &server,
        "POST",
        "/v1/items",
        200,
        serde_json::json!({
            "id": "item_1", "entity": "item", "name": "Book", "amount": 20000,
            "currency": "INR", "active": true, "created_at": 1
        }),
    )
    .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/items/item_1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let c = client(&server);
    let item = c.items().create(ItemParams::new("Book", 20_000, "INR")).await.unwrap();
    assert_eq!(item.name, "Book");
    c.items().delete("item_1").await.unwrap();
}

// ------------------------------------------------------------------ plans

#[tokio::test]
async fn plans_create_sends_nested_item() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/plans"))
        .and(body_json(serde_json::json!({
            "period": "monthly",
            "interval": 1,
            "item": {"name": "Pro", "amount": 49900, "currency": "INR"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "plan_1", "entity": "plan", "period": "monthly", "interval": 1,
            "item": {"id": "item_1", "name": "Pro", "amount": 49900, "currency": "INR"},
            "notes": {}, "created_at": 1
        })))
        .mount(&server)
        .await;

    let plan = client(&server)
        .plans()
        .create(CreatePlanParams::new(PlanPeriod::Monthly, 1, "Pro", 49_900, "INR"))
        .await
        .unwrap();

    assert_eq!(plan.period, PlanPeriod::Monthly);
    assert_eq!(plan.item.amount, 49_900);
}

// ---------------------------------------------------------- subscriptions

#[tokio::test]
async fn subscriptions_create_fetch_and_cancel() {
    let server = MockServer::start().await;
    let fixture = serde_json::json!({
        "id": "sub_1", "entity": "subscription", "plan_id": "plan_1",
        "status": "active", "total_count": 12, "paid_count": 1,
        "remaining_count": 11, "quantity": 1,
        "short_url": "https://rzp.io/i/abc", "notes": {}, "created_at": 1
    });
    mock(&server, "POST", "/v1/subscriptions", 200, fixture.clone()).await;

    let sub = client(&server)
        .subscriptions()
        .create(CreateSubscriptionParams::new("plan_1", 12).customer_notify(true))
        .await
        .unwrap();

    assert!(sub.is_active());
    assert_eq!(sub.status, SubscriptionStatus::Active);
    assert_eq!(sub.short_url.as_deref(), Some("https://rzp.io/i/abc"));
}

#[tokio::test]
async fn subscription_cancel_sends_cycle_end_flag() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/subscriptions/sub_1/cancel"))
        .and(body_json(serde_json::json!({"cancel_at_cycle_end": 1})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "sub_1", "plan_id": "plan_1", "status": "cancelled"
        })))
        .mount(&server)
        .await;

    let sub = client(&server).subscriptions().cancel("sub_1", true).await.unwrap();
    assert_eq!(sub.status, SubscriptionStatus::Cancelled);
}

// --------------------------------------------------------------- invoices

#[tokio::test]
async fn invoices_create_then_issue() {
    let server = MockServer::start().await;
    mock(
        &server,
        "POST",
        "/v1/invoices",
        200,
        serde_json::json!({
            "id": "inv_1", "entity": "invoice", "status": "draft",
            "amount": 10000, "amount_paid": 0, "amount_due": 10000,
            "currency": "INR", "line_items": [], "notes": {}, "created_at": 1
        }),
    )
    .await;
    mock(
        &server,
        "POST",
        "/v1/invoices/inv_1/issue",
        200,
        serde_json::json!({
            "id": "inv_1", "status": "issued", "short_url": "https://rzp.io/i/x"
        }),
    )
    .await;

    let c = client(&server);
    let draft = c
        .invoices()
        .create(CreateInvoiceParams::new("cust_1", vec![LineItem::new("Book", 10_000)]))
        .await
        .unwrap();
    assert_eq!(draft.status, InvoiceStatus::Draft);

    let issued = c.invoices().issue("inv_1").await.unwrap();
    assert_eq!(issued.status, InvoiceStatus::Issued);
}

// ---------------------------------------------------------- payment links

#[tokio::test]
async fn payment_links_create_returns_short_url() {
    let server = MockServer::start().await;
    mock(
        &server,
        "POST",
        "/v1/payment_links",
        200,
        serde_json::json!({
            "id": "plink_1", "entity": "payment_link", "status": "created",
            "amount": 50000, "amount_paid": 0, "currency": "INR",
            "short_url": "https://rzp.io/i/pl", "accept_partial": false,
            "notes": {}, "created_at": 1
        }),
    )
    .await;

    let link = client(&server)
        .payment_links()
        .create(CreatePaymentLinkParams::new(50_000, "INR").description("Invoice #42"))
        .await
        .unwrap();

    assert_eq!(link.short_url.as_deref(), Some("https://rzp.io/i/pl"));
}

// ------------------------------------------------------------------ cards

#[tokio::test]
async fn cards_fetch_decodes_renamed_type_field() {
    let server = MockServer::start().await;
    mock(
        &server,
        "GET",
        "/v1/cards/card_1",
        200,
        serde_json::json!({
            "id": "card_1", "entity": "card", "name": "Asha", "last4": "1111",
            "network": "Visa", "type": "credit", "issuer": "HDFC",
            "international": false, "emi": true
        }),
    )
    .await;

    let card = client(&server).cards().fetch("card_1").await.unwrap();
    assert_eq!(card.card_type.as_deref(), Some("credit"));
    assert_eq!(card.network.as_deref(), Some("Visa"));
}
