//! Orders — the entity you create before opening Checkout.
//!
//! An order ties a payment attempt to an amount you control server-side. Creating
//! one is the first step of nearly every Razorpay integration.

use serde::{Deserialize, Serialize};

use crate::client::RazorpayClient;
use crate::error::RazorpayError;
use crate::resources::common::{Notes, NotesUpdate};
use crate::resources::payments::Payment;
use crate::util::pagination::{Collection, ListOptions};

/// Lifecycle state of an [`Order`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    /// Created but not yet paid against.
    Created,
    /// At least one payment was attempted and failed.
    Attempted,
    /// Paid in full.
    Paid,
    /// A status this crate does not model yet.
    #[serde(other)]
    Unknown,
}

/// An order returned by the API.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Order {
    /// Unique identifier, e.g. `order_EKwxwAgItmmXdp`.
    pub id: String,
    /// Always `"order"`.
    #[serde(default)]
    pub entity: String,
    /// Amount in the currency's smallest unit (paise for INR).
    pub amount: i64,
    /// Amount already paid, in the smallest unit.
    #[serde(default)]
    pub amount_paid: i64,
    /// Amount still outstanding, in the smallest unit.
    #[serde(default)]
    pub amount_due: i64,
    /// ISO 4217 currency code, e.g. `INR`.
    pub currency: String,
    /// Your own reference for this order, if you set one.
    #[serde(default)]
    pub receipt: Option<String>,
    /// Current lifecycle state.
    pub status: OrderStatus,
    /// Number of payment attempts made against this order.
    #[serde(default)]
    pub attempts: u32,
    /// Your metadata.
    #[serde(default)]
    pub notes: Notes,
    /// Creation time as a Unix timestamp in seconds.
    pub created_at: i64,
}

/// Parameters for [`OrdersClient::create`].
///
/// Build with [`CreateOrderParams::new`], which takes the two required fields.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreateOrderParams {
    /// Amount in the smallest currency unit (paise for INR). Minimum is 100 (₹1).
    pub amount: i64,
    /// ISO 4217 currency code, e.g. `INR`.
    pub currency: String,
    /// Your own reference, typically an internal order id. Max 40 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<String>,
    /// Metadata to attach; echoed back on the [`Order`] and in webhooks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Notes>,
    /// Whether to capture automatically (`0`) or require explicit capture (`1`).
    ///
    /// Razorpay encodes this as an integer; see [`CreateOrderParams::manual_capture`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_capture: Option<u8>,
    /// Restrict this order to a single [`crate::resources::common::PaymentMethod`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

impl CreateOrderParams {
    /// A new order for `amount` in the smallest unit of `currency`.
    ///
    /// # Examples
    ///
    /// ```
    /// use razorpay_api::resources::CreateOrderParams;
    ///
    /// // ₹500.00 — amounts are in paise, never rupees.
    /// let params = CreateOrderParams::new(50_000, "INR");
    /// assert_eq!(params.amount, 50_000);
    /// ```
    pub fn new(amount: i64, currency: impl Into<String>) -> Self {
        Self {
            amount,
            currency: currency.into(),
            receipt: None,
            notes: None,
            payment_capture: None,
            method: None,
        }
    }

    /// Attach your own reference to this order.
    pub fn receipt(mut self, receipt: impl Into<String>) -> Self {
        self.receipt = Some(receipt.into());
        self
    }

    /// Attach metadata.
    pub fn notes(mut self, notes: Notes) -> Self {
        self.notes = Some(notes);
        self
    }

    /// Require an explicit [`capture`](crate::resources::payments::PaymentsClient::capture)
    /// instead of capturing automatically.
    ///
    /// Uncaptured payments are auto-refunded by Razorpay after a window, so only
    /// choose this if you really do capture later.
    pub fn manual_capture(mut self) -> Self {
        self.payment_capture = Some(0);
        self
    }

    /// Restrict the order to a single payment method, e.g. `"upi"`.
    pub fn method(mut self, method: impl Into<String>) -> Self {
        self.method = Some(method.into());
        self
    }
}

/// Order endpoints. Obtain one from [`RazorpayClient::orders`].
pub struct OrdersClient<'a> {
    pub(crate) client: &'a RazorpayClient,
}

impl<'a> OrdersClient<'a> {
    /// Create an order — `POST /v1/orders`.
    ///
    /// The returned [`Order::id`] is what you hand to Checkout on the front end.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use razorpay_api::{RazorpayClient, RazorpayError};
    /// # use razorpay_api::resources::CreateOrderParams;
    /// # async fn demo() -> Result<(), RazorpayError> {
    /// let client = RazorpayClient::new("key_id".into(), "key_secret".into());
    /// let order = client
    ///     .orders()
    ///     .create(CreateOrderParams::new(50_000, "INR").receipt("rcpt#1"))
    ///     .await?;
    /// println!("open checkout with {}", order.id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(&self, params: CreateOrderParams) -> Result<Order, RazorpayError> {
        self.client.post("orders", Some(&params)).await
    }

    /// Fetch one order by id — `GET /v1/orders/{id}`.
    pub async fn fetch(&self, id: &str) -> Result<Order, RazorpayError> {
        self.client.get::<(), _>(&format!("orders/{id}"), None).await
    }

    /// List orders — `GET /v1/orders`.
    ///
    /// Pass [`ListOptions::default()`] when you do not need filtering.
    pub async fn all(&self, options: ListOptions) -> Result<Collection<Order>, RazorpayError> {
        self.client.get("orders", Some(&options)).await
    }

    /// List the payments made against an order — `GET /v1/orders/{id}/payments`.
    ///
    /// Useful when reconciling: a `paid` order has exactly one captured payment,
    /// while an `attempted` order may have several failed ones.
    pub async fn fetch_payments(&self, id: &str) -> Result<Collection<Payment>, RazorpayError> {
        self.client
            .get::<(), _>(&format!("orders/{id}/payments"), None)
            .await
    }

    /// Replace an order's notes — `PATCH /v1/orders/{id}`.
    ///
    /// Notes are the only editable field, and the map replaces the existing one
    /// rather than merging into it.
    pub async fn edit(&self, id: &str, notes: Notes) -> Result<Order, RazorpayError> {
        self.client
            .patch(&format!("orders/{id}"), Some(&NotesUpdate::new(notes)))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_optional_params_are_omitted() {
        let json = serde_json::to_string(&CreateOrderParams::new(100, "INR")).unwrap();
        assert_eq!(json, r#"{"amount":100,"currency":"INR"}"#);
    }

    #[test]
    fn builder_sets_fields() {
        let p = CreateOrderParams::new(100, "INR").receipt("r1").manual_capture();
        assert_eq!(p.receipt.as_deref(), Some("r1"));
        assert_eq!(p.payment_capture, Some(0));
    }

    #[test]
    fn unknown_status_does_not_fail_decoding() {
        // Razorpay shipping a new status must not break existing integrations.
        let order: Order = serde_json::from_str(
            r#"{"id":"order_1","amount":1,"currency":"INR","status":"brand_new","created_at":1}"#,
        )
        .unwrap();
        assert_eq!(order.status, OrderStatus::Unknown);
    }
}
