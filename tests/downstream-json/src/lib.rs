#[cfg(test)]
mod tests {
    use trail_config::{Config, ConfigError, Value};

    #[test]
    fn numbers_remain_numbers() {
        let config = Config::load_json(
            r#"{"decimal":1.25,"big":18446744073709551616,"min":-9223372036854775808,"max":18446744073709551615,"negative_zero":-0,"items":[2.5,{"n":3e2}]}"#,
            "/",
        ).unwrap();
        assert_eq!(config.get_float("decimal"), Some(1.25));
        assert_eq!(config.get_float("big"), Some(18446744073709551616.0));
        assert_eq!(config.get_as::<i64>("min"), Some(i64::MIN));
        assert_eq!(config.get_as::<u64>("max"), Some(u64::MAX));
        assert!(config.get_float("negative_zero").unwrap().is_sign_negative());
        let items = config.get_as::<Vec<Value>>("items").unwrap();
        assert_eq!(items[0].as_f64(), Some(2.5));
        assert_eq!(items[1]["n"].as_f64(), Some(300.0));
        assert!(!config.outline().contains("$serde_json"));
    }

    #[test]
    fn objects_preserve_order_and_literal_marker_keys() {
        let config = Config::load_json(
            r#"{"z":1.25,"literal":{"$serde_json::private::Number":"1.25"},"a":2}"#, "/",
        ).unwrap();
        assert_eq!(config.outline(), "z: <number>\nliteral/$serde_json::private::Number: <string>\na: <number>\n");
        assert_eq!(config.str("literal/$serde_json::private::Number"), "1.25");
    }

    #[test]
    fn rejects_duplicate_keys_and_invalid_documents() {
        for input in [
            r#"{"a":1,"a":2}"#,
            r#"{"nested":{"a":1,"\u0061":2}}"#,
            r#"{"n":1e400}"#,
            r#"{"n":-1e400}"#,
            r#"{"n":1} trailing"#,
            r#"{"n":01}"#,
        ] {
            assert!(matches!(Config::load_json(input, "/"), Err(ConfigError::JsonError { .. })), "accepted {input}");
        }
    }

    #[test]
    fn scalar_roots_and_numeric_boundaries() {
        for input in ["null", "true", "\"text\"", "1.25", "[]", "{}"] {
            assert!(Config::load_json(input, "/").is_ok(), "rejected {input}");
        }
        let config = Config::load_json(
            r#"{"low":-9223372036854775809,"tiny":1e-400,"huge":1.7976931348623157e308,"fraction":1.0,"zero":-0.0}"#, "/",
        ).unwrap();
        assert!(matches!(config.get("low"), Some(Value::Number(n)) if n.is_f64()));
        assert_eq!(config.get_float("tiny"), Some(0.0));
        assert_eq!(config.get_float("huge"), Some(f64::MAX));
        assert!(matches!(config.get("fraction"), Some(Value::Number(n)) if n.is_f64()));
        assert!(config.get_float("zero").unwrap().is_sign_negative());
    }

    #[test]
    fn nesting_is_bounded() {
        let input = format!("{}0{}", "[".repeat(150), "]".repeat(150));
        assert!(matches!(Config::load_json(&input, "/"), Err(ConfigError::JsonError { .. })));
    }

    #[test]
    fn nested_errors_keep_document_line_numbers() {
        for (input, line, column) in [
            ("{\n  \"outer\": {\n    \"n\": 1e400\n  }\n}", 3, 14),
            ("{\n  \"outer\": {\n    \"n\": 1, \"n\": 2\n  }\n}", 3, 15),
            ("\n  {\"n\": 1e400}", 2, 13),
            ("\n  1e400", 2, 7),
        ] {
            let ConfigError::JsonError { source, .. } = Config::load_json(input, "/").unwrap_err() else {
                panic!("expected JSON error");
            };
            assert_eq!((source.line(), source.column()), (line, column), "{source}");
        }
    }
}
