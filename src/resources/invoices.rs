//! Invoices — itemized requests for payment, sent to a customer.

use serde::{Deserialize, Serialize};

use crate::client::RazorpayClient;
use crate::error::RazorpayError;
use crate::resources::common::Notes;
use crate::util::pagination::{Collection, ListOptions};

/// Lifecycle state of an [`Invoice`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    /// Created but not yet sent to the customer.
    Draft,
    /// Sent and awaiting payment.
    Issued,
    /// Paid in full.
    Paid,
    /// Partially paid.
    PartiallyPaid,
    /// Past its due date without being paid.
    Expired,
    /// Cancelled before payment.
    Cancelled,
    /// Deleted while still a draft.
    Deleted,
    /// A status this crate does not model yet.
    #[serde(other)]
    Unknown,
}

/// A line on an [`Invoice`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineItem {
    /// Display name.
    #[serde(default)]
    pub name: Option<String>,
    /// Longer description.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    /// Unit price in the smallest currency unit.
    #[serde(default)]
    pub amount: i64,
    /// ISO 4217 currency code.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub currency: Option<String>,
    /// How many units.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub quantity: Option<u32>,
}

impl LineItem {
    /// A line charging `amount` for one unit of `name`.
    pub fn new(name: impl Into<String>, amount: i64) -> Self {
        Self {
            name: Some(name.into()),
            description: None,
            amount,
            currency: None,
            quantity: None,
        }
    }

    /// Charge for more than one unit.
    pub fn quantity(mut self, quantity: u32) -> Self {
        self.quantity = Some(quantity);
        self
    }

    /// Add a description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// An invoice returned by the API.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Invoice {
    /// Unique identifier, e.g. `inv_00000000000001`.
    pub id: String,
    /// Always `"invoice"`.
    #[serde(default)]
    pub entity: String,
    /// Current lifecycle state.
    pub status: InvoiceStatus,
    /// Total amount payable, in the smallest currency unit.
    #[serde(default)]
    pub amount: i64,
    /// Amount paid so far.
    #[serde(default)]
    pub amount_paid: i64,
    /// Amount still outstanding.
    #[serde(default)]
    pub amount_due: i64,
    /// ISO 4217 currency code.
    #[serde(default)]
    pub currency: Option<String>,
    /// Human-readable invoice number.
    #[serde(default)]
    pub invoice_number: Option<String>,
    /// The customer being billed.
    #[serde(default)]
    pub customer_id: Option<String>,
    /// The order backing this invoice.
    #[serde(default)]
    pub order_id: Option<String>,
    /// The payment that settled it.
    #[serde(default)]
    pub payment_id: Option<String>,
    /// The subscription that raised it, when applicable.
    #[serde(default)]
    pub subscription_id: Option<String>,
    /// Shareable short URL for the customer to pay.
    #[serde(default)]
    pub short_url: Option<String>,
    /// The billed lines.
    #[serde(default)]
    pub line_items: Vec<LineItem>,
    /// Free-text description.
    #[serde(default)]
    pub description: Option<String>,
    /// When payment is due, Unix seconds.
    #[serde(default)]
    pub expire_by: Option<i64>,
    /// When it was issued, Unix seconds.
    #[serde(default)]
    pub issued_at: Option<i64>,
    /// When it was paid, Unix seconds.
    #[serde(default)]
    pub paid_at: Option<i64>,
    /// When it was cancelled, Unix seconds.
    #[serde(default)]
    pub cancelled_at: Option<i64>,
    /// Your metadata.
    #[serde(default)]
    pub notes: Notes,
    /// Creation time as a Unix timestamp in seconds.
    #[serde(default)]
    pub created_at: i64,
}

/// Parameters for creating an [`Invoice`].
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CreateInvoiceParams {
    /// Always `"invoice"` for this endpoint.
    #[serde(rename = "type")]
    pub invoice_type: String,
    /// The customer to bill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    /// The lines to charge for.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub line_items: Vec<LineItem>,
    /// Free-text description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Due date, Unix seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_by: Option<i64>,
    /// Whether Razorpay emails/SMSes the customer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sms_notify: Option<u8>,
    /// Whether Razorpay emails the customer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_notify: Option<u8>,
    /// Your own reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<String>,
    /// Metadata to attach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Notes>,
}

impl CreateInvoiceParams {
    /// An invoice for `customer_id` charging the given lines.
    pub fn new(customer_id: impl Into<String>, line_items: Vec<LineItem>) -> Self {
        Self {
            invoice_type: "invoice".to_string(),
            customer_id: Some(customer_id.into()),
            line_items,
            ..Self::default()
        }
    }

    /// Set a due date as a Unix timestamp.
    pub fn expire_by(mut self, expire_by: i64) -> Self {
        self.expire_by = Some(expire_by);
        self
    }

    /// Add a description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Have Razorpay notify the customer by SMS and email.
    pub fn notify(mut self, sms: bool, email: bool) -> Self {
        self.sms_notify = Some(u8::from(sms));
        self.email_notify = Some(u8::from(email));
        self
    }

    /// Attach metadata.
    pub fn notes(mut self, notes: Notes) -> Self {
        self.notes = Some(notes);
        self
    }
}

/// Invoice endpoints. Obtain one from [`RazorpayClient::invoices`].
pub struct InvoicesClient<'a> {
    pub(crate) client: &'a RazorpayClient,
}

impl<'a> InvoicesClient<'a> {
    /// Create an invoice — `POST /v1/invoices`.
    ///
    /// A new invoice starts as [`Draft`](InvoiceStatus::Draft); call
    /// [`issue`](Self::issue) to send it.
    pub async fn create(&self, params: CreateInvoiceParams) -> Result<Invoice, RazorpayError> {
        self.client.post("invoices", Some(&params)).await
    }

    /// Fetch one invoice by id — `GET /v1/invoices/{id}`.
    pub async fn fetch(&self, id: &str) -> Result<Invoice, RazorpayError> {
        self.client.get::<(), _>(&format!("invoices/{id}"), None).await
    }

    /// List invoices — `GET /v1/invoices`.
    pub async fn all(&self, options: ListOptions) -> Result<Collection<Invoice>, RazorpayError> {
        self.client.get("invoices", Some(&options)).await
    }

    /// Issue a draft invoice — `POST /v1/invoices/{id}/issue`.
    ///
    /// This is what sends it to the customer and makes it payable.
    pub async fn issue(&self, id: &str) -> Result<Invoice, RazorpayError> {
        self.client
            .post::<(), _>(&format!("invoices/{id}/issue"), None)
            .await
    }

    /// Cancel an issued invoice — `POST /v1/invoices/{id}/cancel`.
    ///
    /// Only unpaid invoices can be cancelled.
    pub async fn cancel(&self, id: &str) -> Result<Invoice, RazorpayError> {
        self.client
            .post::<(), _>(&format!("invoices/{id}/cancel"), None)
            .await
    }

    /// Update a draft invoice — `PATCH /v1/invoices/{id}`.
    pub async fn edit(
        &self,
        id: &str,
        params: CreateInvoiceParams,
    ) -> Result<Invoice, RazorpayError> {
        self.client.patch(&format!("invoices/{id}"), Some(&params)).await
    }

    /// Delete a draft invoice — `DELETE /v1/invoices/{id}`.
    ///
    /// Only drafts can be deleted; issued invoices must be cancelled instead.
    pub async fn delete(&self, id: &str) -> Result<(), RazorpayError> {
        self.client
            .delete::<serde::de::IgnoredAny>(&format!("invoices/{id}"))
            .await
            .map(|_| ())
    }

    /// Send or resend the invoice notification — `POST /v1/invoices/{id}/notify_by/{medium}`.
    ///
    /// `medium` is `"sms"` or `"email"`.
    pub async fn notify_by(&self, id: &str, medium: &str) -> Result<(), RazorpayError> {
        self.client
            .post::<(), serde::de::IgnoredAny>(&format!("invoices/{id}/notify_by/{medium}"), None)
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_sets_type_and_line_items() {
        let params = CreateInvoiceParams::new("cust_1", vec![LineItem::new("Book", 10_000)]);
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains(r#""type":"invoice""#));
        assert!(json.contains(r#""name":"Book""#));
        assert!(json.contains(r#""amount":10000"#));
    }

    #[test]
    fn empty_line_items_are_omitted() {
        let params = CreateInvoiceParams::new("cust_1", vec![]);
        assert!(!serde_json::to_string(&params).unwrap().contains("line_items"));
    }

    #[test]
    fn partially_paid_status_decodes() {
        let inv: Invoice =
            serde_json::from_str(r#"{"id":"inv_1","status":"partially_paid"}"#).unwrap();
        assert_eq!(inv.status, InvoiceStatus::PartiallyPaid);
    }
}
