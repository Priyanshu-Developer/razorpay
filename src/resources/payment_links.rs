//! Payment links — shareable URLs that collect a payment without a checkout page.

use serde::{Deserialize, Serialize};

use crate::client::RazorpayClient;
use crate::error::RazorpayError;
use crate::resources::common::Notes;
use crate::util::pagination::{Collection, ListOptions};

/// Lifecycle state of a [`PaymentLink`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentLinkStatus {
    /// Created and payable.
    Created,
    /// Partially paid, when partial payments are enabled.
    PartiallyPaid,
    /// Past its expiry without being paid.
    Expired,
    /// Cancelled before payment.
    Cancelled,
    /// Paid in full.
    Paid,
    /// A status this crate does not model yet.
    #[serde(other)]
    Unknown,
}

/// Who to notify about a payment link, and how.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifySettings {
    /// Send an SMS to the customer's contact.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sms: Option<bool>,
    /// Send an email to the customer's address.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub email: Option<bool>,
}

/// Customer details attached to a payment link.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkCustomer {
    /// Customer's name.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    /// Email address, required if you want email notifications.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub email: Option<String>,
    /// Contact number, required if you want SMS notifications.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub contact: Option<String>,
}

/// A payment link returned by the API.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PaymentLink {
    /// Unique identifier, e.g. `plink_ERgnLnOEQOKRLL`.
    pub id: String,
    /// Always `"payment_link"`.
    #[serde(default)]
    pub entity: String,
    /// Current lifecycle state.
    pub status: PaymentLinkStatus,
    /// Amount requested, in the smallest currency unit.
    #[serde(default)]
    pub amount: i64,
    /// Amount paid so far.
    #[serde(default)]
    pub amount_paid: i64,
    /// ISO 4217 currency code.
    #[serde(default)]
    pub currency: Option<String>,
    /// The URL to share with the customer.
    #[serde(default)]
    pub short_url: Option<String>,
    /// Free-text description shown on the payment page.
    #[serde(default)]
    pub description: Option<String>,
    /// Your own reference.
    #[serde(default)]
    pub reference_id: Option<String>,
    /// Whether the customer may pay in instalments.
    #[serde(default)]
    pub accept_partial: bool,
    /// Smallest partial payment accepted, in the smallest unit.
    #[serde(default)]
    pub first_min_partial_amount: Option<i64>,
    /// The customer this link was addressed to.
    #[serde(default)]
    pub customer: Option<LinkCustomer>,
    /// Notification settings.
    #[serde(default)]
    pub notify: Option<NotifySettings>,
    /// Whether Razorpay reminds the customer to pay.
    #[serde(default)]
    pub reminder_enable: bool,
    /// Where the customer is sent after paying.
    #[serde(default)]
    pub callback_url: Option<String>,
    /// HTTP method used for the callback.
    #[serde(default)]
    pub callback_method: Option<String>,
    /// When the link expires, Unix seconds.
    #[serde(default)]
    pub expire_by: Option<i64>,
    /// Your metadata.
    #[serde(default)]
    pub notes: Notes,
    /// Creation time as a Unix timestamp in seconds.
    #[serde(default)]
    pub created_at: i64,
}

/// Parameters for creating a [`PaymentLink`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreatePaymentLinkParams {
    /// Amount to collect, in the smallest currency unit.
    pub amount: i64,
    /// ISO 4217 currency code.
    pub currency: String,
    /// Description shown on the payment page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Who to bill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<LinkCustomer>,
    /// How to notify them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify: Option<NotifySettings>,
    /// Whether Razorpay should send payment reminders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminder_enable: Option<bool>,
    /// Allow paying in instalments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_partial: Option<bool>,
    /// Smallest first instalment, in the smallest unit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_min_partial_amount: Option<i64>,
    /// Your own reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
    /// Where to send the customer after payment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
    /// HTTP method for the callback; Razorpay requires `"get"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_method: Option<String>,
    /// Expiry, Unix seconds. Must be at least 15 minutes out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_by: Option<i64>,
    /// Metadata to attach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Notes>,
}

impl CreatePaymentLinkParams {
    /// A link collecting `amount` in the smallest unit of `currency`.
    pub fn new(amount: i64, currency: impl Into<String>) -> Self {
        Self {
            amount,
            currency: currency.into(),
            description: None,
            customer: None,
            notify: None,
            reminder_enable: None,
            accept_partial: None,
            first_min_partial_amount: None,
            reference_id: None,
            callback_url: None,
            callback_method: None,
            expire_by: None,
            notes: None,
        }
    }

    /// Describe what the customer is paying for.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Address the link to a customer.
    pub fn customer(mut self, customer: LinkCustomer) -> Self {
        self.customer = Some(customer);
        self
    }

    /// Have Razorpay notify the customer.
    ///
    /// Requires the matching contact detail on [`customer`](Self::customer).
    pub fn notify(mut self, sms: bool, email: bool) -> Self {
        self.notify = Some(NotifySettings {
            sms: Some(sms),
            email: Some(email),
        });
        self
    }

    /// Allow the customer to pay in instalments.
    pub fn accept_partial(mut self, first_min_amount: Option<i64>) -> Self {
        self.accept_partial = Some(true);
        self.first_min_partial_amount = first_min_amount;
        self
    }

    /// Send the customer here after paying.
    ///
    /// Razorpay only supports `GET` callbacks, so the method is set for you.
    pub fn callback_url(mut self, url: impl Into<String>) -> Self {
        self.callback_url = Some(url.into());
        self.callback_method = Some("get".to_string());
        self
    }

    /// Expire the link at this Unix timestamp.
    pub fn expire_by(mut self, expire_by: i64) -> Self {
        self.expire_by = Some(expire_by);
        self
    }

    /// Attach your own reference.
    pub fn reference_id(mut self, reference_id: impl Into<String>) -> Self {
        self.reference_id = Some(reference_id.into());
        self
    }

    /// Attach metadata.
    pub fn notes(mut self, notes: Notes) -> Self {
        self.notes = Some(notes);
        self
    }
}

/// Payment link endpoints. Obtain one from [`RazorpayClient::payment_links`].
pub struct PaymentLinksClient<'a> {
    pub(crate) client: &'a RazorpayClient,
}

impl<'a> PaymentLinksClient<'a> {
    /// Create a payment link — `POST /v1/payment_links`.
    ///
    /// Share the returned [`PaymentLink::short_url`] with the customer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use razorpay_api::{RazorpayClient, RazorpayError};
    /// # use razorpay_api::resources::{CreatePaymentLinkParams, LinkCustomer};
    /// # async fn demo(client: RazorpayClient) -> Result<(), RazorpayError> {
    /// let link = client
    ///     .payment_links()
    ///     .create(
    ///         CreatePaymentLinkParams::new(50_000, "INR")
    ///             .description("Invoice #42")
    ///             .customer(LinkCustomer {
    ///                 name: Some("Asha".into()),
    ///                 email: Some("asha@example.com".into()),
    ///                 contact: None,
    ///             })
    ///             .notify(false, true),
    ///     )
    ///     .await?;
    /// println!("send them {:?}", link.short_url);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(
        &self,
        params: CreatePaymentLinkParams,
    ) -> Result<PaymentLink, RazorpayError> {
        self.client.post("payment_links", Some(&params)).await
    }

    /// Fetch one payment link by id — `GET /v1/payment_links/{id}`.
    pub async fn fetch(&self, id: &str) -> Result<PaymentLink, RazorpayError> {
        self.client
            .get::<(), _>(&format!("payment_links/{id}"), None)
            .await
    }

    /// List payment links — `GET /v1/payment_links`.
    pub async fn all(
        &self,
        options: ListOptions,
    ) -> Result<Collection<PaymentLink>, RazorpayError> {
        self.client.get("payment_links", Some(&options)).await
    }

    /// Cancel a payment link — `POST /v1/payment_links/{id}/cancel`.
    ///
    /// Only links in [`Created`](PaymentLinkStatus::Created) can be cancelled.
    pub async fn cancel(&self, id: &str) -> Result<PaymentLink, RazorpayError> {
        self.client
            .post::<(), _>(&format!("payment_links/{id}/cancel"), None)
            .await
    }

    /// Resend the notification — `POST /v1/payment_links/{id}/notify_by/{medium}`.
    ///
    /// `medium` is `"sms"` or `"email"`.
    pub async fn notify_by(&self, id: &str, medium: &str) -> Result<(), RazorpayError> {
        self.client
            .post::<(), serde::de::IgnoredAny>(
                &format!("payment_links/{id}/notify_by/{medium}"),
                None,
            )
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_link_serializes_two_fields() {
        let json = serde_json::to_string(&CreatePaymentLinkParams::new(100, "INR")).unwrap();
        assert_eq!(json, r#"{"amount":100,"currency":"INR"}"#);
    }

    #[test]
    fn callback_url_also_sets_method() {
        let p = CreatePaymentLinkParams::new(100, "INR").callback_url("https://x.test/done");
        assert_eq!(p.callback_method.as_deref(), Some("get"));
    }

    #[test]
    fn accept_partial_sets_flag() {
        let p = CreatePaymentLinkParams::new(100, "INR").accept_partial(Some(50));
        assert_eq!(p.accept_partial, Some(true));
        assert_eq!(p.first_min_partial_amount, Some(50));
    }
}
