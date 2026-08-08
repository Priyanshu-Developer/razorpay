//! Shared list-query parameters and the list-response envelope.
//!
//! Every Razorpay list endpoint accepts the same four filters and answers with the
//! same envelope shape, so both live here once rather than per resource.

use serde::{Deserialize, Serialize};

/// Filters accepted by every `all()` / list endpoint.
///
/// All fields are optional and omitted from the query string when [`None`] —
/// Razorpay rejects `count=null`, so an unset field must not be serialized at all.
///
/// # Examples
///
/// The common case is "just give me the defaults":
///
/// ```
/// use razorpay_api::ListOptions;
///
/// let opts = ListOptions::default();
/// assert!(opts.count.is_none());
/// ```
///
/// Or build up a filtered window with the chainable setters:
///
/// ```
/// use razorpay_api::ListOptions;
///
/// let opts = ListOptions::new().count(25).skip(50).from(1_700_000_000);
/// assert_eq!(opts.count, Some(25));
/// assert_eq!(opts.skip, Some(50));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ListOptions {
    /// Unix timestamp (seconds); returns records created at or after this time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<i64>,

    /// Unix timestamp (seconds); returns records created at or before this time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<i64>,

    /// Number of records to fetch. Razorpay's default is 10 and its maximum is 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,

    /// Number of records to skip — combine with [`count`](Self::count) to page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<u32>,
}

impl ListOptions {
    /// An empty set of filters, equivalent to [`ListOptions::default()`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Only return records created at or after this Unix timestamp (seconds).
    pub fn from(mut self, from: i64) -> Self {
        self.from = Some(from);
        self
    }

    /// Only return records created at or before this Unix timestamp (seconds).
    pub fn to(mut self, to: i64) -> Self {
        self.to = Some(to);
        self
    }

    /// Fetch at most `count` records. Razorpay caps this at 100.
    pub fn count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    /// Skip the first `skip` records.
    pub fn skip(mut self, skip: u32) -> Self {
        self.skip = Some(skip);
        self
    }
}

/// The envelope every list endpoint returns: `{"entity": "collection", "count": N, "items": [...]}`.
///
/// Generic over the item type so one struct serves every resource.
///
/// # Examples
///
/// ```
/// # use razorpay_api::Collection;
/// # use razorpay_api::resources::Order;
/// # fn demo(page: Collection<Order>) {
/// for order in &page {
///     println!("{} is {:?}", order.id, order.status);
/// }
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Collection<T> {
    /// Always `"collection"` for list responses.
    #[serde(default)]
    pub entity: String,

    /// Number of items in [`items`](Self::items) — not the total matching records.
    #[serde(default)]
    pub count: u32,

    /// The page of records.
    #[serde(default = "Vec::new")]
    pub items: Vec<T>,
}

impl<T> Collection<T> {
    /// Number of items in this page.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether this page came back empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Borrowing iterator over the page's items.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }
}

impl<T> IntoIterator for Collection<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Collection<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl<T> Default for Collection<T> {
    fn default() -> Self {
        Self {
            entity: "collection".to_string(),
            count: 0,
            items: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_fields_are_omitted_entirely() {
        // `count=null` in a query string is a 400 from Razorpay, so absence matters.
        let qs = serde_urlencoded::to_string(ListOptions::default()).unwrap();
        assert_eq!(qs, "");
    }

    #[test]
    fn set_fields_serialize() {
        let qs = serde_urlencoded::to_string(ListOptions::new().count(10).skip(20)).unwrap();
        assert_eq!(qs, "count=10&skip=20");
    }

    #[test]
    fn collection_deserializes_and_iterates() {
        let json = r#"{"entity":"collection","count":2,"items":[1,2]}"#;
        let c: Collection<i64> = serde_json::from_str(json).unwrap();
        assert_eq!(c.count, 2);
        assert_eq!(c.len(), 2);
        assert!(!c.is_empty());
        assert_eq!(c.iter().sum::<i64>(), 3);
        assert_eq!(c.into_iter().collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn missing_items_defaults_to_empty_rather_than_failing() {
        let c: Collection<i64> = serde_json::from_str(r#"{"entity":"collection","count":0}"#).unwrap();
        assert!(c.is_empty());
    }
}
