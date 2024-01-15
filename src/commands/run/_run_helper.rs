use crossterm::style::Stylize;

use crate::utils::{
    parse_multiple_conf, parse_multiple_conf_as_opt_with_grouping_and_interpolation,
    parse_multiple_conf_with_opt, replace_with_conf, Interpol,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use super::action::RunActionArgs;

pub static ANONYMOUS_ACTION: &str = "UNKNOWN";

/// Check arguments, could be probably done with clap
/// but I'm too lazy to do it
pub(crate) fn check_input(run_action_args: &RunActionArgs) -> anyhow::Result<()> {
    let mut err = vec![];
    if run_action_args.save_to_ts.is_some() && run_action_args.expect.is_none() {
        err.push("Expect is required when saving to test suite");
    }
    if run_action_args.save.is_some() && run_action_args.save_to_ts.is_some() {
        err.push("Cannot save to flow and test suite at the same time");
    }
    let tuple = (run_action_args.verb.as_ref(), run_action_args.url.as_ref());
    if run_action_args.name.as_ref().is_none() {
        match tuple {
            (Some(_), Some(_)) => {}
            _ => err.push("Verb and url are required"),
        }
    }
    if !err.is_empty() {
        anyhow::bail!(err.join("\n").dark_red());
    }
    Ok(())
}

pub enum BodyType<'bt> {
    Empty,
    Default(&'bt str),
}

impl<'bt> From<&'bt str> for BodyType<'bt> {
    fn from(s: &'bt str) -> BodyType<'bt> {
        match s {
            "" => BodyType::Empty,
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
pub fn get_body<'a>(body: &'a str, ctx: &HashMap<String, String>) -> Option<Cow<'a, str>> {
    let interpolated_body = replace_with_conf(body, ctx, Interpol::MultiInterpol);

    match &interpolated_body {
        // if static body exists, use it otherwise None is used
        Cow::Borrowed(body_as_str) => {
            let body_type: BodyType = (*body_as_str).into();
            match body_type {
                BodyType::Empty => None,
                BodyType::Default(default_body_as_str) => {
                    _get_body(default_body_as_str, interpolated_body.clone(), body)
                }
            }
        }
        // body had some interpolated value
        Cow::Owned(body_value) => _get_body(body_value, interpolated_body.clone(), body),
    }
}

/// complete an url with http if not present
/// if requested url starts with http or https, do nothing
/// if requested url starts with :, add http://localhost
/// otherwise add https://
/// ```rust
/// assert_eq!(complete_url("http://localhost:8080"), "http://localhost:8080");
/// assert_eq!(complete_url(":8080"), "https://localhost:8080");
/// ```
fn complete_url(url: &str) -> Cow<str> {
    // if url starts with http, do nothing
    if url.starts_with("http") {
        return Cow::Borrowed(url);
    }
    if url.starts_with(':') {
        Cow::Owned(format!("http://localhost{}", url))
    } else {
        Cow::Owned(format!("https://{}", url))
    }
}

fn get_full_url<'a>(project_url: Option<&'a str>, action_url: &'a str) -> Cow<'a, str> {
    match project_url {
        Some(main_url) => {
            if main_url.is_empty() {
                return complete_url(action_url);
            }
            Cow::Owned(format!("{}/{}", complete_url(main_url), action_url))
        }
        None => complete_url(action_url),
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
    if action_url.is_empty() {
        return HashSet::new();
    }

    let full_url = get_full_url(project_url, action_url).into_owned();

    // returning url with no interpolation
    // to be checked later
    if path_params.is_empty() {
        return HashSet::from([full_url]);
    }

    let all_path_params = parse_multiple_conf_as_opt_with_grouping_and_interpolation(
        path_params,
        ctx,
        Interpol::SimpleInterpol,
    );

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
pub fn get_xtracted_path(
    extracted_path: &str,
    force: bool,
    ctx: &HashMap<String, String>,
) -> Option<HashMap<String, Option<String>>> {
    if extracted_path.is_empty() {
        return None;
    }
    let value = parse_multiple_conf_with_opt(extracted_path);

    let all_values = value
        .values()
        .filter_map(|v| v.as_ref())
        .map(|v| ctx.contains_key(v))
        .collect::<Vec<_>>();

    let needs_to_continue = all_values.iter().all(|v| *v);
    if !all_values.is_empty() && needs_to_continue && !force {
        println!("Some extracted values already in ctx. Skipping ! \n Use force to rerun");
        return None;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_url() {
        assert_eq!(
            complete_url("http://localhost:8080"),
            "http://localhost:8080"
        );
        assert_eq!(complete_url(":8080"), "http://localhost:8080");
        assert_eq!(complete_url("google.com"), "https://google.com");
    }

    #[test]
    fn test_get_full_url() {
        assert_eq!(
            get_full_url(Some("http://localhost:8080"), "test"),
            "http://localhost:8080/test"
        );
        assert_eq!(get_full_url(None, ":8080"), "http://localhost:8080");
    }
}
