//! Items — reusable named prices you attach to invoices and plans.

use serde::{Deserialize, Serialize};

use crate::client::RazorpayClient;
use crate::error::RazorpayError;
use crate::util::pagination::{Collection, ListOptions};

/// A reusable priced line item.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Item {
    /// Unique identifier, e.g. `item_7Oxp4hmm6T4SCn`.
    pub id: String,
    /// Always `"item"`.
    #[serde(default)]
    pub entity: String,
    /// Display name.
    pub name: String,
    /// Longer description.
    #[serde(default)]
    pub description: Option<String>,
    /// Unit price in the smallest currency unit.
    pub amount: i64,
    /// ISO 4217 currency code.
    pub currency: String,
    /// Whether the item can still be used.
    #[serde(default)]
    pub active: bool,
    /// Unit of measure, e.g. `kg`.
    #[serde(default)]
    pub unit: Option<String>,
    /// HSN code for tax reporting.
    #[serde(default)]
    pub hsn_code: Option<String>,
    /// SAC code for tax reporting.
    #[serde(default)]
    pub sac_code: Option<String>,
    /// Tax rate applied, in basis points.
    #[serde(default)]
    pub tax_rate: Option<f64>,
    /// Creation time as a Unix timestamp in seconds.
    #[serde(default)]
    pub created_at: i64,
}

/// Parameters for creating or updating an [`Item`].
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ItemParams {
    /// Display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Longer description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Unit price in the smallest currency unit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<i64>,
    /// ISO 4217 currency code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Whether the item is usable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

impl ItemParams {
    /// A new item with the three fields Razorpay requires at creation.
    pub fn new(name: impl Into<String>, amount: i64, currency: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            amount: Some(amount),
            currency: Some(currency.into()),
            ..Self::default()
        }
    }

    /// Empty parameters, for partial updates via [`ItemsClient::edit`].
    pub fn update() -> Self {
        Self::default()
    }

    /// Set the description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the unit price.
    pub fn amount(mut self, amount: i64) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Activate or deactivate the item.
    pub fn active(mut self, active: bool) -> Self {
        self.active = Some(active);
        self
    }
}

/// Item endpoints. Obtain one from [`RazorpayClient::items`].
pub struct ItemsClient<'a> {
    pub(crate) client: &'a RazorpayClient,
}

impl<'a> ItemsClient<'a> {
    /// Create an item — `POST /v1/items`.
    pub async fn create(&self, params: ItemParams) -> Result<Item, RazorpayError> {
        self.client.post("items", Some(&params)).await
    }

    /// Fetch one item by id — `GET /v1/items/{id}`.
    pub async fn fetch(&self, id: &str) -> Result<Item, RazorpayError> {
        self.client.get::<(), _>(&format!("items/{id}"), None).await
    }

    /// List items — `GET /v1/items`.
    pub async fn all(&self, options: ListOptions) -> Result<Collection<Item>, RazorpayError> {
        self.client.get("items", Some(&options)).await
    }

    /// Update an item — `PATCH /v1/items/{id}`.
    pub async fn edit(&self, id: &str, params: ItemParams) -> Result<Item, RazorpayError> {
        self.client.patch(&format!("items/{id}"), Some(&params)).await
    }

    /// Delete an item — `DELETE /v1/items/{id}`.
    pub async fn delete(&self, id: &str) -> Result<(), RazorpayError> {
        self.client
            .delete::<serde::de::IgnoredAny>(&format!("items/{id}"))
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_sends_required_fields_only() {
        let json = serde_json::to_string(&ItemParams::new("Book", 20_000, "INR")).unwrap();
        assert_eq!(json, r#"{"name":"Book","amount":20000,"currency":"INR"}"#);
    }

    #[test]
    fn update_sends_only_changed_fields() {
        let json = serde_json::to_string(&ItemParams::update().active(false)).unwrap();
        assert_eq!(json, r#"{"active":false}"#);
    }
}
