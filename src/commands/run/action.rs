use crate::http;
use crate::utils::{parse_multiple_conf, parse_multiple_conf_as_opt};
use clap::Args;
use log::debug;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Args, Serialize, Deserialize, Debug, Clone)]
pub struct RunActionArgs {
    // action name
    name: String,

    // path params separated by a ,
    #[arg(short, long)]
    path_params: Option<Vec<String>>,

    // query params separated by a ,
    #[arg(short, long)]
    query_params: Option<Vec<String>>,

    // body of the action
    #[arg(short, long)]
    body: Option<Vec<String>>,

    // extract path of the response
    #[arg(short, long)]
    extract_path: Option<Vec<String>>,

    // chain with another action
    #[arg(short, long)]
    chain: Option<Vec<String>>,

    // save command line as flow
    #[arg(long)]
    save_as: Option<String>,

    // force action rerun even if its extracted value exists in current context
    #[arg(long)]
    pub force: bool,

    // print the output of the command
    #[arg(long)]
    no_print: bool,
}

impl RunActionArgs {
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
                                let contains_acc = data_vec.iter().any(|s| s.contains("{"));
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
    pub async fn run_action<'a>(&'a mut self, requester: &'a http::Api<'_>) -> anyhow::Result<()> {
        let cloned = self.clone();
        // creating a new context hashmap for storing extracted values
        let mut ctx: HashMap<String, String> = match requester.db_handler.get_conf().await {
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

        for ((((action_name, action_body), action_extract_path), path_params), query_params) in
            zipped
        {
            let mut action = requester.db_handler.get_action(action_name).await?;

            let computed_body = match action_body.as_str() {
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
                    // transform the body
                    serde_json::from_str::<HashMap<String, String>>(action_body)
                        .ok()
                        .map(|_| action_body.to_string())
                        .or_else(|| {
                            let as_map = parse_multiple_conf(&action_body);
                            Some(serde_json::to_string(&as_map).unwrap())
                        })
                }
            };

            let computed_extract_path = match action_extract_path.as_str() {
                "" => None,
                _ => {
                    let value = action_extract_path
                        .as_str()
                        .split(",")
                        .map(|s| {
                            let mut split = s.split(":");
                            (
                                split.next().unwrap().to_string(),
                                split.next().map(String::from),
                            )
                        })
                        .collect::<HashMap<_, _>>();

                    let all_values = value
                        .values()
                        .filter_map(|v| v.as_ref())
                        .map(|v| ctx.contains_key(v))
                        .collect::<Vec<_>>();

                    let needs_to_continue = all_values.iter().all(|v| *v);
                    if !all_values.is_empty() && needs_to_continue && !self.force {
                        println!("Continuing !");
                        debug!("Continuing !");
                        continue;
                    }
                    Some(value)
                }
            };

            let computed_path_params = parse_multiple_conf_as_opt(path_params);
            let computed_query_params = parse_multiple_conf_as_opt(query_params);

            requester
                .run_action(
                    &mut action,
                    &computed_path_params,
                    &computed_query_params,
                    &computed_body,
                    &computed_extract_path,
                    self.no_print,
                    &mut ctx,
                )
                .await?;
        }

        // save as requested
        if self.save_as.is_some() {
            requester
                .db_handler
                .upsert_flow(
                    self.save_as.as_ref().unwrap(),
                    &cloned,
                )
                .await?;
        }

        Ok(())
    }
}
