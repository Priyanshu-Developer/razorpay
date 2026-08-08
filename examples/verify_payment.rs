//! Verify the signature Checkout posts back after a payment.
//!
//! Runs offline — no API keys or network needed:
//!
//! ```sh
//! cargo run --example verify_payment
//! ```
//!
//! # Why this step exists
//!
//! After Checkout completes, the *browser* posts `razorpay_order_id`,
//! `razorpay_payment_id`, and `razorpay_signature` to your server. The browser is
//! attacker-controlled. Skipping verification means anyone can POST a fabricated
//! "payment succeeded" and receive goods for free.

use razorpay_sdk::signature::{compute_signature, verify_payment_signature};

fn main() {
    // In production these come from your API key secret and the Checkout callback.
    let key_secret = "EnLs21M47BllR3X8PSFtjtbd";
    let order_id = "order_IEIaMR65cU6MI1";
    let payment_id = "pay_IEIazBq55mBSmS";

    // Razorpay signs "{order_id}|{payment_id}" with your key secret. We compute it
    // here only to have a genuine value to demonstrate with — in a real handler
    // this arrives from the browser as `razorpay_signature`.
    let signature = compute_signature(&format!("{order_id}|{payment_id}"), key_secret);

    println!("order   : {order_id}");
    println!("payment : {payment_id}");
    println!("signature: {signature}\n");

    match verify_payment_signature(order_id, payment_id, &signature, key_secret) {
        Ok(()) => println!("genuine payment — safe to fulfil the order"),
        Err(e) => println!("rejected: {e}"),
    }

    // A tampered payment id fails, which is the whole point.
    let forged = verify_payment_signature(order_id, "pay_attacker_made_this", &signature, key_secret);
    println!(
        "forged payment id -> {}",
        match forged {
            Ok(()) => "accepted (this would be a bug!)".to_string(),
            Err(e) => format!("rejected: {e}"),
        }
    );

    // So does an unrelated signature.
    let bad_sig = verify_payment_signature(order_id, payment_id, "deadbeef", key_secret);
    println!(
        "garbage signature -> {}",
        match bad_sig {
            Ok(()) => "accepted (this would be a bug!)".to_string(),
            Err(e) => format!("rejected: {e}"),
        }
    );
}
