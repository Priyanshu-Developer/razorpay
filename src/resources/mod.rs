//! Typed API resources.
//!
//! Each submodule holds one resource's entity types, its request parameters, and a
//! `*Client` exposing its endpoints. You never construct those clients directly —
//! get them from the accessors on [`RazorpayClient`](crate::RazorpayClient):
//!
//! ```no_run
//! # use razorpay_sdk::{RazorpayClient, RazorpayError, ListOptions};
//! # async fn demo(client: RazorpayClient) -> Result<(), RazorpayError> {
//! let order = client.orders().fetch("order_123").await?;
//! let payments = client.payments().all(ListOptions::default()).await?;
//! # Ok(())
//! # }
//! ```

pub mod cards;
pub mod common;
pub mod customers;
pub mod invoices;
pub mod items;
pub mod orders;
pub mod payment_links;
pub mod payments;
pub mod plans;
pub mod refunds;
pub mod subscriptions;
pub mod tokens;
pub mod webhooks;

// Flat re-exports so callers can `use razorpay_sdk::resources::Order` without
// knowing which submodule it lives in.
pub use cards::{Card, CardsClient};
pub use common::{EntityMode, Notes, NotesUpdate, PaymentMethod};
pub use customers::{Customer, CustomerParams, CustomersClient};
pub use invoices::{CreateInvoiceParams, Invoice, InvoiceStatus, InvoicesClient, LineItem};
pub use items::{Item, ItemParams, ItemsClient};
pub use orders::{CreateOrderParams, Order, OrderStatus, OrdersClient};
pub use payment_links::{
    CreatePaymentLinkParams, LinkCustomer, NotifySettings, PaymentLink, PaymentLinkStatus,
    PaymentLinksClient,
};
pub use payments::{CaptureParams, Payment, PaymentStatus, PaymentsClient, RefundStatus};
pub use plans::{CreatePlanParams, Plan, PlanItem, PlanPeriod, PlansClient};
pub use refunds::{CreateRefundParams, Refund, RefundSpeed, RefundState, RefundsClient};
pub use subscriptions::{
    CreateSubscriptionParams, Subscription, SubscriptionStatus, SubscriptionsClient,
};
pub use tokens::Token;
pub use webhooks::{
    InvoicePayload, OrderPayload, PaymentPayload, RefundPayload, SubscriptionPayload, WebhookEvent,
    Wrapped,
};
