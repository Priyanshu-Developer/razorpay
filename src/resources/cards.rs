//! Cards — read-only details of the instrument behind a card payment.

use serde::Deserialize;

use crate::client::RazorpayClient;
use crate::error::RazorpayError;

/// Card details.
///
/// Only the last four digits and network metadata are ever exposed — Razorpay
/// never returns a full card number.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Card {
    /// Unique identifier, e.g. `card_JXPULjlR3DjqLp`.
    #[serde(default)]
    pub id: Option<String>,
    /// Always `"card"`.
    #[serde(default)]
    pub entity: String,
    /// Name printed on the card.
    #[serde(default)]
    pub name: Option<String>,
    /// Last four digits.
    #[serde(default)]
    pub last4: Option<String>,
    /// Card network, e.g. `Visa`, `MasterCard`, `RuPay`.
    #[serde(default)]
    pub network: Option<String>,
    /// `credit`, `debit`, or `prepaid`.
    #[serde(rename = "type", default)]
    pub card_type: Option<String>,
    /// Issuing bank code.
    #[serde(default)]
    pub issuer: Option<String>,
    /// Whether the card is enrolled for international payments.
    #[serde(default)]
    pub international: bool,
    /// Whether the card supports EMI.
    #[serde(default)]
    pub emi: bool,
    /// Whether the card can be used for subscriptions.
    #[serde(default)]
    pub sub_type: Option<String>,
    /// First six digits, when Razorpay exposes them.
    #[serde(default)]
    pub iin: Option<String>,
    /// Expiry month, 1–12.
    #[serde(default)]
    pub expiry_month: Option<u32>,
    /// Expiry year, four digits.
    #[serde(default)]
    pub expiry_year: Option<u32>,
}

/// Card endpoints. Obtain one from [`RazorpayClient::cards`].
pub struct CardsClient<'a> {
    pub(crate) client: &'a RazorpayClient,
}

impl<'a> CardsClient<'a> {
    /// Fetch card details by id — `GET /v1/cards/{id}`.
    ///
    /// The id comes from [`Payment::card_id`](crate::resources::payments::Payment::card_id).
    pub async fn fetch(&self, id: &str) -> Result<Card, RazorpayError> {
        self.client.get::<(), _>(&format!("cards/{id}"), None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_field_is_renamed_from_reserved_word() {
        let card: Card = serde_json::from_str(r#"{"id":"card_1","type":"credit","last4":"1111"}"#).unwrap();
        assert_eq!(card.card_type.as_deref(), Some("credit"));
        assert_eq!(card.last4.as_deref(), Some("1111"));
    }
}
