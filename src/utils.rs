use crate::db::dto::Action;
use crossterm::style::Stylize;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use rand::*;
use std::borrow::Cow;
use std::collections::HashMap;

pub fn replace_with_conf<'a>(str: &'a str, conf: &HashMap<String, String>) -> Cow<'a, str> {
    if !str.contains('{') {
        return str.into();
    }
    let interpolated = conf.iter().fold(str.to_string(), |acc, (k, v)| {
        acc.replace(format!("{{{k}}}", k = k).as_str(), v)
    });
    interpolated.into()
}

/// Parse a conf string to a hashmap
pub fn parse_cli_conf_to_map(conf: Option<&Vec<String>>) -> HashMap<&str, &str> {
    match conf {
        Some(conf) => conf
            .iter()
            .map(|s| s.split(':').collect::<Vec<_>>())
            .map(|v| (v[0], v[1]))
            .collect(),

        None => HashMap::new(),
    }
}

pub fn parse_multiple_conf(conf: &str) -> HashMap<&str, &str> {
    if !conf.contains('{') {
        return conf
            .split(',')
            .map(|s| {
                let mut split = s.split(':');
                (split.next().unwrap(), split.next().unwrap())
            })
            .collect::<HashMap<_, _>>();
    }
    serde_json::from_str(conf).expect("Error deserializing conf")
}

/// only for extracted path
pub fn parse_multiple_conf_with_opt(conf: &str) -> HashMap<&str, Option<&str>> {
    conf.split(',')
        .map(|s| {
            let mut split = s.split(':');
            (split.next().unwrap(), split.next())
        })
        .collect::<HashMap<_, _>>()
}

pub fn parse_multiple_conf_as_opt(conf: &str) -> Option<HashMap<&str, &str>> {
    match conf {
        "" => None,
        _ => {
            let path_value_by_name = parse_multiple_conf(conf);
            Some(path_value_by_name)
        }
    }
}

pub fn get_str_as_interpolated_map<'a>(
    data: &'a str,
    ctx: &HashMap<String, String>,
) -> Option<HashMap<Cow<'a, str>, Cow<'a, str>>> {
    let interpolated = replace_with_conf(data, ctx);
    match interpolated {
        Cow::Borrowed(str) => {
            let p = parse_multiple_conf_as_opt(str);
            p.map(|p| {
                p.iter()
                    .map(|(k, v)| ((*k).into(), (*v).into()))
                    .collect::<HashMap<_, _>>()
            })
        }
        Cow::Owned(str) => {
            let p = parse_multiple_conf_as_opt(&str);
            p.map(|p| {
                p.iter()
                    .map(|(k, v)| ((*k).to_string().into(), (*v).to_string().into()))
                    .collect::<HashMap<_, _>>()
            })
        }
    }
}

pub fn _parse_multiple_conf_as_opt_with_grouping(
    str: Cow<str>,
) -> HashMap<Cow<str>, Vec<Cow<str>>> {
    match str {
        Cow::Borrowed(str) => str
            .split(',')
            .map(|str| {
                let mut split = str.split(':');
                (
                    Cow::Borrowed(split.next().expect("key not found")),
                    split
                        .next()
                        .expect("value not found")
                        .split('|')
                        .map(Cow::Borrowed)
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
            .collect(),
        Cow::Owned(str) => str
            .split(',')
            .map(|str| {
                let mut split = str.split(':');
                (
                    Cow::Owned::<str>(split.next().expect("key not found").to_string()),
                    split
                        .next()
                        .expect("value not found")
                        .split('|')
                        .map(|v| Cow::Owned(v.to_string()))
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
            .collect(),
    }
}

pub fn parse_multiple_conf_as_opt_with_grouping(
    conf: Cow<str>,
) -> Option<HashMap<Cow<str>, Vec<Cow<str>>>> {
    if conf.is_empty() || conf.contains('{') {
        return None;
    }

    let value = _parse_multiple_conf_as_opt_with_grouping(conf);

    Some(value)
}

pub fn parse_multiple_conf_as_opt_with_grouping_and_interpolation<'a>(
    conf: &'a str,
    ctx: &HashMap<String, String>,
) -> Vec<Option<HashMap<Cow<'a, str>, Cow<'a, str>>>> {
    let p = replace_with_conf(conf, ctx);
    let parsed_conf = parse_multiple_conf_as_opt_with_grouping(p);
    match parsed_conf {
        None => vec![None],
        Some(possible_values_by_key) => possible_values_by_key
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
                    possible_values_by_key
                        .iter()
                        .zip(v.iter())
                        .map(|((k, _), v)| (k.clone(), v.clone()))
                        .collect(),
                )
            })
            .collect(),
    }
}

/// Generate a random emoji from the unicode range
/// which is then incorporated in a project name
pub fn random_emoji() -> char {
    let x: u32 = thread_rng().gen_range(0x1F600..0x1F64F);
    char::from_u32(x).unwrap_or('💔')
}

/// Create an undefined spinner
/// Should enable tick after its creation
/// to be used in a multi progress bar
pub fn spinner(message: Option<&str>) -> ProgressBar {
    let p = ProgressBar::new_spinner();
    p.set_style(
        ProgressStyle::with_template("{spinner:.blue} {msg}")
            .unwrap()
            // For more spinners check out the cli-spinners project:
            // https://github.com/sindresorhus/cli-spinners/blob/master/spinners.json
            .tick_strings(
                &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"],
            ),
    );
    p.set_message(
        message
            .map(String::from)
            .unwrap_or("Loading...".to_string()),
    );
    p
}

/// Format a query given an action, an url and query params
pub fn get_full_url(
    url: &str,
    query_params: Option<&HashMap<Cow<'_, str>, Cow<'_, str>>>,
) -> String {
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
    action: &Action,
    computed_url: &str,
    query_params: Option<&HashMap<Cow<'_, str>, Cow<'_, str>>>,
) -> String {
    format!(
        "{} {}",
        action.verb.clone().yellow(),
        get_full_url(computed_url, query_params).green()
    )
}
