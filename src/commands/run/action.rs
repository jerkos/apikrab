use crate::db::db_handler::DBHandler;
use crate::http;
use crate::utils::parse_multiple_conf;
use clap::Args;
use log::debug;
use std::collections::HashMap;

#[derive(Args)]
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
    force: bool,

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
                                let contains_acc = data_vec.iter().all(|s| !s.contains("{"));
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
    pub async fn run_action(
        &mut self,
        db_handler: &DBHandler,
        requester: &http::Api,
    ) -> anyhow::Result<()> {
        // creating a new context hashmap for storing extracted values
        let mut ctx: HashMap<String, String> = match db_handler.get_conf().await {
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
            let mut action = db_handler.get_action(action_name).await?;

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
                    let p = action_extract_path.as_str().split(":").collect::<Vec<_>>();
                    let v = p.get(1);
                    match v {
                        Some(path_name) => {
                            if ctx.contains_key(*path_name) && !self.force {
                                println!("Continuing !");
                                debug!("Continuing !");
                                continue;
                            }
                        }
                        _ => {}
                    }
                    Some(action_extract_path.to_string())
                }
            };

            let computed_path_params = match path_params.as_str() {
                "" => None,
                _ => {
                    let path_value_by_name = parse_multiple_conf(&path_params);
                    Some(path_value_by_name)
                }
            };
            requester
                .run_action(
                    &mut action,
                    &computed_path_params,
                    &computed_body,
                    db_handler,
                    &computed_extract_path,
                    self.no_print,
                    &mut ctx,
                )
                .await?;
        }

        if is_chained_action && self.save_as.is_some() {
            db_handler
                .upsert_flow(
                    self.save_as.as_ref().unwrap(),
                    &self.chain.as_ref().unwrap(),
                    &self.body,
                    &self.path_params,
                    &self.extract_path,
                )
                .await?;
        }

        Ok(())
    }
}
