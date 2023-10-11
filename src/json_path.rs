/// Dummy start of implementation of json_path
pub fn json_path(json_str: &str, search: &str) -> Option<serde_json::Value> {
    // if search is $, return the whole json
    if search == "$" {
        return serde_json::from_str(json_str).ok();
    }

    let json: serde_json::Value = serde_json::from_str(json_str).ok()?;
    search
        .split('.')
        .try_fold(&json, |json, token| {
            let (corrected_json, corrected_token) = if token.contains('[') {
                let split = token.split('[').collect::<Vec<&str>>();
                let (first_token, index_or_condition_type) = match &split[..] {
                    [first_token, index_or_condition_type] => {
                        (first_token, index_or_condition_type)
                    }
                    _ => panic!("Invalid token"),
                };
                let index_or_condition_type = index_or_condition_type.trim_end_matches(']');
                match json {
                    serde_json::Value::Object(map) => {
                        (map.get(*first_token), index_or_condition_type)
                    }
                    _ => (None, index_or_condition_type),
                }
            } else {
                (Some(json), token)
            };
            match corrected_json {
                Some(json) => match json {
                    serde_json::Value::Object(map) => map.get(corrected_token),
                    serde_json::Value::Array(array) => {
                        array.get(corrected_token.parse::<usize>().ok()?)
                    }
                    _ => None,
                },
                None => None,
            }
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jme_path() {
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
        let result = json_path(json, "a.b.c[0].d");
        assert_eq!(result, Some(serde_json::Value::String("e".to_string())));
    }
}
