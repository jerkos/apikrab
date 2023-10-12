use crate::utils::{
    parse_multiple_conf, parse_multiple_conf_as_opt_with_grouping_and_interpolation,
    parse_multiple_conf_with_opt, replace_with_conf,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

static ANONYMOUS_ACTION: &str = "UNKNOWN";

/// Check if action is anonymous
pub fn is_anonymous_action(action_name: &str) -> bool {
    action_name == ANONYMOUS_ACTION
}

/// Check arguments, could be probably done with clap
/// but I'm too lazy to do it
pub(crate) fn check_input(
    name: Option<&str>,
    verb: Option<&str>,
    url: Option<&str>,
) -> anyhow::Result<()> {
    let tuple = (verb, url);
    match name {
        None => match tuple {
            (Some(_), Some(_)) => Ok(()),
            _ => anyhow::bail!("Verb and url are required"),
        },
        Some(_) => match tuple {
            (Some(_), Some(_)) => {
                anyhow::bail!("Verb and url are provided but got an action as input")
            }
            _ => Ok(()),
        },
    }
}

pub enum BodyType<'bt> {
    Static(&'bt str),
    LastSuccessfulBody(&'bt str),
    Empty,
    Default(&'bt str),
}

impl<'bt> From<&'bt str> for BodyType<'bt> {
    fn from(s: &'bt str) -> BodyType<'bt> {
        match s {
            "" => BodyType::Empty,
            "LAST_SUCCESSFUL_BODY" => BodyType::LastSuccessfulBody(s),
            "STATIC" => BodyType::Static(s),
            _ => BodyType::Default(s),
        }
    }
}

/// private method for getting body
fn _get_body<'a>(str: &str, interpolated_body: Cow<'a, str>, body: &str) -> Option<Cow<'a, str>> {
    if str.contains('{') {
        return Some(interpolated_body);
    }
    let body_as_map = parse_multiple_conf(body);
    Some(Cow::Owned(
        serde_json::to_string(&body_as_map).ok().unwrap(),
    ))
}

/// Body interpolation
/// Some magical values exists e.g. LAST_SUCCESSFUL_BODY, STATIC
pub fn get_body<'a>(
    body: &'a str,
    static_body: Option<&'a str>,
    body_example: Option<&'a str>,
    ctx: &HashMap<String, String>,
) -> Option<Cow<'a, str>> {
    let interpolated_body = replace_with_conf(body, ctx);

    match &interpolated_body {
        // if static body exists, use it otherwise None is used
        Cow::Borrowed(str) => {
            let body_type: BodyType = (*str).into();
            match body_type {
                BodyType::Empty => None,
                BodyType::Static(_) => Some(Cow::Borrowed(static_body.as_ref()?)),
                BodyType::LastSuccessfulBody(_) => Some(Cow::Borrowed(body_example.as_ref()?)),
                BodyType::Default(str) => _get_body(str, interpolated_body.clone(), body),
            }
        }
        // body had some interpolated value
        Cow::Owned(body_value) => _get_body(body_value, interpolated_body.clone(), body),
    }
}

/// Compute several url given path params
/// using the cartesian product of all values
/// te generate all possible urls
pub fn get_computed_urls(
    path_params: &str,
    project_url: Option<&str>,
    action_url: &str,
    ctx: &HashMap<String, String>,
) -> HashSet<String> {
    let all_path_params =
        parse_multiple_conf_as_opt_with_grouping_and_interpolation(path_params, ctx);

    let full_url = match project_url {
        Some(main_url) => format!("{}/{}", main_url, action_url),
        None => action_url.to_string(),
    };

    all_path_params
        .iter()
        .map(|params| {
            let mut url = full_url.clone();
            if let Some(params) = params {
                for (k, v) in params.iter() {
                    url = url.replace(format!("{{{}}}", k).as_str(), v);
                }
            }
            url
        })
        .collect()
}

/// Extract path interpolation
/// Special cases that the name may or may not be present
pub fn get_xtracted_path<'a>(
    extracted_path: &'a str,
    force: bool,
    ctx: &HashMap<String, String>,
) -> Option<HashMap<&'a str, Option<&'a str>>> {
    if extracted_path.is_empty() {
        return None;
    }
    let value = parse_multiple_conf_with_opt(extracted_path);

    let all_values = value
        .values()
        .filter_map(|v| *v)
        .map(|v| ctx.contains_key(v))
        .collect::<Vec<_>>();

    let needs_to_continue = all_values.iter().all(|v| *v);
    if !all_values.is_empty() && needs_to_continue && !force {
        println!("Some extracted values already in ctx. Skipping ! \n Use force to rerun");
        return None;
    }
    Some(value)
}
