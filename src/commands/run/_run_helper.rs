use crate::utils::{
    parse_multiple_conf, parse_multiple_conf_as_opt_with_grouping_and_interpolation,
    parse_multiple_conf_with_opt, replace_with_conf, Interpol,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use super::action::RunActionArgs;

static ANONYMOUS_ACTION: &str = "UNKNOWN";

/// Check if action is anonymous
pub fn is_anonymous_action(action_name: &str) -> bool {
    action_name == ANONYMOUS_ACTION
}

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
    match run_action_args.name.as_ref() {
        None => match tuple {
            (Some(_), Some(_)) => {}
            _ => err.push("Verb and url are required"),
        },
        _ => {}
    }
    if !err.is_empty() {
        anyhow::bail!(err.join("\n"));
    }
    Ok(())
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
    let interpolated_body = replace_with_conf(body, ctx, Interpol::MultiInterpol);

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
    let full_url = match project_url {
        Some(main_url) => format!("{}/{}", main_url, action_url),
        None => action_url.to_string(),
    };

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

/// Merge cli args with db args
/// Usually db args are overridden by cli args
///
/// ```rust
/// let mut run_action_args = RunActionArgs::default();
/// let o = RunActionArgs {
///    verb: Some("GET".to_string()),
///   body: Some("".to_string()),
/// };
/// merge_with(&mut run_action_args, &o);
/// ```
pub fn merge_with(run_actions_args: &RunActionArgs, o: &RunActionArgs) -> RunActionArgs {
    let mut clone = run_actions_args.clone();
    if o.verb.is_some() {
        clone.verb = o.verb.clone();
    }
    if o.body.is_some() {
        clone.body = o.body.clone();
    }
    if o.path_params.is_some() {
        clone.path_params = o.path_params.clone();
    }
    if o.query_params.is_some() {
        clone.query_params = o.query_params.clone();
    }
    if o.header.is_some() {
        clone.header = o.header.clone();
    }

    if o.chain.is_some() {
        clone.chain = o.chain.clone();
        /*
        clone.chain = clone.chain.zip(o.chain.clone())
        .map(|(mut v1, mut v2)| {v1.append(&mut v2); v1});
        */
    }

    if o.name.is_some() {
        clone.name = o.name.clone();
    }
    if o.extract_path.is_some() {
        clone.extract_path = o.extract_path.clone();
    }
    if o.url.is_some() {
        clone.url = o.url.clone();
    }
    if o.expect.is_some() {
        clone.expect = o.expect.clone();
    }
    if o.form_data {
        clone.form_data = o.form_data;
    }
    if o.url_encoded {
        clone.url_encoded = o.url_encoded;
    }
    clone
}
