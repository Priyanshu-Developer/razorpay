//! Tokens — saved instruments that let a customer pay without re-entering details.

use serde::Deserialize;

use crate::resources::cards::Card;
use crate::resources::common::Notes;

/// A saved payment instrument belonging to a customer.
///
/// Tokens are created by Checkout when a customer opts to save their card, not
/// through this crate; fetch and delete them via
/// [`CustomersClient`](crate::resources::customers::CustomersClient).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Token {
    /// Unique identifier, e.g. `token_HouA2OQR5Z2jTL`.
    pub id: String,
    /// Always `"token"`.
    #[serde(default)]
    pub entity: String,
    /// The payment method this token represents, e.g. `card`.
    #[serde(default)]
    pub method: Option<String>,
    /// Card details, when this is a card token.
    #[serde(default)]
    pub card: Option<Card>,
    /// VPA details, when this is a UPI token.
    #[serde(default)]
    pub vpa: Option<serde_json::Value>,
    /// Bank code, for netbanking tokens.
    #[serde(default)]
    pub bank: Option<String>,
    /// Wallet name, for wallet tokens.
    #[serde(default)]
    pub wallet: Option<String>,
    /// Whether the customer chose to keep this instrument on file.
    #[serde(default)]
    pub recurring: bool,
    /// Why recurring payments are unavailable on this token, when they are.
    #[serde(default)]
    pub recurring_details: Option<serde_json::Value>,
    /// Whether this is the customer's default instrument.
    #[serde(default)]
    pub auth_type: Option<String>,
    /// Last time this token was used, as a Unix timestamp in seconds.
    #[serde(default)]
    pub used_at: Option<i64>,
    /// Expiry as a Unix timestamp in seconds.
    #[serde(default)]
    pub expired_at: Option<i64>,
    /// Your metadata.
    #[serde(default)]
    pub notes: Notes,
    /// Creation time as a Unix timestamp in seconds.
    #[serde(default)]
    pub created_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_token_decodes() {
        let token: Token = serde_json::from_str(r#"{"id":"token_1","method":"card"}"#).unwrap();
        assert_eq!(token.method.as_deref(), Some("card"));
        assert!(!token.recurring);
    }
}
