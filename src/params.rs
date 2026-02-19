use crate::Value;

/// SQL parameter container.
#[derive(Clone, Debug, PartialEq)]
pub enum Params {
    /// Positional values mapped to `?` placeholders.
    Positional(Vec<Value>),
    /// Named values mapped to `:name` style placeholders.
    Named(Vec<(String, Value)>),
}

impl Params {
    /// Builds positional parameters.
    pub fn positional(values: impl Into<Vec<Value>>) -> Self {
        Self::Positional(values.into())
    }

    /// Builds named parameters.
    ///
    /// Names can be provided with or without prefix (`:`, `@`, `$`).
    pub fn named<I, K>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, Value)>,
        K: Into<String>,
    {
        Self::Named(
            pairs
                .into_iter()
                .map(|(name, value)| (name.into(), value))
                .collect(),
        )
    }
}

impl Default for Params {
    fn default() -> Self {
        Self::Positional(Vec::new())
    }
}

impl From<()> for Params {
    fn from(_: ()) -> Self {
        Self::default()
    }
}

impl From<Vec<Value>> for Params {
    fn from(values: Vec<Value>) -> Self {
        Self::Positional(values)
    }
}

impl<const N: usize> From<[Value; N]> for Params {
    fn from(values: [Value; N]) -> Self {
        Self::Positional(values.into())
    }
}

impl From<Vec<(String, Value)>> for Params {
    fn from(values: Vec<(String, Value)>) -> Self {
        Self::Named(values)
    }
}

/// Single statement inside a batch request.
#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    /// SQL text.
    pub sql: String,
    /// Statement parameters.
    pub params: Params,
    /// Whether the statement should return rows.
    pub want_rows: bool,
}

impl Statement {
    /// Creates a row-returning statement.
    pub fn query<P: Into<Params>>(sql: impl Into<String>, params: P) -> Self {
        Self {
            sql: sql.into(),
            params: params.into(),
            want_rows: true,
        }
    }

    /// Creates an execution-only statement.
    pub fn execute<P: Into<Params>>(sql: impl Into<String>, params: P) -> Self {
        Self {
            sql: sql.into(),
            params: params.into(),
            want_rows: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Params, Statement, Value};

    #[test]
    fn positional_from_array() {
        let params: Params = [Value::integer(1), Value::text("kit")].into();
        match params {
            Params::Positional(values) => assert_eq!(values.len(), 2),
            _ => panic!("expected positional"),
        }
    }

    #[test]
    fn named_builder() {
        let params = Params::named([("name", Value::text("kit"))]);
        match params {
            Params::Named(values) => {
                assert_eq!(values.len(), 1);
                assert_eq!(values[0].0, "name");
            }
            _ => panic!("expected named"),
        }
    }

    #[test]
    fn statement_constructors() {
        let query = Statement::query("SELECT 1", ());
        let exec = Statement::execute("DELETE FROM t", ());
        assert!(query.want_rows);
        assert!(!exec.want_rows);
    }
}
