//! Set up recurring billing: customer → plan → subscription.
//!
//! ```sh
//! export RAZORPAY_KEY_ID=rzp_test_...
//! export RAZORPAY_KEY_SECRET=...
//! cargo run --example subscription
//! ```

use razorpay_api::resources::{CreatePlanParams, CreateSubscriptionParams, CustomerParams, PlanPeriod};
use razorpay_api::{RazorpayClient, RazorpayError};

#[tokio::main]
async fn main() -> Result<(), RazorpayError> {
    let key_id = std::env::var("RAZORPAY_KEY_ID").expect("set RAZORPAY_KEY_ID");
    let key_secret = std::env::var("RAZORPAY_KEY_SECRET").expect("set RAZORPAY_KEY_SECRET");
    let client = RazorpayClient::new(key_id, key_secret);

    // 1. The customer being billed. `reuse_existing` makes this idempotent —
    //    without it, Razorpay errors when the customer already exists.
    let customer = client
        .customers()
        .create(
            CustomerParams::new()
                .name("Asha Rao")
                .email("asha@example.com")
                .contact("+919999999999")
                .reuse_existing(),
        )
        .await?;
    println!("customer     : {}", customer.id);

    // 2. The plan: what to charge and how often. ₹499 every month.
    //    Plans are immutable — to change pricing, create a new one.
    let plan = client
        .plans()
        .create(
            CreatePlanParams::new(PlanPeriod::Monthly, 1, "Pro", 49_900, "INR")
                .description("Pro tier, billed monthly"),
        )
        .await?;
    println!("plan         : {} ({:?} x{})", plan.id, plan.period, plan.interval);

    // 3. The subscription ties them together for a fixed number of cycles.
    let subscription = client
        .subscriptions()
        .create(
            CreateSubscriptionParams::new(&plan.id, 12)
                .customer_id(&customer.id)
                .customer_notify(true),
        )
        .await?;

    println!("subscription : {}", subscription.id);
    println!("status       : {:?}", subscription.status);
    println!("cycles       : {} total", subscription.total_count);

    // Nothing is billed until the customer authorizes at this URL.
    match &subscription.short_url {
        Some(url) => println!("\nSend the customer here to authorize:\n  {url}"),
        None => println!("\nNo authorization URL returned."),
    }

    println!("\nWatch for `subscription.charged` webhooks to track each cycle.");

    Ok(())
}
