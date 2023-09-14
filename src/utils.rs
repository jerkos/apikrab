use rand::*;
use std::collections::HashMap;

pub fn replace_with_conf(str: &str, conf: &HashMap<String, String>) -> String {
    conf.iter().fold(str.to_string(), |acc, (k, v)| {
        acc.replace(format!("{{{k}}}", k = k).as_str(), v)
    })
}

/// Parse a conf string to a hashmap
pub fn parse_conf_to_map(conf: &Option<Vec<String>>) -> HashMap<String, String> {
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

pub fn random_emoji() -> char {
    let x: u32 = thread_rng().gen_range(0x1F600..0x1F64F);
    char::from_u32(x).unwrap_or('💔')
}
