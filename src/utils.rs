use crossterm::style::Stylize;
use itertools::Itertools;
use rand::*;
use std::borrow::Cow;
use std::collections::HashMap;

const SINGLE_INTERPOL_START: char = '{';
const SINGLE_INTERPOL_END: char = '}';
const MULTI_INTERPOL_START: &str = "{{";
const MULTI_INTERPOL_END: &str = "}}";

#[derive(Debug, Clone, Copy)]
pub enum Interpol {
    MultiInterpol,
    SimpleInterpol,
}

/// a string is interpolated if it contains {{ and }}
pub fn contains_interpolation(str: &str, interpol: Interpol) -> bool {
    match interpol {
        Interpol::MultiInterpol => {
            str.contains(MULTI_INTERPOL_START) && str.contains(MULTI_INTERPOL_END)
        }
        Interpol::SimpleInterpol => {
            str.contains(SINGLE_INTERPOL_START) && str.contains(SINGLE_INTERPOL_END)
        }
    }
}

/// check if a map contains interpolation
pub fn map_contains_interpolation(map: &HashMap<String, String>, interpol: Interpol) -> bool {
    map.values().any(|v| contains_interpolation(v, interpol))
}

/// returning a cow here to avoid cloning the string
/// body is supposed to be a json string and therefore
/// can be quite long
pub fn replace_with_conf<'a>(
    str: &'a str,
    conf: &HashMap<String, String>,
    interpol: Interpol,
) -> Cow<'a, str> {
    if !contains_interpolation(str, interpol) {
        return Cow::Borrowed(str);
    }
    let interpolated = conf.iter().fold(str.to_string(), |acc, (k, v)| {
        acc.replace(format!("{{{{{k}}}}}", k = k).as_str(), v)
    });
    Cow::Owned(interpolated)
}

/// Parse a configuration key: str, val: str from a vec of str to a hashmap
/// Used to parse cli commands
pub fn parse_cli_conf_to_map(conf: Option<&Vec<String>>) -> Option<HashMap<String, String>> {
    match conf {
        Some(conf) => Some(
            conf.iter()
                .map(|s| s.split(':').collect::<Vec<_>>())
                .map(|v| (v[0].to_string(), v[1..].join(":")))
                .collect(),
        ),
        None => None,
    }
}

/// Parse a configuration key: str, val: str from a vec of str to a hashmap
/// conf can be json or values separated by comma
pub fn _parse_multiple_conf<'a, T, F>(conf: &'a str, func: F) -> HashMap<String, T>
where
    F: Fn(Option<String>) -> T,
    T: serde::de::Deserialize<'a>,
{
    // try parse json string first
    // if it fails then parse comma separated values
    serde_json::from_str(conf)
        .map_err(|e| anyhow::anyhow!(e))
        .unwrap_or_else(|_| {
            conf.split(',')
                .map(|s| {
                    let mut split = s.split(':');
                    (
                        split.next().unwrap().to_string(),
                        func(Some(split.map(|v| v.to_string()).collect::<Vec<String>>().join(":"))), //split.next().map(|v| v.to_string())),
                    )
                })
                .collect::<HashMap<_, _>>()
        })
}

pub fn parse_multiple_conf<'a>(conf: &'a str) -> HashMap<String, String> {
    let closure = |v: Option<String>| v.unwrap();
    _parse_multiple_conf(conf, closure)
}

/// only for extracted path
/// it is not json for sure
pub fn parse_multiple_conf_with_opt<'a>(conf: &'a str) -> HashMap<String, Option<String>> {
    let closure = |v: Option<String>| v;
    _parse_multiple_conf(conf, closure)
}

pub fn get_str_as_interpolated_map(
    data: &str,
    ctx: &HashMap<String, String>,
    interpol: Interpol,
) -> Option<HashMap<String, String>> {
    if data.is_empty() {
        return None;
    }
    let interpolated = replace_with_conf(data, ctx, interpol);

    Some(
        parse_multiple_conf(&interpolated)
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    )
}

fn _parse_multiple_conf_as_opt_with_grouping(str: Cow<str>) -> HashMap<String, Vec<String>> {
    str.split(',')
        .map(|str| {
            let mut split = str.split(':');
            (
                split.next().expect("key not found").to_string(),
                split
                    .next()
                    .expect("value not found")
                    .split('|')
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>(),
            )
        })
        .group_by(|(k, _)| k.clone())
        .into_iter()
        .map(|(k, group)| {
            (
                k,
                group.flat_map(|(_, values)| values.into_iter()).collect(),
            )
        })
        .collect()
}

/// For query params and path params
pub fn parse_multiple_conf_as_opt_with_grouping_and_interpolation(
    conf: &str,
    ctx: &HashMap<String, String>,
    interpol: Interpol,
) -> Vec<Option<HashMap<String, String>>> {
    let p = replace_with_conf(conf, ctx, interpol);
    if p.is_empty() {
        return vec![None];
    }
    let parsed_conf = _parse_multiple_conf_as_opt_with_grouping(p);
    if parsed_conf.is_empty() {
        return vec![None];
    }
    parsed_conf
        .values()
        .fold(vec![], |acc, values| {
            if acc.is_empty() {
                return values.iter().map(|v| vec![v.clone()]).collect::<Vec<_>>();
            }
            let z = acc.into_iter().cartesian_product(values);
            z.map(|(mut acc, value)| {
                acc.push(value.clone());
                acc
            })
            .collect::<Vec<_>>()
        })
        .iter()
        .map(|v| {
            Some(
                parsed_conf
                    .iter()
                    .zip(v.iter())
                    .map(|((k, _), v)| (k.clone(), v.clone()))
                    .collect(),
            )
        })
        .collect()
}


/// For query params and path params
/// if the value is empty then we try to get the value from the current
/// run_action_args object
pub fn val_or_join<'a>(val: &'a str, opt: Option<&Vec<String>>) -> Cow<'a, str> {
    if !val.is_empty() {
        return Cow::Borrowed(val);
    }
        match opt.as_ref() {
            Some(h) => Cow::Owned(h.iter().filter(|v| !v.is_empty()).join(",")),
            None => Cow::Owned("".to_string()),
        }
    }

/// Generate a random emoji from the unicode range
/// which is then incorporated in a project name
pub fn random_emoji() -> char {
    let x: u32 = thread_rng().gen_range(0x1F600..0x1F64F);
    char::from_u32(x).unwrap_or('💔')
}

/// Format a query given an action, an url and query params
pub fn get_full_url(url: &str, query_params: Option<&HashMap<String, String>>) -> String {
    match query_params {
        Some(query_params) => {
            let query_params_as_str = query_params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<String>>()
                .join("&");
            format!("{}?{}", url, query_params_as_str)
        }
        None => url.to_string(),
    }
}

/// Format a query given an action, an url and query params
pub fn format_query(
    verb: &str,
    computed_url: &str,
    query_params: Option<&HashMap<String, String>>,
) -> String {
    format!(
        "{} {}",
        verb.yellow(),
        get_full_url(computed_url, query_params).green()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_with_conf() {
        let conf = vec![("a".to_string(), "1".to_string())]
            .into_iter()
            .collect::<HashMap<_, _>>();
        let str = "a:{{a}}";
        let interpolated = replace_with_conf(str, &conf, Interpol::MultiInterpol);
        assert_eq!(interpolated, "a:1");
    }

    #[test]
    fn test_parse_multiple_conf_as_opt_with_grouping() {
        let conf = "a:1|2|3,b:4|5|6";
        let parsed = _parse_multiple_conf_as_opt_with_grouping(conf.into());
        assert_eq!(parsed.get("a").unwrap(), &vec!["1", "2", "3"]);
        assert_eq!(parsed.get("b").unwrap(), &vec!["4", "5", "6"]);
    }
}
