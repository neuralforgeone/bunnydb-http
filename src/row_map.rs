//! Experimental row mapping helpers.
//!
//! Enabled with the `row-map` feature.

use crate::{Col, Value};

#[derive(Debug)]
pub struct RowRef<'a> {
    pub cols: &'a [Col],
    pub values: &'a [Value],
}

impl<'a> RowRef<'a> {
    pub fn get(&self, name: &str) -> Option<&Value> {
        let idx = self
            .cols
            .iter()
            .position(|col| col.name.eq_ignore_ascii_case(name))?;
        self.values.get(idx)
    }

    pub fn get_i64(&self, name: &str) -> Option<i64> {
        match self.get(name)? {
            Value::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn get_f64(&self, name: &str) -> Option<f64> {
        match self.get(name)? {
            Value::Float(value) => Some(*value),
            _ => None,
        }
    }

    pub fn get_text(&self, name: &str) -> Option<&str> {
        match self.get(name)? {
            Value::Text(value) => Some(value.as_str()),
            _ => None,
        }
    }
}
