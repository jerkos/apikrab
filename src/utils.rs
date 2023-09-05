use std::collections::HashMap;

pub fn replace_with_conf(str: &str, conf: &HashMap<String, String>) -> String {
    conf.iter().fold(str.to_string(), |acc, (k, v)| {
        acc.replace(format!("{{{k}}}", k = k).as_str(), v)
    })
}

pub fn deserialize<'a, T: serde::Deserialize<'a>>(data: &'a str) -> T {
    serde_json::from_str::<T>(data).expect("Invalid configuration")
}

/// Parse a conf string to a hashmap
pub fn parse_conf_to_map(conf: &Option<Vec<String>>) -> HashMap<String, String> {
    match conf {
        Some(conf) => conf
            .iter()
            .map(|s| s.split(":").collect::<Vec<_>>())
            .map(|v| (v[0].to_string(), v[1].to_string()))
            .collect(),

        None => HashMap::new(),
    }
}

pub fn parse_multiple_conf(conf: &str) -> HashMap<String, String> {
    conf.split(',')
        .map(|s| {
            let mut split = s.split(":");
            (
                split.next().unwrap().to_string(),
                split.next().unwrap().to_string(),
            )
        })
        .collect::<HashMap<_, _>>()
}

pub fn parse_multiple_conf_as_opt(conf: &str) -> Option<HashMap<String, String>> {
    match conf {
        "" => None,
        _ => {
            let path_value_by_name = parse_multiple_conf(&conf);
            Some(path_value_by_name)
        }
    }
}
