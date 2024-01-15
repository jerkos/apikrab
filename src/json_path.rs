use std::{borrow::Cow, cmp::Ordering, collections::HashMap, ops::Deref, rc::Rc, str::FromStr};

use itertools::Itertools;
use serde_json::{Map, Value};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator};

const OBRACKET: &str = "[";
const CBRACKET: &str = "]";
const OPAREN: &str = "(";
const CPAREN: &str = ")";
const IPOINT: &str = "?";
const COLON: &str = ":";
const AROBASE: &str = "@";
const DOLLAR: &str = "$";
const DOT: &str = ".";
const COMMA: &str = ",";
const OBRACE: &str = "{";
const CBRACE: &str = "}";

#[derive(EnumString, Display, Debug, Clone, AsRefStr, PartialEq)]
pub enum JspToken {
    #[strum(serialize = "$")]
    Dollar,
    #[strum(serialize = "@")]
    Arobase,
    #[strum(serialize = "*")]
    Wild,
    #[strum(serialize = ":")]
    Colon,
    #[strum(serialize = ".")]
    Dot,
    #[strum(serialize = "")]
    Empty,
}

/// Comparison token, danger implementation is dependant of the order of the enum
/// variant declaration
#[derive(EnumString, EnumIter, Display, Debug, Clone, AsRefStr, PartialEq)]
pub enum CmpToken {
    #[strum(serialize = "==")]
    Eq,
    #[strum(serialize = "!=")]
    Neq,
    #[strum(serialize = ">=")]
    Gte,
    #[strum(serialize = ">")]
    Gt,
    #[strum(serialize = "<=")]
    Lte,
    #[strum(serialize = "<")]
    Lt,
}

// Enum of all implemented jmespath functions
#[derive(EnumString, EnumIter, Display, Debug, Clone, AsRefStr, PartialEq)]
pub enum Fn {
    #[strum(serialize = "sort")]
    Sort,
    #[strum(serialize = "join")]
    Join,
    #[strum(serialize = "length")]
    Length,
}

fn content_between<'a>(json_str: &'a str, start_char: &'a str, end_char: &'a str) -> &'a str {
    json_str
        .trim_start_matches(start_char)
        .trim_end_matches(end_char)
}

fn starts_with_arobase_or_dollar(json_str: &str) -> bool {
    json_str.starts_with(AROBASE) || json_str.starts_with(DOLLAR)
}

fn contains_comp_token(json_str: &str) -> Option<CmpToken> {
    let cmp_tokens = CmpToken::iter()
        .filter(|cmp_token| json_str.contains(cmp_token.as_ref()))
        .collect::<Vec<_>>();
    if cmp_tokens.is_empty() {
        return None;
    }
    return cmp_tokens.first().cloned();
}

fn left_and_right<'a>(json_str: &'a str, split_token: &'a str) -> (&'a str, &'a str) {
    let split = json_str
        .split(split_token)
        .map(|v| v.trim())
        .collect::<Vec<&'a str>>();
    match &split[..] {
        [left, right] => (left, right),
        _ => panic!("expected left and right expression"),
    }
}

fn extract_value_from_obj(
    root: &Value,
    current_value: &Value,
    values: &[JspExp],
    results: &mut Vec<Value>,
) {
    for jsexp_attribute in values {
        let result_opt = evaluate(root, current_value, jsexp_attribute);
        if let Some(result) = result_opt {
            results.push(result.into_owned());
        }
    }
}

fn build_value_from_map<'a>(
    root: &'a Value,
    current_value: &'a Value,
    map: &'a HashMap<String, JspExp>,
) -> Option<Cow<'a, Value>> {
    let mut results = vec![];
    extract_value_from_obj(
        root,
        current_value,
        &map.clone().into_values().collect_vec(),
        &mut results,
    );
    Some(Cow::Owned(Value::Object(Map::from_iter(
        map.iter()
            .enumerate()
            .map(|(i, (key, _))| (key.to_owned(), results.get(i).unwrap().clone())),
    ))))
}

#[derive(Debug, Clone, PartialEq)]
pub enum JspExp {
    //[1]
    Index(usize),

    //[0:2]
    IndexRange(usize, usize),

    // [0:2:1]
    //IndexRangeWithStep(usize, usize, usize),

    // @.price
    Value(JspToken, Vec<String>),

    //@.price >= 10
    CmpExpression(
        CmpToken,
        Box<crate::json_path::JspExp>,
        Box<crate::json_path::JspExp>,
    ),

    // sort(@.price)
    Fn(Fn, Vec<crate::json_path::JspExp>),

    // present in multiselect for example
    Attribute(String), // will be next Vec<Box<crate::json_path::JspExp>>),

    // name, name.age
    // will contains Attribute
    MultiSelect(Vec<crate::json_path::JspExp>),

    // {name: price, age: }
    MultSelectHash(HashMap<String, crate::json_path::JspExp>),
}

impl FromStr for JspExp {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with(OPAREN) {
            let content = content_between(s, OPAREN, CPAREN);
            Ok(content.parse::<JspExp>()?)
        } else if s.starts_with(OBRACKET) {
            // bracket expression
            let content = content_between(s, OBRACKET, CBRACKET);
            if content.starts_with(IPOINT) {
                Ok(content
                    .strip_prefix(IPOINT)
                    .expect("'?' awaited at beginning")
                    .parse::<JspExp>()?)
            } else if content.contains(COLON) {
                // todo handle 3 values
                let (start, end) = left_and_right(content, COLON);
                Ok(JspExp::IndexRange(
                    start.parse::<usize>()?,
                    end.parse::<usize>()?,
                ))
            } else if content.contains(COMMA) {
                let values = content
                    .split(COMMA)
                    .map(|s| s.trim().to_owned())
                    .map(JspExp::Attribute)
                    .collect_vec();
                Ok(JspExp::MultiSelect(values))
            } else {
                // return index
                Ok(JspExp::Index(content.parse::<usize>()?))
            }
        } else if s.starts_with(OBRACE) {
            let content = content_between(s, OBRACE, CBRACE);
            let values = content
                .split(COMMA)
                .map(|s| {
                    let mut split = s.trim().split(COLON);
                    (
                        split.next().expect("expecting key").trim().to_owned(),
                        JspExp::Attribute(split.next().expect("expecting value").trim().to_owned()),
                    )
                })
                .collect();
            Ok(JspExp::MultSelectHash(values))
        } else if let Some(cmp_token) = contains_comp_token(s) {
            // logical expression between left and right sides
            let (left, right) = left_and_right(s, cmp_token.as_ref());
            // return cmp expression
            Ok(JspExp::CmpExpression(
                cmp_token.clone(),
                Box::new(left.parse::<JspExp>()?),
                Box::new(right.parse::<JspExp>()?),
            ))
        } else if starts_with_arobase_or_dollar(s) {
            let mut values = s.split(DOT);
            let value = values.next();
            let attributes = values.map(String::from).collect::<Vec<String>>();
            match value {
                Some(AROBASE) => Ok(JspExp::Value(JspToken::Arobase, attributes)),
                Some(DOLLAR) => Ok(JspExp::Value(JspToken::Dollar, attributes)),
                None => Ok(JspExp::Value(JspToken::Empty, attributes)),
                _ => Err(anyhow::anyhow!("Invalid value, expecting @ or $")),
            }
        // parsing function
        } else if let Some(func) = Fn::iter()
            .filter(|fn_name| s.starts_with(fn_name.as_ref()))
            .collect::<Vec<_>>()
            .first()
        {
            let start = func.as_ref().to_owned() + OPAREN;
            let content = content_between(s, &start, CPAREN);
            Ok(JspExp::Fn(
                func.clone(),
                content
                    .split_terminator("',")
                    .filter(|v| !v.is_empty())
                    .map(|v| {
                        if v.trim().ends_with('\'') {
                            return ", ";
                        }
                        v.trim().trim_start_matches('\'')
                    })
                    .map(|s| s.parse::<JspExp>())
                    .collect::<Result<Vec<JspExp>, _>>()?,
            ))
        } else {
            Ok(JspExp::Value(JspToken::Empty, vec![s.to_string()]))
        }
    }
}

/// Evaluate a json path expression
fn evaluate<'a>(
    root: &'a Value,
    current_value: &'a Value,
    jsp_exp: &'a JspExp,
) -> Option<Cow<'a, Value>> {
    match jsp_exp {
        JspExp::Index(index) => {
            // got an index expression so awaiting an array
            // if not returning none
            match current_value {
                Value::Array(array) => array.get(*index).map(Cow::Borrowed),
                _ => None,
            }
        }
        JspExp::IndexRange(start, end) => {
            // same here if current value is not an array
            // returning early
            match current_value {
                Value::Array(array) => {
                    let r = &array[*start..*end];
                    Some(Cow::Owned(Value::Array(r.to_vec())))
                }
                _ => None,
            }
        }
        JspExp::Value(token, attributes) => {
            // if empty token and right value
            if token == &JspToken::Empty && attributes.len() == 1 {
                return Some(Cow::Owned(Value::String(
                    attributes[0]
                        .trim_end_matches('\'')
                        .trim_start_matches('\'')
                        .to_string(),
                )));
            }
            // if token is arobase or dollar, json value are used
            let target_value = match token {
                JspToken::Arobase => current_value,
                JspToken::Dollar => root,
                _ => panic!("Invalid token"),
            };
            let mut target_value_mut = target_value;
            for attribute in attributes {
                match current_value {
                    Value::Object(map) => {
                        target_value_mut = map.get(attribute)?;
                    }
                    _ => return None,
                }
            }
            Some(Cow::Borrowed(target_value_mut))
        }
        JspExp::Attribute(attributes) => {
            // if current value is an object, returning the attribute
            let mut target_value = current_value;
            for attribute in attributes.split(DOT) {
                if let Value::Object(map) = &target_value {
                    target_value = map.get(attribute)?;
                }
            }
            Some(Cow::Borrowed(target_value))
        }
        JspExp::MultiSelect(values) => {
            // copilot here
            let mut results: Vec<Value> = vec![];
            // iterate over current values but cloning it before
            match &current_value {
                Value::Object(_) => {
                    extract_value_from_obj(root, current_value, values, &mut results)
                }
                Value::Array(array) => {
                    for obj in array {
                        let mut partial_results = vec![];
                        extract_value_from_obj(root, obj, values, &mut partial_results);
                        results.push(Value::Array(partial_results));
                    }
                }
                _ => {}
            }
            // iterate over values, and create a vec of projected values
            Some(Cow::Owned(Value::Array(results)))
        }
        JspExp::MultSelectHash(ref map) => match &current_value {
            Value::Object(_) => build_value_from_map(root, current_value, map),
            Value::Array(array) => Some(Cow::Owned(Value::Array(
                array
                    .iter()
                    .filter_map(|v| build_value_from_map(root, v, map).map(|v| v.into_owned()))
                    .collect_vec(),
            ))),
            _ => None,
        },
        JspExp::CmpExpression(cmp_token, left, right) => match current_value {
            Value::Array(array) => {
                let filtered =
                    array
                        .iter()
                        .filter(|v| {
                            let left_opt = evaluate(root, v, left);
                            let right_opt = evaluate(root, v, right);

                            if let (None, None) | (Some(_), None) | (None, Some(_)) =
                                (&left_opt, &right_opt)
                            {
                                return false;
                            }
                            let (left, right) =
                                (left_opt.as_ref().unwrap(), right_opt.as_ref().unwrap());

                            match (left, right) {
                                (
                                    Cow::Borrowed(Value::Number(left)),
                                    Cow::Borrowed(Value::Number(right)),
                                )
                                | (
                                    Cow::Borrowed(Value::Number(left)),
                                    Cow::Owned(Value::Number(right)),
                                )
                                | (
                                    Cow::Owned(Value::Number(left)),
                                    Cow::Borrowed(Value::Number(right)),
                                )
                                | (
                                    Cow::Owned(Value::Number(left)),
                                    Cow::Owned(Value::Number(right)),
                                ) => {
                                    let left = left.as_f64().expect("failed to parse f64");
                                    let right = right.as_f64().expect("failed to parse f64");
                                    match cmp_token {
                                        CmpToken::Eq => left == right,
                                        CmpToken::Neq => left != right,
                                        CmpToken::Gte => left >= right,
                                        CmpToken::Gt => left > right,
                                        CmpToken::Lte => left <= right,
                                        CmpToken::Lt => left < right,
                                    }
                                }
                                (
                                    Cow::Borrowed(Value::String(left)),
                                    Cow::Borrowed(Value::String(right)),
                                )
                                | (
                                    Cow::Borrowed(Value::String(left)),
                                    Cow::Owned(Value::String(right)),
                                )
                                | (
                                    Cow::Owned(Value::String(left)),
                                    Cow::Borrowed(Value::String(right)),
                                )
                                | (
                                    Cow::Owned(Value::String(left)),
                                    Cow::Owned(Value::String(right)),
                                ) => match cmp_token {
                                    CmpToken::Eq => left == right,
                                    CmpToken::Neq => left != right,
                                    CmpToken::Gte => left >= right,
                                    CmpToken::Gt => left > right,
                                    CmpToken::Lte => left <= right,
                                    CmpToken::Lt => left < right,
                                },
                                _ => false,
                            }
                        })
                        .cloned()
                        .collect_vec();
                Some(Cow::Owned(Value::Array(filtered)))
            }
            _ => None,
        },
        // handle function
        JspExp::Fn(fn_name, values) => match fn_name {
            Fn::Join => match current_value {
                // join values of the array
                Value::Array(array) => {
                    println!("values: {:?}", values[0]);
                    match &values[0] {
                        JspExp::Value(_, v) => Some(Cow::Owned(Value::String(
                            array
                                .iter()
                                .map(|v| match v {
                                    Value::String(s) => s.clone(),
                                    Value::Number(n) => n.to_string(),
                                    _ => "".to_owned(),
                                })
                                .collect_vec()
                                .join(v[0].trim_end_matches('\'').trim_start_matches('\'')),
                        ))),
                        _ => None,
                    }
                }
                _ => None,
            },
            Fn::Length => match current_value {
                Value::Array(array) => Some(Cow::Owned(Value::Number(
                    serde_json::Number::from_f64(array.len() as f64).unwrap(),
                ))),
                _ => None,
            },
            Fn::Sort => {
                match &values[0] {
                    JspExp::Value(_, _) => {}
                    _ => return None,
                };
                match current_value {
                    Value::Array(array) => {
                        let mut cloned = array.clone();
                        cloned.sort_by(|a, b| {
                            let left_opt = evaluate(root, a, &values[0]);
                            let right_opt = evaluate(root, b, &values[0]);
                            match (left_opt.as_ref(), right_opt.as_ref()) {
                                (None, None) => Ordering::Equal,
                                (Some(_), None) => Ordering::Greater,
                                (None, Some(_)) => Ordering::Less,
                                (
                                    Some(Cow::Borrowed(Value::Number(left))),
                                    Some(Cow::Borrowed(Value::Number(right))),
                                ) => {
                                    let left = left.as_f64().expect("failed to parse f64");
                                    let right = right.as_f64().expect("failed to parse f64");
                                    left.partial_cmp(&right).unwrap()
                                }
                                (
                                    Some(Cow::Borrowed(Value::String(left))),
                                    Some(Cow::Borrowed(Value::String(right))),
                                ) => left.as_str().partial_cmp(right.as_str()).unwrap(),
                                // do not know how to compare ?
                                _ => std::cmp::Ordering::Equal,
                            }
                        });
                        Some(Cow::Owned(Value::Array(cloned)))
                    }
                    _ => None,
                }
            }
        },
    }
}

fn get_name_and_jsexp(value: &str, ending_str: Option<&str>) -> (String, Option<JspExp>) {
    match ending_str {
        None => {
            if Fn::iter().any(|fn_name| value.starts_with(fn_name.as_ref())) {
                return ("".to_string(), Some(value.parse::<JspExp>().ok().unwrap()));
            }
            (value.to_string(), None)
        }
        Some(brace_or_bracket) => {
            let mut split = value.split(brace_or_bracket);
            let name = split.next().expect("name expected or empty string");
            let jsexp = split
                .next()
                .map(|v| format!("{}{}", brace_or_bracket, v))
                .expect("jsexp expected");
            (name.to_string(), jsexp.parse::<JspExp>().ok())
        }
    }
}

/// parse x.a[?(@.price <= 10)].z
/// ugly function !
pub fn parse_input_js_path(json_path: &str) -> Vec<(String, Option<JspExp>)> {
    let mut results = vec![];
    let mut prev_value = "".to_string();

    for current_value in json_path.split(DOT) {
        let bracket_started = current_value.contains(OBRACKET);
        let bracket_finished = current_value.ends_with(CBRACKET);

        let brace_started = current_value.contains(OBRACE);
        let brace_finished = current_value.ends_with(CBRACE);

        let bracket_started_not_finished = bracket_started && !bracket_finished;
        let brace_started_not_finished = brace_started && !brace_finished;

        if bracket_started_not_finished || brace_started_not_finished {
            prev_value = current_value.to_string();
            continue;
        }
        // if prev value is not empty and bracket or brace is not finished
        if !(prev_value.is_empty() || bracket_finished || brace_finished) {
            prev_value = format!("{}.{}", prev_value, current_value);
            continue;
        }

        let value = if prev_value.is_empty() {
            current_value.to_string()
        } else {
            format!("{}.{}", prev_value, current_value)
        };

        results.push(get_name_and_jsexp(
            &value,
            if brace_finished {
                Some(OBRACE)
            } else if bracket_finished {
                Some(OBRACKET)
            } else {
                None
            },
        ));

        if bracket_finished || brace_finished {
            prev_value = "".to_string();
        }
    }
    results
}

pub fn json_path<'a>(json_str: &'a str, search_str: &'a str) -> Option<Value> {
    let dollar = JspToken::Dollar.as_ref();
    if search_str == dollar {
        return serde_json::from_str(json_str).ok();
    }

    let dollar_plus_dot = format!("{}.", dollar);
    let search = search_str.strip_prefix(&dollar_plus_dot);

    if search.is_none() {
        eprintln!("Invalid search: {:?}", search_str);
        return None;
    }

    let search = search.unwrap();

    // parse json string as json value using serde_json
    let json: Rc<Value> = Rc::new(serde_json::from_str(json_str).ok()?);

    // cloning the Rc creates a new pointer to the same value
    let mut result = Rc::clone(&json);

    // tokenize search
    let tokenized = parse_input_js_path(search);

    // iterate over tokenization
    for (name, jsexp) in &tokenized {
        // retrieving the current json value
        let mut result_mut = Rc::make_mut(&mut result);

        // updating the current value
        let current_value: Option<&Value> = if !name.is_empty() {
            match result_mut {
                &mut Value::Object(ref map) => map.get(name.as_str()),
                ref mut value_array => {
                    match value_array.as_array_mut() {
                        Some(ref mut array) => {
                            // retain only values with name
                            array.retain(|v| v.get(name.as_str()).is_some());
                            array.iter_mut().for_each(|v| {
                                let target_value = v.get(name.as_str()).unwrap().clone();
                                *v = target_value;
                            });

                            Some(value_array)
                        }
                        _ => {
                            println!("No value found");
                            return None;
                        }
                    }
                }
            }
        } else {
            Some(result_mut)
        };

        // if current value is none, returning early
        // else evaluating the jsexp which returns a cow json value
        result = match current_value {
            None => return None,
            Some(value) => match jsexp.as_ref() {
                Some(exp) => evaluate(&json, value, exp)?.into_owned().into(),
                None => value.clone().into(),
            },
        };
    }
    Some(result.deref().clone())
}
