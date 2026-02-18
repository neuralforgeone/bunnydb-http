use crate::{
    wire::{self, ExecuteStatement, NamedArg},
    BunnyDbError, Col, ExecResult, Params, QueryResult, Value,
};

pub(crate) fn build_execute_statement(
    sql: &str,
    params: Params,
    want_rows: bool,
) -> Result<ExecuteStatement, BunnyDbError> {
    match params {
        Params::Positional(values) => {
            let args = values
                .into_iter()
                .map(encode_value)
                .collect::<Result<Vec<_>, _>>()?;

            Ok(ExecuteStatement {
                sql: sql.to_owned(),
                args: (!args.is_empty()).then_some(args),
                named_args: None,
                want_rows,
            })
        }
        Params::Named(values) => {
            let named_args = values
                .into_iter()
                .map(|(name, value)| {
                    let name = normalize_named_parameter_name(&name)?;
                    let value = encode_value(value)?;
                    Ok(NamedArg { name, value })
                })
                .collect::<Result<Vec<_>, BunnyDbError>>()?;

            Ok(ExecuteStatement {
                sql: sql.to_owned(),
                args: None,
                named_args: (!named_args.is_empty()).then_some(named_args),
                want_rows,
            })
        }
    }
}

pub(crate) fn decode_query_result(
    result: wire::ExecuteResult,
) -> Result<QueryResult, BunnyDbError> {
    let cols = result
        .cols
        .into_iter()
        .map(|col| Col {
            name: col.name,
            decltype: col.decltype,
        })
        .collect();

    let rows = result
        .rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(decode_value)
                .collect::<Result<Vec<_>, BunnyDbError>>()
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(QueryResult {
        cols,
        rows,
        replication_index: result.replication_index,
        rows_read: result.rows_read,
        rows_written: result.rows_written,
        query_duration_ms: result.query_duration_ms,
    })
}

pub(crate) fn decode_exec_result(result: wire::ExecuteResult) -> Result<ExecResult, BunnyDbError> {
    let last_insert_rowid = result
        .last_insert_rowid
        .map(|value| {
            value.parse::<i64>().map_err(|err| {
                BunnyDbError::Decode(format!("invalid last_insert_rowid '{value}': {err}"))
            })
        })
        .transpose()?;

    Ok(ExecResult {
        affected_row_count: result.affected_row_count,
        last_insert_rowid,
        replication_index: result.replication_index,
        rows_read: result.rows_read,
        rows_written: result.rows_written,
    })
}

pub(crate) fn decode_value(value: wire::Value) -> Result<Value, BunnyDbError> {
    match value {
        wire::Value::Null {} => Ok(Value::Null),
        wire::Value::Integer { value } => value
            .parse::<i64>()
            .map(Value::Integer)
            .map_err(|err| BunnyDbError::Decode(format!("invalid integer value '{value}': {err}"))),
        wire::Value::Float { value } => value
            .parse::<f64>()
            .map_err(|err| BunnyDbError::Decode(format!("invalid float value '{value}': {err}")))
            .and_then(|parsed| {
                if parsed.is_finite() {
                    Ok(Value::Float(parsed))
                } else {
                    Err(BunnyDbError::Decode(format!(
                        "non-finite float value '{value}' is unsupported"
                    )))
                }
            }),
        wire::Value::Text { value } => Ok(Value::Text(value)),
        wire::Value::Blob { base64 } => Ok(Value::BlobBase64(base64)),
    }
}

fn encode_value(value: Value) -> Result<wire::Value, BunnyDbError> {
    match value {
        Value::Null => Ok(wire::Value::Null {}),
        Value::Integer(value) => Ok(wire::Value::Integer {
            value: value.to_string(),
        }),
        Value::Float(value) => {
            if !value.is_finite() {
                return Err(BunnyDbError::Decode(format!(
                    "non-finite float value '{value}' is unsupported"
                )));
            }
            Ok(wire::Value::Float {
                value: value.to_string(),
            })
        }
        Value::Text(value) => Ok(wire::Value::Text { value }),
        Value::BlobBase64(base64) => Ok(wire::Value::Blob { base64 }),
    }
}

fn normalize_named_parameter_name(name: &str) -> Result<String, BunnyDbError> {
    let normalized = name.trim_start_matches([':', '@', '$']);
    if normalized.is_empty() {
        return Err(BunnyDbError::Decode(
            "named parameter name cannot be empty".to_owned(),
        ));
    }
    Ok(normalized.to_owned())
}

#[cfg(test)]
mod tests {
    use crate::{decode, wire, BunnyDbError, Params, Value};

    #[test]
    fn build_positional_stmt() {
        let stmt = decode::build_execute_statement(
            "SELECT ?",
            Params::positional([Value::integer(1)]),
            true,
        )
        .expect("must build statement");
        assert!(stmt.args.is_some());
        assert!(stmt.named_args.is_none());
    }

    #[test]
    fn build_named_stmt_strips_prefix() {
        let stmt = decode::build_execute_statement(
            "SELECT :name",
            Params::named([(":name", Value::text("kit"))]),
            true,
        )
        .expect("must build statement");

        let args = stmt.named_args.expect("must contain named args");
        assert_eq!(args[0].name, "name");
    }

    #[test]
    fn build_rejects_non_finite_float() {
        let err = decode::build_execute_statement(
            "SELECT ?",
            Params::positional([Value::float(f64::NAN)]),
            true,
        )
        .expect_err("must fail");

        assert!(matches!(err, BunnyDbError::Decode(_)));
    }

    #[test]
    fn decode_integer_parse_error() {
        let value = wire::Value::Integer {
            value: "nope".to_owned(),
        };
        let err = decode::decode_value(value).expect_err("must fail");
        assert!(matches!(err, BunnyDbError::Decode(_)));
    }

    #[test]
    fn decode_query_result_preserves_telemetry() {
        let decoded = decode::decode_query_result(wire::ExecuteResult {
            cols: vec![],
            rows: vec![],
            affected_row_count: 0,
            last_insert_rowid: None,
            replication_index: Some("42".to_owned()),
            rows_read: Some(11),
            rows_written: Some(3),
            query_duration_ms: Some(1.75),
        })
        .expect("must decode");

        assert_eq!(decoded.replication_index.as_deref(), Some("42"));
        assert_eq!(decoded.rows_read, Some(11));
        assert_eq!(decoded.rows_written, Some(3));
        assert_eq!(decoded.query_duration_ms, Some(1.75));
    }

    #[test]
    fn decode_exec_result_preserves_telemetry() {
        let decoded = decode::decode_exec_result(wire::ExecuteResult {
            cols: vec![],
            rows: vec![],
            affected_row_count: 1,
            last_insert_rowid: Some("7".to_owned()),
            replication_index: Some("43".to_owned()),
            rows_read: Some(2),
            rows_written: Some(1),
            query_duration_ms: Some(0.25),
        })
        .expect("must decode");

        assert_eq!(decoded.affected_row_count, 1);
        assert_eq!(decoded.last_insert_rowid, Some(7));
        assert_eq!(decoded.replication_index.as_deref(), Some("43"));
        assert_eq!(decoded.rows_read, Some(2));
        assert_eq!(decoded.rows_written, Some(1));
    }
}
