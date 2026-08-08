//! Plans — the billing cadence and price a subscription runs on.

use serde::{Deserialize, Serialize};

use crate::client::RazorpayClient;
use crate::error::RazorpayError;
use crate::resources::common::Notes;
use crate::resources::items::Item;
use crate::util::pagination::{Collection, ListOptions};

/// How often a plan bills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanPeriod {
    /// Bills every `interval` days.
    Daily,
    /// Bills every `interval` weeks.
    Weekly,
    /// Bills every `interval` months.
    Monthly,
    /// Bills every `interval` years.
    Yearly,
    /// A period this crate does not model yet.
    #[serde(other)]
    Unknown,
}

/// A billing plan.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Plan {
    /// Unique identifier, e.g. `plan_00000000000001`.
    pub id: String,
    /// Always `"plan"`.
    #[serde(default)]
    pub entity: String,
    /// The billing unit.
    pub period: PlanPeriod,
    /// How many `period` units between charges — `period: Monthly, interval: 3`
    /// bills quarterly.
    pub interval: u32,
    /// The priced item this plan charges.
    pub item: Item,
    /// Your metadata.
    #[serde(default)]
    pub notes: Notes,
    /// Creation time as a Unix timestamp in seconds.
    #[serde(default)]
    pub created_at: i64,
}

/// Parameters for creating a [`Plan`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreatePlanParams {
    /// The billing unit.
    pub period: PlanPeriod,
    /// How many `period` units between charges.
    pub interval: u32,
    /// The item this plan bills for.
    pub item: PlanItem,
    /// Metadata to attach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Notes>,
}

/// The inline item definition sent when creating a plan.
///
/// Plans embed their item rather than referencing an existing [`Item`] id.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanItem {
    /// Display name shown to the customer.
    pub name: String,
    /// Amount charged each cycle, in the smallest currency unit.
    pub amount: i64,
    /// ISO 4217 currency code.
    pub currency: String,
    /// Longer description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl CreatePlanParams {
    /// A plan billing `amount` every `interval` × `period`.
    ///
    /// # Examples
    ///
    /// ```
    /// use razorpay_api::resources::{CreatePlanParams, PlanPeriod};
    ///
    /// // ₹499 every month.
    /// let monthly = CreatePlanParams::new(PlanPeriod::Monthly, 1, "Pro", 49_900, "INR");
    /// assert_eq!(monthly.interval, 1);
    ///
    /// // ₹499 every three months.
    /// let quarterly = CreatePlanParams::new(PlanPeriod::Monthly, 3, "Pro", 49_900, "INR");
    /// assert_eq!(quarterly.interval, 3);
    /// ```
    pub fn new(
        period: PlanPeriod,
        interval: u32,
        name: impl Into<String>,
        amount: i64,
        currency: impl Into<String>,
    ) -> Self {
        Self {
            period,
            interval,
            item: PlanItem {
                name: name.into(),
                amount,
                currency: currency.into(),
                description: None,
            },
            notes: None,
        }
    }

    /// Describe the item in more detail.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.item.description = Some(description.into());
        self
    }

    /// Attach metadata.
    pub fn notes(mut self, notes: Notes) -> Self {
        self.notes = Some(notes);
        self
    }
}

/// Plan endpoints. Obtain one from [`RazorpayClient::plans`].
///
/// Plans are immutable once created — to change pricing, create a new plan and
/// move subscribers to it.
pub struct PlansClient<'a> {
    pub(crate) client: &'a RazorpayClient,
}

impl<'a> PlansClient<'a> {
    /// Create a plan — `POST /v1/plans`.
    pub async fn create(&self, params: CreatePlanParams) -> Result<Plan, RazorpayError> {
        self.client.post("plans", Some(&params)).await
    }

    /// Fetch one plan by id — `GET /v1/plans/{id}`.
    pub async fn fetch(&self, id: &str) -> Result<Plan, RazorpayError> {
        self.client.get::<(), _>(&format!("plans/{id}"), None).await
    }

    /// List plans — `GET /v1/plans`.
    pub async fn all(&self, options: ListOptions) -> Result<Collection<Plan>, RazorpayError> {
        self.client.get("plans", Some(&options)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_serializes_with_nested_item() {
        let json =
            serde_json::to_string(&CreatePlanParams::new(PlanPeriod::Monthly, 1, "Pro", 100, "INR"))
                .unwrap();
        assert_eq!(
            json,
            r#"{"period":"monthly","interval":1,"item":{"name":"Pro","amount":100,"currency":"INR"}}"#
        );
    }

    #[test]
    fn unknown_period_decodes_to_fallback() {
        let plan: Plan = serde_json::from_str(
            r#"{"id":"plan_1","period":"fortnightly","interval":1,
                "item":{"id":"item_1","name":"x","amount":1,"currency":"INR"}}"#,
        )
        .unwrap();
        assert_eq!(plan.period, PlanPeriod::Unknown);
    }
}
