#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Float(f64),
    Text(String),
    BlobBase64(String),
}

impl Value {
    pub fn null() -> Self {
        Self::Null
    }

    pub fn integer(value: i64) -> Self {
        Self::Integer(value)
    }

    pub fn float(value: f64) -> Self {
        Self::Float(value)
    }

    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

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
