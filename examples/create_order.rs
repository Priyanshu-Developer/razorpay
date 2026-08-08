//! Create an order — the first step of every integration.
//!
//! ```sh
//! export RAZORPAY_KEY_ID=rzp_test_...
//! export RAZORPAY_KEY_SECRET=...
//! cargo run --example create_order
//! ```

use std::collections::HashMap;

use razorpay_sdk::resources::CreateOrderParams;
use razorpay_sdk::{ListOptions, RazorpayClient, RazorpayError};

#[tokio::main]
async fn main() -> Result<(), RazorpayError> {
    let key_id = std::env::var("RAZORPAY_KEY_ID").expect("set RAZORPAY_KEY_ID");
    let key_secret = std::env::var("RAZORPAY_KEY_SECRET").expect("set RAZORPAY_KEY_SECRET");

    // Build the client once; it pools connections internally.
    let client = RazorpayClient::new(key_id, key_secret);

    let notes = HashMap::from([("customer_ref".to_string(), "user_42".to_string())]);

    // 50_000 paise is ₹500 — amounts are always in the smallest currency unit.
    let order = client
        .orders()
        .create(
            CreateOrderParams::new(50_000, "INR")
                .receipt("rcpt#1001")
                .notes(notes),
        )
        .await?;

    println!("created  : {}", order.id);
    println!("amount   : {} paise", order.amount);
    println!("status   : {:?}", order.status);
    println!("\nHand `{}` to Checkout on the front end.", order.id);

    // Fetching it back confirms the round trip.
    let fetched = client.orders().fetch(&order.id).await?;
    println!("\nrefetched: {} ({:?})", fetched.id, fetched.status);

    // Every list endpoint takes ListOptions and returns a Collection<T>.
    let recent = client.orders().all(ListOptions::new().count(5)).await?;
    println!("\n{} recent order(s):", recent.len());
    for o in &recent {
        println!("  {} — {} {} — {:?}", o.id, o.amount, o.currency, o.status);
    }

    Ok(())
}
