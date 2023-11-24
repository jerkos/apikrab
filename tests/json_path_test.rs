use std::str::FromStr;

use apikrab::json_path::{json_path, parse_input_js_path, CmpToken, JspExp, JspToken};
use itertools::Itertools;
use serde_json::{Map, Value};

#[test]
fn test_from_str() {
    let result = JspExp::from_str("[?(@.price >= 10)]").unwrap();
    println!("{:?}", result);
    assert_eq!(
        result,
        JspExp::CmpExpression(
            CmpToken::Gte,
            Box::new(JspExp::Value(JspToken::Arobase, vec!["price".to_string()])),
            Box::new(JspExp::Value(JspToken::Empty, vec!["10".to_string()]))
        )
    );
}

#[test]
fn test_from_str_dollar() {
    let result = JspExp::from_str("[?($.price >= @.price)]").unwrap();
    println!("{:?}", result);
    assert_eq!(
        result,
        JspExp::CmpExpression(
            CmpToken::Gte,
            Box::new(JspExp::Value(JspToken::Dollar, vec!["price".to_string()])),
            Box::new(JspExp::Value(JspToken::Arobase, vec!["price".to_string()]))
        )
    );
}

#[test]
fn test_from_str_index_range() {
    let result = JspExp::from_str("[1:10]").unwrap();
    println!("{:?}", result);
    assert_eq!(result, JspExp::IndexRange(1, 10));
}

#[test]
fn test_from_str_index() {
    let result = JspExp::from_str("[10]").unwrap();
    println!("{:?}", result);
    assert_eq!(result, JspExp::Index(10usize));
}

#[test]
fn test_parse_input_js_path() {
    let json_path = "x.a[?(@.price <= 10)].z";
    let r = parse_input_js_path(json_path);
    let result = r.iter().map(|(name, _)| name).collect_vec();
    println!("{:?}", result);
    assert_eq!(result, vec!["x", "a", "z"]);
}

#[test]
fn test_parse_input_js_path_2() {
    let json_path = "x.a.b[?(@.price <= 10)].z[0].y[1:10].vv.bb[?(true)].zz[?($.p == 123.12)]";
    let r = parse_input_js_path(json_path);
    assert_eq!(
        r,
        vec![
            ("x".to_string(), None),
            ("a".to_string(), None),
            (
                "b".to_string(),
                Some(JspExp::CmpExpression(
                    CmpToken::Lte,
                    Box::new(JspExp::Value(JspToken::Arobase, vec!["price".to_string()])),
                    Box::new(JspExp::Value(JspToken::Empty, vec!["10".to_string()]))
                ))
            ),
            ("z".to_string(), Some(JspExp::Index(0))),
            ("y".to_string(), Some(JspExp::IndexRange(1, 10))),
            ("vv".to_string(), None),
            (
                "bb".to_string(),
                Some(JspExp::Value(JspToken::Empty, vec!["true".to_string()]))
            ),
            (
                "zz".to_string(),
                Some(JspExp::CmpExpression(
                    CmpToken::Eq,
                    Box::new(JspExp::Value(JspToken::Dollar, vec!["p".to_string()])),
                    Box::new(JspExp::Value(JspToken::Empty, vec!["123.12".to_string()]))
                ))
            ),
        ]
    );
}

#[test]
fn test_jme_path_bis() {
    let json = r#"
        {
            "a": {
                "b": {
                    "c": [
                        {
                            "d": "e"
                        }
                    ]
                }
            }
        }
        "#;
    let result = json_path(json, "$.a.b.c[0].d");
    assert_eq!(result, Some(Value::String("e".to_string())));
}

#[test]
fn test_machines() {
    let json = r#"
    {
        "machines": [
            {"name": "a", "state": "running"},
            {"name": "b", "state": "stopped"},
            {"name": "c", "state": "running"}
        ]
    }"#;
    let result = json_path(json, "$.machines[?(@.state == 'running')].name");
    println!("{:?}", result);
    assert_eq!(
        result,
        Some(Value::Array(vec![
            Value::String("a".to_string()),
            Value::String("c".to_string())
        ]))
    );
}

#[test]
fn test_machines_multiselect() {
    let json = r#"
    {
        "machines": [
            {"name": "a", "state": "running"},
            {"name": "b", "state": "stopped"},
            {"name": "c", "state": "running"}
        ]
    }"#;
    let result = json_path(json, "$.machines[?(@.state == 'running')].[name, state]");
    println!("{:?}", result);
    assert_eq!(
        result,
        Some(Value::Array(vec![
            Value::Array(vec![
                Value::String("a".to_string()),
                Value::String("running".to_string())
            ]),
            Value::Array(vec![
                Value::String("c".to_string()),
                Value::String("running".to_string())
            ]),
        ]))
    );
}

#[test]
fn test_machines_multiselect_object() {
    let json = r#"{"name": "a", "state": "running"}"#;
    let result = json_path(json, "$.[name, state]");
    println!("{:?}", result);
    assert_eq!(
        result,
        Some(Value::Array(vec![
            Value::String("a".to_string()),
            Value::String("running".to_string())
        ]),)
    );
}

#[test]
fn test_machines_multiselect_hash() {
    let json = r#"{"name": "a", "state": "running"}"#;
    let result = json_path(json, "$.{Name:name, Value:state}");
    println!("{:?}", result);
    assert_eq!(
        result,
        Some(Value::Object(Map::from_iter(vec![
            ("Name".to_string(), Value::String("a".to_string())),
            ("Value".to_string(), Value::String("running".to_string()))
        ]))),
    );
}
