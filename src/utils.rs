use crossterm::style::Stylize;
use itertools::Itertools;
use rand::*;
use std::borrow::Cow;
use std::collections::HashMap;

/// a string is interpolated if it contains {{ and }}
pub fn contains_interpolation(str: &str) -> bool {
    str.contains("{{") && str.contains("}}")
}

/// returning a cow here to avoid cloning the string
/// body is supposed to be a json string and therefore
/// can be quite long
pub fn replace_with_conf<'a>(str: &'a str, conf: &HashMap<String, String>) -> Cow<'a, str> {
    if !contains_interpolation(str) {
        return Cow::Borrowed(str);
    }
    let interpolated = conf.iter().fold(str.to_string(), |acc, (k, v)| {
        acc.replace(format!("{{{{{k}}}}}", k = k).as_str(), v)
    });
    Cow::Owned(interpolated)
}

/// Parse a configuration key: str, val: str from a vec of str to a hashmap
/// Used to parse cli commands
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

/// Parse a configuration key: str, val: str from a vec of str to a hashmap
/// conf can be json or values separated by comma
pub fn _parse_multiple_conf<'a, T, F>(conf: &'a str, func: F) -> HashMap<&'a str, T>
where
    F: Fn(Option<&'a str>) -> T,
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
                    (split.next().unwrap(), func(split.next()))
                })
                .collect::<HashMap<_, _>>()
        })
}

pub fn parse_multiple_conf<'a>(conf: &'a str) -> HashMap<&'a str, &'a str> {
    let closure = |v: Option<&'a str>| v.unwrap();
    _parse_multiple_conf(conf, closure)
}

/// only for extracted path
/// it is not json for sure
pub fn parse_multiple_conf_with_opt<'a>(conf: &'a str) -> HashMap<&'a str, Option<&'a str>> {
    let closure = |v: Option<&'a str>| v;
    _parse_multiple_conf(conf, closure)
}

pub fn get_str_as_interpolated_map(
    data: &str,
    ctx: &HashMap<String, String>,
) -> Option<HashMap<String, String>> {
    if data.is_empty() {
        return None;
    }
    let interpolated = replace_with_conf(data, ctx);

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
) -> Vec<Option<HashMap<String, String>>> {
    let p = replace_with_conf(conf, ctx);
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
        let interpolated = replace_with_conf(str, &conf);
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
