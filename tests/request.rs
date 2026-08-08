use razorpay_api::{RazorpayClient, RazorpayError};
use serde::{Deserialize, Serialize};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Debug, Deserialize, PartialEq)]
struct Order {
    id: String,
    amount: i64,
}

#[derive(Serialize)]
struct CreateOrder {
    amount: i64,
    currency: String,
}

#[derive(Serialize)]
struct ListQuery {
    count: u32,
}

fn client(server: &MockServer) -> RazorpayClient {
    RazorpayClient::new("key_id".into(), "key_secret".into())
        .with_base_url(format!("{}/v1/", server.uri()).parse().unwrap())
}

#[tokio::test]
async fn get_sends_basic_auth_and_query_and_parses() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/orders"))
        // base64("key_id:key_secret")
        .and(header("authorization", "Basic a2V5X2lkOmtleV9zZWNyZXQ="))
        .and(query_param("count", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "order_1", "amount": 500
        })))
        .mount(&server)
        .await;

    let order: Order = client(&server)
        .get_public("orders", Some(&ListQuery { count: 2 }))
        .await
        .unwrap();
    assert_eq!(order, Order { id: "order_1".into(), amount: 500 });
}

#[tokio::test]
async fn post_sends_json_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .and(body_json(serde_json::json!({"amount": 500, "currency": "INR"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "order_2", "amount": 500
        })))
        .mount(&server)
        .await;

    let order: Order = client(&server)
        .post_public("orders", Some(&CreateOrder { amount: 500, currency: "INR".into() }))
        .await
        .unwrap();
    assert_eq!(order.id, "order_2");
}

#[tokio::test]
async fn error_envelope_on_400_becomes_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": {
                "code": "BAD_REQUEST_ERROR",
                "description": "amount must be atleast 100",
                "source": "business",
                "step": "payment_initiation",
                "reason": "input_validation_failed",
                "field": "amount"
            }
        })))
        .mount(&server)
        .await;

    let err = client(&server)
        .get_public::<(), Order>("orders/order_x", None)
        .await
        .unwrap_err();
    match err {
        RazorpayError::Api(ref e) => {
            assert_eq!(e.code, "BAD_REQUEST_ERROR");
            assert_eq!(e.description, "amount must be atleast 100");
            assert_eq!(e.field.as_deref(), Some("amount"));
            assert_eq!(e.error_source.as_deref(), Some("business"));
            assert_eq!(e.step.as_deref(), Some("payment_initiation"));
            assert_eq!(e.reason.as_deref(), Some("input_validation_failed"));
            assert_eq!(e.http_status.as_u16(), 400);
            // A 400 is the client's fault; retrying sends the same bad request.
            assert!(!err.is_retryable());
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn error_envelope_with_http_200_still_becomes_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": { "code": "GATEWAY_ERROR", "description": "declined" }
        })))
        .mount(&server)
        .await;

    let err = client(&server).get_public::<(), Order>("payments/p1", None).await.unwrap_err();
    assert!(matches!(err, RazorpayError::Api(ref e)
        if e.code == "GATEWAY_ERROR" && e.http_status.as_u16() == 200));
}

#[tokio::test]
async fn non_2xx_without_envelope_reports_status_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(502).set_body_string("<html>bad gateway</html>"))
        .mount(&server)
        .await;

    let err = client(&server).get_public::<(), Order>("orders", None).await.unwrap_err();
    match err {
        RazorpayError::UnexpectedStatus { http_status, body } => {
            assert_eq!(http_status.as_u16(), 502);
            assert!(body.contains("bad gateway"));
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn empty_body_deserializes_as_unit() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/payments/p1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let out: () = client(&server).delete_public("payments/p1").await.unwrap();
    assert_eq!(out, ());
}

#[tokio::test]
async fn leading_slash_path_does_not_drop_base_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "order_3", "amount": 1
        })))
        .mount(&server)
        .await;

    let order: Order = client(&server).get_public::<(), Order>("/orders", None).await.unwrap();
    assert_eq!(order.id, "order_3");
}

#[tokio::test]
async fn bearer_token_auth_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(header("authorization", "Bearer tok_123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "order_4", "amount": 1
        })))
        .mount(&server)
        .await;

    let c = RazorpayClient::new_with_bearer_token("tok_123".into())
        .with_base_url(format!("{}/v1/", server.uri()).parse().unwrap());
    let order: Order = c.get_public::<(), Order>("orders", None).await.unwrap();
    assert_eq!(order.id, "order_4");
}
