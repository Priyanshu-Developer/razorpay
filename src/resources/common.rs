//! Types shared across more than one resource.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Free-form key/value metadata attached to most Razorpay entities.
///
/// Razorpay allows up to 15 keys, with string keys and values.
pub type Notes = HashMap<String, String>;

/// A request body carrying only `notes`, used by the several `edit` endpoints
/// whose sole editable field is metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NotesUpdate {
    /// The replacement notes map.
    pub notes: Notes,
}

impl NotesUpdate {
    /// Wrap a notes map into an edit body.
    pub fn new(notes: Notes) -> Self {
        Self { notes }
    }
}

impl From<Notes> for NotesUpdate {
    fn from(notes: Notes) -> Self {
        Self { notes }
    }
}

/// How a payment was made.
///
/// Carries an [`Unknown`](PaymentMethod::Unknown) fallback: Razorpay adds methods
/// without a version bump, and a strict enum would turn that into a decode failure
/// on a previously working integration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    /// Credit or debit card.
    Card,
    /// Netbanking.
    Netbanking,
    /// Wallet (Paytm, PhonePe, …).
    Wallet,
    /// UPI.
    Upi,
    /// EMI.
    Emi,
    /// Pay-later provider.
    Paylater,
    /// Cardless EMI.
    Cardless,
    /// A method this crate does not model yet.
    #[serde(other)]
    Unknown,
}

/// Whether an entity is in test or live mode — mirrors which API key was used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityMode {
    /// Created with test keys; no real money moved.
    Test,
    /// Created with live keys.
    Live,
    /// A mode this crate does not model yet.
    #[serde(other)]
    Unknown,
}
