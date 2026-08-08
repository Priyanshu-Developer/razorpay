//! Subscriptions — recurring charges against a [`Plan`](crate::resources::plans::Plan).

use serde::{Deserialize, Serialize};

use crate::client::RazorpayClient;
use crate::error::RazorpayError;
use crate::resources::common::{Notes, NotesUpdate};
use crate::util::pagination::{Collection, ListOptions};

/// Lifecycle state of a [`Subscription`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    /// Created but the customer has not yet authorized it.
    Created,
    /// Waiting on the customer to complete authentication.
    Authenticated,
    /// Billing normally.
    Active,
    /// Paused; no charges are attempted.
    Paused,
    /// A charge failed and Razorpay is retrying.
    Pending,
    /// Retries exhausted.
    Halted,
    /// Ran to its scheduled end.
    Completed,
    /// Cancelled before completing.
    Cancelled,
    /// Expired without being authorized.
    Expired,
    /// A status this crate does not model yet.
    #[serde(other)]
    Unknown,
}

/// A subscription returned by the API.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Subscription {
    /// Unique identifier, e.g. `sub_00000000000001`.
    pub id: String,
    /// Always `"subscription"`.
    #[serde(default)]
    pub entity: String,
    /// The plan being billed.
    pub plan_id: String,
    /// Current lifecycle state.
    pub status: SubscriptionStatus,
    /// The customer being billed.
    #[serde(default)]
    pub customer_id: Option<String>,
    /// Total billing cycles scheduled.
    #[serde(default)]
    pub total_count: u32,
    /// Cycles billed so far.
    #[serde(default)]
    pub paid_count: u32,
    /// Cycles remaining.
    #[serde(default)]
    pub remaining_count: u32,
    /// How many charges have failed.
    #[serde(default)]
    pub auth_attempts: u32,
    /// Quantity of the plan being billed each cycle.
    #[serde(default)]
    pub quantity: u32,
    /// URL where the customer authorizes the subscription.
    #[serde(default)]
    pub short_url: Option<String>,
    /// Whether Razorpay sends the customer notifications.
    #[serde(default)]
    pub customer_notify: bool,
    /// Start of the current billing cycle, Unix seconds.
    #[serde(default)]
    pub current_start: Option<i64>,
    /// End of the current billing cycle, Unix seconds.
    #[serde(default)]
    pub current_end: Option<i64>,
    /// When the subscription began, Unix seconds.
    #[serde(default)]
    pub start_at: Option<i64>,
    /// When the subscription is scheduled to end, Unix seconds.
    #[serde(default)]
    pub end_at: Option<i64>,
    /// When it was actually ended, Unix seconds.
    #[serde(default)]
    pub ended_at: Option<i64>,
    /// When the trial period ends, Unix seconds.
    #[serde(default)]
    pub expire_by: Option<i64>,
    /// When it was charged most recently, Unix seconds.
    #[serde(default)]
    pub charge_at: Option<i64>,
    /// Any one-off charges added on top of the plan.
    #[serde(default)]
    pub addons: Vec<serde_json::Value>,
    /// Your metadata.
    #[serde(default)]
    pub notes: Notes,
    /// Creation time as a Unix timestamp in seconds.
    #[serde(default)]
    pub created_at: i64,
}

impl Subscription {
    /// Whether the subscription is currently billing.
    pub fn is_active(&self) -> bool {
        matches!(self.status, SubscriptionStatus::Active)
    }
}

/// Parameters for creating a [`Subscription`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreateSubscriptionParams {
    /// The plan to bill against.
    pub plan_id: String,
    /// How many cycles to bill before completing.
    pub total_count: u32,
    /// Quantity of the plan per cycle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,
    /// Whether Razorpay should email/SMS the customer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_notify: Option<u8>,
    /// When to start billing, Unix seconds. Omitted means immediately.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_at: Option<i64>,
    /// Deadline for the customer to authorize, Unix seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_by: Option<i64>,
    /// Cycles to skip charging at the start, for a free trial.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    /// Metadata to attach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Notes>,
}

impl CreateSubscriptionParams {
    /// Bill `plan_id` for `total_count` cycles.
    pub fn new(plan_id: impl Into<String>, total_count: u32) -> Self {
        Self {
            plan_id: plan_id.into(),
            total_count,
            quantity: None,
            customer_notify: None,
            start_at: None,
            expire_by: None,
            customer_id: None,
            notes: None,
        }
    }

    /// Bill an existing customer.
    pub fn customer_id(mut self, customer_id: impl Into<String>) -> Self {
        self.customer_id = Some(customer_id.into());
        self
    }

    /// Bill more than one unit of the plan per cycle.
    pub fn quantity(mut self, quantity: u32) -> Self {
        self.quantity = Some(quantity);
        self
    }

    /// Delay the first charge until this Unix timestamp.
    pub fn start_at(mut self, start_at: i64) -> Self {
        self.start_at = Some(start_at);
        self
    }

    /// Let Razorpay notify the customer about this subscription.
    pub fn customer_notify(mut self, notify: bool) -> Self {
        self.customer_notify = Some(u8::from(notify));
        self
    }

    /// Attach metadata.
    pub fn notes(mut self, notes: Notes) -> Self {
        self.notes = Some(notes);
        self
    }
}

/// Body for [`SubscriptionsClient::cancel`].
#[derive(Debug, Clone, PartialEq, Serialize)]
struct CancelParams {
    cancel_at_cycle_end: u8,
}

/// Body for [`SubscriptionsClient::pause`] and [`SubscriptionsClient::resume`].
#[derive(Debug, Clone, PartialEq, Serialize)]
struct PauseResumeParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pause_at: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resume_at: Option<&'static str>,
}

/// Subscription endpoints. Obtain one from [`RazorpayClient::subscriptions`].
pub struct SubscriptionsClient<'a> {
    pub(crate) client: &'a RazorpayClient,
}

impl<'a> SubscriptionsClient<'a> {
    /// Create a subscription — `POST /v1/subscriptions`.
    ///
    /// The returned [`Subscription::short_url`] is where the customer authorizes
    /// recurring charges; nothing is billed until they do.
    pub async fn create(
        &self,
        params: CreateSubscriptionParams,
    ) -> Result<Subscription, RazorpayError> {
        self.client.post("subscriptions", Some(&params)).await
    }

    /// Fetch one subscription by id — `GET /v1/subscriptions/{id}`.
    pub async fn fetch(&self, id: &str) -> Result<Subscription, RazorpayError> {
        self.client
            .get::<(), _>(&format!("subscriptions/{id}"), None)
            .await
    }

    /// List subscriptions — `GET /v1/subscriptions`.
    pub async fn all(
        &self,
        options: ListOptions,
    ) -> Result<Collection<Subscription>, RazorpayError> {
        self.client.get("subscriptions", Some(&options)).await
    }

    /// Cancel a subscription — `POST /v1/subscriptions/{id}/cancel`.
    ///
    /// `at_cycle_end` of `true` lets the customer keep access through the period
    /// they already paid for; `false` cancels immediately.
    pub async fn cancel(
        &self,
        id: &str,
        at_cycle_end: bool,
    ) -> Result<Subscription, RazorpayError> {
        let body = CancelParams {
            cancel_at_cycle_end: u8::from(at_cycle_end),
        };
        self.client
            .post(&format!("subscriptions/{id}/cancel"), Some(&body))
            .await
    }

    /// Pause a subscription — `POST /v1/subscriptions/{id}/pause`.
    ///
    /// Charging stops until [`resume`](Self::resume); the schedule is not extended.
    pub async fn pause(&self, id: &str) -> Result<Subscription, RazorpayError> {
        let body = PauseResumeParams {
            pause_at: Some("now"),
            resume_at: None,
        };
        self.client
            .post(&format!("subscriptions/{id}/pause"), Some(&body))
            .await
    }

    /// Resume a paused subscription — `POST /v1/subscriptions/{id}/resume`.
    pub async fn resume(&self, id: &str) -> Result<Subscription, RazorpayError> {
        let body = PauseResumeParams {
            pause_at: None,
            resume_at: Some("now"),
        };
        self.client
            .post(&format!("subscriptions/{id}/resume"), Some(&body))
            .await
    }

    /// Replace a subscription's notes — `PATCH /v1/subscriptions/{id}`.
    pub async fn edit(&self, id: &str, notes: Notes) -> Result<Subscription, RazorpayError> {
        self.client
            .patch(&format!("subscriptions/{id}"), Some(&NotesUpdate::new(notes)))
            .await
    }

    /// List invoices raised for a subscription — `GET /v1/invoices?subscription_id={id}`.
    pub async fn invoices(
        &self,
        id: &str,
    ) -> Result<Collection<crate::resources::invoices::Invoice>, RazorpayError> {
        #[derive(Serialize)]
        struct Query<'q> {
            subscription_id: &'q str,
        }
        self.client
            .get("invoices", Some(&Query { subscription_id: id }))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_omits_unset_fields() {
        let json = serde_json::to_string(&CreateSubscriptionParams::new("plan_1", 12)).unwrap();
        assert_eq!(json, r#"{"plan_id":"plan_1","total_count":12}"#);
    }

    #[test]
    fn customer_notify_maps_bool_to_int() {
        let json = serde_json::to_string(
            &CreateSubscriptionParams::new("plan_1", 1).customer_notify(true),
        )
        .unwrap();
        assert!(json.contains(r#""customer_notify":1"#));
    }

    #[test]
    fn active_helper_reflects_status() {
        let sub: Subscription = serde_json::from_str(
            r#"{"id":"sub_1","plan_id":"plan_1","status":"active"}"#,
        )
        .unwrap();
        assert!(sub.is_active());
    }
}
