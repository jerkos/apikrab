use crate::commands::run::_http_result::HttpResult;
use crate::commands::run::_printer::Printer;
use crate::db::dto::Action;
use crate::http;
use crate::http::FetchResult;
use crate::utils::{
    get_str_as_interpolated_map, parse_multiple_conf, parse_multiple_conf_as_opt,
    parse_multiple_conf_with_opt, replace_with_conf,
};
use clap::Args;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct R {
    pub result: anyhow::Result<FetchResult>,
    pub ctx: HashMap<String, String>,
}

#[derive(Args, Serialize, Deserialize, Debug, Clone)]
pub struct RunActionArgs {
    /// action name
    name: String,

    /// path params separated by a ,
    #[arg(short, long)]
    path_params: Option<Vec<String>>,

    /// query params separated by a ,
    #[arg(short, long)]
    query_params: Option<Vec<String>>,

    /// body of the action
    #[arg(short, long)]
    body: Option<Vec<String>>,

    /// extract path of the response
    #[arg(short, long)]
    extract_path: Option<Vec<String>>,

    /// chain with another action
    #[arg(short, long)]
    chain: Option<Vec<String>>,

    /// save command line as flow
    #[arg(long)]
    save_as: Option<String>,

    #[arg(long)]
    clipboard: bool,

    /// force action rerun even if its extracted value exists in current context
    #[arg(long)]
    pub force: bool,

    /// print the output of the command
    #[arg(long)]
    pub no_print: bool,

    /// grep the output of the command
    #[arg(long)]
    pub grep: bool,
}

impl RunActionArgs {
    pub fn get_computed_body(
        &self,
        body: &str,
        action: &Action,
        ctx: &HashMap<String, String>,
    ) -> Option<String> {
        let body_as_map = match body {
            // if static body exists, use it otherwise None is used
            "" => action.static_body.as_ref().map(|s| s.to_string()),
            "LAST_SUCCESSFUL_BODY" => {
                let last_successful_body = action
                    .body_example
                    .as_ref()
                    .expect("No last successful body found!");
                Some(last_successful_body.to_string())
            }
            _ => {
                // todo handle other deserialization values than hashmap
                serde_json::from_str::<HashMap<String, String>>(body)
                    .ok()
                    .map(|_| body.to_string())
                    .or_else(|| {
                        let as_map = parse_multiple_conf(body);
                        Some(serde_json::to_string(&as_map).unwrap())
                    })
            }
        };
        body_as_map
            .as_ref()
            .map(|body| replace_with_conf(body, ctx))
            .or(Some(body.to_owned()))
    }

    pub fn get_computed_url(
        path_params: &str,
        project_url: &str,
        action_url: &str,
        ctx: &HashMap<String, String>,
    ) -> String {
        let path_params_as_map = parse_multiple_conf_as_opt(path_params);
        let full_url = format!("{}/{}", project_url, action_url);
        path_params_as_map
            .as_ref()
            .map(|path_params| replace_with_conf(&full_url, path_params))
            .map(|full_url| replace_with_conf(&full_url, ctx))
            .unwrap_or_else(|| replace_with_conf(&full_url, ctx))
    }

    pub fn get_computed_extracted_path(
        &self,
        extracted_path: &str,
        ctx: &HashMap<String, String>,
    ) -> Option<HashMap<String, Option<String>>> {
        match extracted_path {
            "" => None,
            _ => {
                let value = parse_multiple_conf_with_opt(extracted_path);

                let all_values = value
                    .values()
                    .filter_map(|v| v.as_ref())
                    .map(|v| ctx.contains_key(v))
                    .collect::<Vec<_>>();

                let needs_to_continue = all_values.iter().all(|v| *v);
                if !all_values.is_empty() && needs_to_continue && !self.force {
                    println!(
                        "Some extracted values already in ctx. Skipping ! \n Use force to rerun"
                    );
                    return None;
                }
                Some(value)
            }
        }
    }

    ///
    /// Prepare all data for running an action
    /// if chain is given, we need to have the same length for all parameters
    /// e.g. action -b "{}"  -p id:1,job_id:2 -e $:VARIABLE -c action2 -b "{}"
    /// will be transformed to
    /// body: ["{}", "{}"]
    /// path_params: ["id:1", "job_id:2"]
    ///
    ///
    /// In case of single action, we can write
    /// action -b key:value  -b key2:value2 -p id:1  -p job_id:2 -e r.0:VARIABLE -e r.1:VARIABLE2
    ///
    fn prepare(&mut self, is_chained_cmd: bool) -> anyhow::Result<()> {
        let chain_len = self.chain.as_mut().map(|v| v.len() + 1).unwrap_or(1);

        let mut data = HashMap::from([
            ("body", &mut self.body),
            ("extracted_path", &mut self.extract_path),
            ("chain", &mut self.chain),
            ("path_param", &mut self.path_params),
            ("query_param", &mut self.query_params),
        ]);
        // unwrap data as mutable
        let mut unwrapped_data = data
            .iter_mut()
            .map(|(k, v)| {
                match v {
                    Some(data_vec) => {
                        if !is_chained_cmd {
                            if data_vec.len() > 1 {
                                let contains_acc = data_vec.iter().any(|s| s.contains('{'));
                                if contains_acc {
                                    anyhow::bail!(
                                        "Chain, body and extract path must have the same length"
                                    );
                                }
                            }
                            let merged_data = data_vec.join(",");
                            **v = Some(vec![merged_data]);
                        }
                    }
                    None => {
                        let mut empty_vec = vec![];
                        if k != &"chain" {
                            while empty_vec.len() < chain_len {
                                empty_vec.push("".to_string());
                            }
                        }
                        **v = Some(empty_vec);
                    }
                }
                Ok((k, v.as_mut().unwrap()))
            })
            .filter_map(|v| v.is_ok().then(|| v.unwrap()))
            .collect::<HashMap<_, _>>();

        let chain = unwrapped_data.get_mut(&"chain").unwrap();
        chain.insert(0, self.name.to_string());

        // check for chain action we do have same length for parameters
        if chain_len > 0 {
            let is_valid = unwrapped_data
                .values()
                .filter(|v| !v.is_empty())
                .map(|v| v.len())
                .all(|len| len == chain_len);
            if !is_valid {
                anyhow::bail!("Chain, body and extract path must have the same length");
            }
        }

        Ok(())
    }

    /// Main function for running an action
    pub async fn run_action<'a>(&'a mut self, http: &'a http::Api<'_>) -> anyhow::Result<Vec<R>> {
        let cloned = self.clone();
        // creating a new context hashmap for storing extracted values
        let ctx: HashMap<String, String> = match http.db_handler.get_conf().await {
            Ok(ctx) => ctx.get_value(),
            Err(..) => HashMap::new(),
        };
        // check if action is chained
        let is_chained_action = self.chain.is_some();

        // prepare all data
        self.prepare(is_chained_action)?;

        // run all actions given data
        let zipped = self
            .chain
            .as_ref()
            .expect("Intern error")
            .iter()
            .zip(self.body.as_ref().expect("Intern error").iter())
            .zip(self.extract_path.as_ref().expect("Intern error").iter())
            .zip(self.path_params.as_ref().expect("Intern error").iter())
            .zip(self.query_params.as_ref().expect("Intern error").iter())
            .collect::<Vec<_>>();

        let mut action_results: Vec<R> = vec![];
        for ((((action_name, action_body), action_extract_path), path_params), query_params) in
            zipped
        {
            let mut action = http.db_handler.get_action(action_name).await?;
            let project = http.db_handler.get_project(&action.project_name).await?;
            let mut extended_ctx = project.get_conf();
            extended_ctx.extend(ctx.iter().map(|(k, v)| (k.clone(), v.clone())));

            let computed_body = self.get_computed_body(action_body, &action, &extended_ctx);
            let computed_extract_path =
                self.get_computed_extracted_path(action_extract_path, &extended_ctx);
            let computed_query_params = get_str_as_interpolated_map(query_params, &extended_ctx);
            let computed_headers = get_str_as_interpolated_map(&action.headers, &extended_ctx)
                .unwrap_or(HashMap::new());
            let computed_url = Self::get_computed_url(
                path_params,
                &project.test_url.expect("Unknown URL"),
                &action.url,
                &extended_ctx,
            );

            let mut printer = Printer::new(self.no_print, self.clipboard, self.grep);

            let result = http
                .fetch(
                    action_name,
                    &computed_url,
                    &action.verb,
                    &computed_headers,
                    &computed_query_params,
                    &computed_body,
                    &printer,
                )
                .await;

            let mut result_handler = HttpResult::new(http.db_handler, &result, &mut printer);

            // handle result
            result_handler
                .handle_result(
                    &mut action,
                    &computed_body,
                    &computed_extract_path,
                    &mut extended_ctx,
                )
                .await?;

            action_results.push(R {
                result,
                ctx: extended_ctx,
            });
        }

        // save as requested
        if self.save_as.is_some() {
            http.db_handler
                .upsert_flow(self.save_as.as_ref().unwrap(), &cloned)
                .await?;
        }

        Ok(action_results)
    }
}
