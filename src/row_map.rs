//! Experimental row mapping helpers.
//!
//! Enabled with the `row-map` feature.

use crate::{Col, Value};

/// Lightweight row view for name-based access helpers.
#[derive(Debug)]
pub struct RowRef<'a> {
    /// Query columns aligned with `values`.
    pub cols: &'a [Col],
    /// Row values aligned with `cols`.
    pub values: &'a [Value],
}

impl<'a> RowRef<'a> {
    /// Returns a value by case-insensitive column name.
    pub fn get(&self, name: &str) -> Option<&Value> {
        let idx = self
            .cols
            .iter()
            .position(|col| col.name.eq_ignore_ascii_case(name))?;
        self.values.get(idx)
    }

    /// Returns an integer value by column name.
    pub fn get_i64(&self, name: &str) -> Option<i64> {
        match self.get(name)? {
            Value::Integer(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns a float value by column name.
    pub fn get_f64(&self, name: &str) -> Option<f64> {
        match self.get(name)? {
            Value::Float(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns a text value by column name.
    pub fn get_text(&self, name: &str) -> Option<&str> {
        match self.get(name)? {
            Value::Text(value) => Some(value.as_str()),
            _ => None,
        }
    }
}
