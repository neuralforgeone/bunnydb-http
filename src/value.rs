/// Logical value type used for SQL parameters and decoded rows.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub enum Value {
    /// SQL null.
    Null,
    /// Signed integer.
    Integer(i64),
    /// Floating-point number (must be finite).
    Float(f64),
    /// UTF-8 text.
    Text(String),
    /// Base64-encoded binary payload.
    BlobBase64(String),
}

impl Value {
    /// Creates a null value.
    pub fn null() -> Self {
        Self::Null
    }

    /// Creates an integer value.
    pub fn integer(value: i64) -> Self {
        Self::Integer(value)
    }

    /// Creates a float value.
    pub fn float(value: f64) -> Self {
        Self::Float(value)
    }

    /// Creates a text value.
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    /// Creates a base64 blob value.
    pub fn blob_base64(value: impl Into<String>) -> Self {
        Self::BlobBase64(value.into())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Self::Integer(value.into())
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::Value;

    #[test]
    fn helper_constructors() {
        assert_eq!(Value::null(), Value::Null);
        assert_eq!(Value::integer(7), Value::Integer(7));
        assert_eq!(Value::float(1.25), Value::Float(1.25));
        assert_eq!(Value::text("abc"), Value::Text("abc".to_owned()));
        assert_eq!(
            Value::blob_base64("AQID"),
            Value::BlobBase64("AQID".to_owned())
        );
    }
}
