use itertools::Itertools;
use rand::*;
use std::collections::HashMap;

pub fn replace_with_conf(str: &str, conf: &HashMap<String, String>) -> String {
    conf.iter().fold(str.to_string(), |acc, (k, v)| {
        acc.replace(format!("{{{k}}}", k = k).as_str(), v)
    })
}

/// Parse a conf string to a hashmap
pub fn parse_cli_conf_to_map(conf: &Option<Vec<String>>) -> HashMap<String, String> {
    match conf {
        Some(conf) => conf
            .iter()
            .map(|s| s.split(':').collect::<Vec<_>>())
            .map(|v| (v[0].to_string(), v[1].to_string()))
            .collect(),

        None => HashMap::new(),
    }
}

pub fn parse_multiple_conf(conf: &str) -> HashMap<String, String> {
    if !conf.contains('{') {
        return conf
            .split(',')
            .map(|s| {
                let mut split = s.split(':');
                (
                    split.next().unwrap().to_string(),
                    split.next().unwrap().to_string(),
                )
            })
            .collect::<HashMap<_, _>>();
    }
    serde_json::from_str(conf).expect("Error deserializing conf")
}

/// only for extracted path
pub fn parse_multiple_conf_with_opt(conf: &str) -> HashMap<String, Option<String>> {
    conf.split(',')
        .map(|s| {
            let mut split = s.split(':');
            (
                split.next().unwrap().to_string(),
                split.next().map(String::from),
            )
        })
        .collect::<HashMap<_, _>>()
}

pub fn parse_multiple_conf_as_opt(conf: &str) -> Option<HashMap<String, String>> {
    match conf {
        "" => None,
        _ => {
            let path_value_by_name = parse_multiple_conf(conf);
            Some(path_value_by_name)
        }
    }
}

pub fn get_str_as_interpolated_map(
    data: &str,
    ctx: &HashMap<String, String>,
) -> Option<HashMap<String, String>> {
    parse_multiple_conf_as_opt(&replace_with_conf(data, ctx))
}

pub fn parse_multiple_conf_as_opt_with_grouping(
    conf: &str,
) -> Option<HashMap<String, Vec<String>>> {
    match conf {
        "" => None,
        _ => {
            if conf.contains('{') {
                println!("json is not supported for grouping");
                return None;
            }
            let value: HashMap<String, Vec<String>> = conf
                .split(',')
                .map(|s| {
                    let mut split = s.split(':');
                    let key = split.next().expect("key not found");
                    let value = split.next().expect("value not found");
                    let split: Vec<String> = value.split('|').map(String::from).collect();
                    (key, split)
                })
                .group_by(|(k, _)| k.to_string())
                .into_iter()
                .map(|(k, group)| {
                    let values: Vec<String> =
                        group.flat_map(|(_, values)| values.into_iter()).collect();
                    (k, values)
                })
                .collect();

            Some(value)
        }
    }
}

pub fn parse_multiple_conf_as_opt_with_grouping_and_interpolation(
    conf: &str,
    ctx: &HashMap<String, String>,
) -> Vec<Option<HashMap<String, String>>> {
    let parsed_conf = parse_multiple_conf_as_opt_with_grouping(&replace_with_conf(conf, ctx));
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
