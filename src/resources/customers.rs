//! Customers — saved payer identities, required for saved cards and subscriptions.

use serde::{Deserialize, Serialize};

use crate::client::RazorpayClient;
use crate::error::RazorpayError;
use crate::resources::common::Notes;
use crate::resources::tokens::Token;
use crate::util::pagination::{Collection, ListOptions};

/// A customer returned by the API.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Customer {
    /// Unique identifier, e.g. `cust_1Aa00000000001`.
    pub id: String,
    /// Always `"customer"`.
    #[serde(default)]
    pub entity: String,
    /// Customer's name.
    #[serde(default)]
    pub name: Option<String>,
    /// Email address.
    #[serde(default)]
    pub email: Option<String>,
    /// Contact number.
    #[serde(default)]
    pub contact: Option<String>,
    /// GSTIN, when supplied.
    #[serde(default)]
    pub gstin: Option<String>,
    /// Your metadata.
    #[serde(default)]
    pub notes: Notes,
    /// Creation time as a Unix timestamp in seconds.
    #[serde(default)]
    pub created_at: i64,
}

/// Parameters for creating or updating a customer.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CustomerParams {
    /// Customer's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Email address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Contact number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    /// GSTIN.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gstin: Option<String>,
    /// Metadata to attach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Notes>,
    /// When `false`, creating a customer that already exists returns the existing
    /// one instead of failing. Razorpay's default is to fail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_existing: Option<u8>,
}

impl CustomerParams {
    /// Empty parameters; set fields with the builder methods.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the customer's name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the email address.
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    /// Set the contact number.
    pub fn contact(mut self, contact: impl Into<String>) -> Self {
        self.contact = Some(contact.into());
        self
    }

    /// Set the GSTIN.
    pub fn gstin(mut self, gstin: impl Into<String>) -> Self {
        self.gstin = Some(gstin.into());
        self
    }

    /// Attach metadata.
    pub fn notes(mut self, notes: Notes) -> Self {
        self.notes = Some(notes);
        self
    }

    /// Return the existing customer instead of erroring when one already exists
    /// with the same contact and email.
    ///
    /// This makes [`CustomersClient::create`] idempotent, which is usually what
    /// you want when creating a customer lazily at checkout.
    pub fn reuse_existing(mut self) -> Self {
        self.fail_existing = Some(0);
        self
    }
}

/// Customer endpoints. Obtain one from [`RazorpayClient::customers`].
pub struct CustomersClient<'a> {
    pub(crate) client: &'a RazorpayClient,
}

impl<'a> CustomersClient<'a> {
    /// Create a customer — `POST /v1/customers`.
    ///
    /// By default Razorpay errors if a customer with the same contact and email
    /// exists; [`CustomerParams::reuse_existing`] turns that into a fetch.
    pub async fn create(&self, params: CustomerParams) -> Result<Customer, RazorpayError> {
        self.client.post("customers", Some(&params)).await
    }

    /// Fetch one customer by id — `GET /v1/customers/{id}`.
    pub async fn fetch(&self, id: &str) -> Result<Customer, RazorpayError> {
        self.client.get::<(), _>(&format!("customers/{id}"), None).await
    }

    /// List customers — `GET /v1/customers`.
    pub async fn all(&self, options: ListOptions) -> Result<Collection<Customer>, RazorpayError> {
        self.client.get("customers", Some(&options)).await
    }

    /// Update a customer — `PUT /v1/customers/{id}`.
    pub async fn edit(
        &self,
        id: &str,
        params: CustomerParams,
    ) -> Result<Customer, RazorpayError> {
        self.client
            .put(&format!("customers/{id}"), Some(&params))
            .await
    }

    /// List a customer's saved payment tokens — `GET /v1/customers/{id}/tokens`.
    pub async fn tokens(&self, id: &str) -> Result<Collection<Token>, RazorpayError> {
        self.client
            .get::<(), _>(&format!("customers/{id}/tokens"), None)
            .await
    }

    /// Fetch one saved token — `GET /v1/customers/{id}/tokens/{token_id}`.
    pub async fn fetch_token(&self, id: &str, token_id: &str) -> Result<Token, RazorpayError> {
        self.client
            .get::<(), _>(&format!("customers/{id}/tokens/{token_id}"), None)
            .await
    }

    /// Delete a saved token — `DELETE /v1/customers/{id}/tokens/{token_id}`.
    ///
    /// Call this when a customer removes a saved card.
    pub async fn delete_token(&self, id: &str, token_id: &str) -> Result<(), RazorpayError> {
        self.client
            .delete::<serde::de::IgnoredAny>(&format!("customers/{id}/tokens/{token_id}"))
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_params_serialize_to_empty_object() {
        assert_eq!(serde_json::to_string(&CustomerParams::new()).unwrap(), "{}");
    }

    #[test]
    fn reuse_existing_sets_fail_existing_zero() {
        let json = serde_json::to_string(&CustomerParams::new().reuse_existing()).unwrap();
        assert_eq!(json, r#"{"fail_existing":0}"#);
    }
}
