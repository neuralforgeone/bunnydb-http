use crate::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum Params {
    Positional(Vec<Value>),
    Named(Vec<(String, Value)>),
}

impl Params {
    pub fn positional(values: impl Into<Vec<Value>>) -> Self {
        Self::Positional(values.into())
    }

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

#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    pub sql: String,
    pub params: Params,
    pub want_rows: bool,
}

impl Statement {
    pub fn query<P: Into<Params>>(sql: impl Into<String>, params: P) -> Self {
        Self {
            sql: sql.into(),
            params: params.into(),
            want_rows: true,
        }
    }

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
