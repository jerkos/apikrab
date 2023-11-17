use std::{borrow::Cow, str::FromStr};

use itertools::Itertools;
use serde_json::Value;
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

#[derive(Debug, Clone, PartialEq)]
pub enum JspExp {
    //[1]
    Index(usize),

    //[0:2]
    IndexRange(usize, usize),

    // @.price
    Value(JspToken, Vec<String>),

    //@.price >= 10
    CmpExpression(
        CmpToken,
        Box<crate::json_path::JspExp>,
        Box<crate::json_path::JspExp>,
    ),
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
                let (start, end) = left_and_right(content, COLON);
                Ok(JspExp::IndexRange(
                    start.parse::<usize>()?,
                    end.parse::<usize>()?,
                ))
            } else {
                // return index
                Ok(JspExp::Index(content.parse::<usize>()?))
            }
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
            let target_value = match token {
                JspToken::Arobase | JspToken::Empty => current_value,
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
        JspExp::CmpExpression(cmp_token, left, right) => match current_value {
            Value::Array(array) => {
                let filtered = array
                    .into_iter()
                    .filter(|v| {
                        let left_opt = evaluate(root, v, left);
                        let right_opt = evaluate(root, v, right);

                        match (left_opt, right_opt) {
                            (None, None) | (Some(_), None) | (None, Some(_)) => return false,

                            (
                                Some(Cow::Borrowed(Value::Number(left))),
                                Some(Cow::Borrowed(Value::Number(right))),
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
                                Some(Cow::Borrowed(Value::String(left))),
                                Some(Cow::Borrowed(Value::String(right))),
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
                    .map(|v| v.clone())
                    .collect_vec();
                Some(Cow::Owned(Value::Array(filtered)))
            }
            _ => None,
        },
    }
}

fn get_name_and_jsexp(value: &str) -> (String, Option<JspExp>) {
    if !value.contains(OBRACKET) && !value.contains(CBRACKET) {
        return (value.to_string(), None);
    }
    let mut split = value.split(OBRACKET);
    let name = split.next().expect("name expected");
    let jsexp = split
        .next()
        .map(|v| format!("{}{}", OBRACKET, v))
        .expect("jsexp expected");
    (name.to_string(), jsexp.parse::<JspExp>().ok())
}

/// parse x.a[?(@.price <= 10)].z
/// ugly function !
pub fn parse_input_js_path(json_path: &str) -> Vec<(String, Option<JspExp>)> {
    let mut results = vec![];
    let mut prev_value = "".to_string();

    for current_value in json_path.split(DOT) {
        let start_bracket_but_not_finish =
            current_value.contains(OBRACKET) && !current_value.contains(CBRACKET);
        let finish_bracket = current_value.ends_with(CBRACKET);

        if start_bracket_but_not_finish {
            prev_value = current_value.to_string();
            continue;
        }

        if !prev_value.is_empty() && !finish_bracket {
            prev_value = format!("{}.{}", prev_value, current_value.to_string());
            continue;
        }

        let value = if prev_value.is_empty() {
            current_value.to_string()
        } else {
            format!("{}.{}", prev_value, current_value.to_string())
        };
        results.push(get_name_and_jsexp(&value));

        if finish_bracket {
            prev_value = "".to_string();
        }
    }
    results
}

pub fn json_path<'a>(json_str: &'a str, search: &'a str) -> Option<Value> {
    let dollar = JspToken::Dollar.as_ref();
    if search == dollar {
        return serde_json::from_str(json_str).ok();
    }

    let dollar_plus_dot = format!("{}.", dollar);
    let search = search.strip_prefix(&dollar_plus_dot);

    if search.is_none() {
        eprint!("Invalid search: {}", search.unwrap());
        return None;
    }

    let search = search.unwrap();

    // parse json string as json value using serde_json
    let json: Value = serde_json::from_str(json_str).ok()?;
    let mut result = json.clone();
    let tokenized = parse_input_js_path(search);
    for (name, jsexp) in &tokenized {
        let current_value = result.get(name.as_str());
        result = match current_value {
            None => return None,
            Some(value) => match jsexp.as_ref() {
                Some(exp) => evaluate(&json, &value, exp)?.into_owned(),
                None => value.clone(),
            },
        };
    }
    Some(result)
}
