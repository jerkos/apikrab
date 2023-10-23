use crate::commands::run::_http_result::HttpResult;
use crate::commands::run::_printer::Printer;
use crate::commands::run::_run_helper::{
    check_input, get_body, get_computed_urls, get_xtracted_path, is_anonymous_action,
};
use crate::db::db_handler::DBHandler;
use crate::db::dto::{Action, Context, History, Project};
use crate::http;
use crate::http::FetchResult;
use crate::utils::{
    format_query, get_full_url, get_str_as_interpolated_map, parse_cli_conf_to_map,
    parse_multiple_conf_as_opt_with_grouping_and_interpolation,
};
use anyhow::Error;
use clap::Args;
use crossterm::style::Stylize;
use futures::future;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

pub struct R {
    pub url: String,
    pub result: anyhow::Result<FetchResult>,
    pub ctx: HashMap<String, String>,
}

#[derive(Args, Serialize, Deserialize, Debug, Clone)]
pub struct RunActionArgs {
    /// action name
    pub(crate) name: Option<String>,

    #[arg(short, long, value_parser = ["GET", "POST", "PUT", "DELETE"])]
    pub(crate) verb: Option<String>,

    #[arg(short, long)]
    pub(crate) url: Option<String>,

    /// path params separated by a ,
    #[arg(short, long)]
    path_params: Option<Vec<String>>,

    /// query params separated by a ,
    #[arg(short, long)]
    query_params: Option<Vec<String>>,

    /// body of the action
    #[arg(short, long)]
    body: Option<Vec<String>>,

    /// optional headers
    #[arg(short = 'H', long)]
    headers: Option<Vec<String>>,

    /// extract path of the response
    #[arg(short, long)]
    pub extract_path: Option<Vec<String>>,

    /// chain with another action
    #[arg(short, long)]
    pub(crate) chain: Option<Vec<String>>,

    /// save command line as flow
    #[arg(long)]
    save_as: Option<String>,

    /// save result in the clipboard
    #[arg(long)]
    #[serde(default)]
    clipboard: bool,

    /// force action rerun even if its extracted value exists in current context
    #[arg(long)]
    pub(crate) force: bool,

    /// print the output of the command
    #[arg(long)]
    pub(crate) quiet: bool,

    /// grep the output of the command
    #[arg(long)]
    #[serde(default)]
    pub grep: bool,
}

impl RunActionArgs {
    fn init_progress_bars(step_count: u64) -> (MultiProgress, ProgressBar) {
        // init a multi bar to show progress
        let multi_bar = MultiProgress::new();

        let main_pb = multi_bar.add(
            ProgressBar::new(step_count).with_style(
                ProgressStyle::default_bar()
                    .template(
                        "{spinner} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
                    )
                    .unwrap(),
            ),
        );
        main_pb.enable_steady_tick(Duration::from_millis(100));

        (multi_bar, main_pb)
    }

    fn add_progress_bar_for_request(
        multi_bar: &MultiProgress,
        action_verb: &str,
        computed_url: &str,
        query_params: Option<&HashMap<String, String>>,
    ) -> ProgressBar {
        // creating a progress bar for the current request
        let pb = multi_bar.add(
            ProgressBar::new_spinner()
                .with_style(
                    ProgressStyle::with_template("{spinner:.blue} {msg}")
                        .unwrap()
                        .tick_strings(&["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"]),
                )
                .with_message(format!(
                    "Running {} ",
                    format_query(action_verb, computed_url, query_params)
                )),
        );
        pb.enable_steady_tick(Duration::from_millis(100));
        pb
    }

    /// finish progress bar
    pub fn finish_progress_bar(
        pb: &ProgressBar,
        fetch_result: Result<&FetchResult, &Error>,
        action_verb: &str,
        computed_url: &str,
        query_params: Option<&HashMap<String, String>>,
    ) {
        pb.finish_with_message(format!(
            "{}  {}",
            fetch_result
                .as_ref()
                .map(|r| {
                    let s = r.status.to_string();
                    if r.is_success() {
                        format!("{} ✅", s.green())
                    } else {
                        format!("{} ❌", s.red())
                    }
                })
                .unwrap_or("".to_string()),
            format_query(action_verb, computed_url, query_params)
        ));
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
        chain.insert(
            0,
            self.name
                .as_ref()
                .map(|n| n.to_string())
                .unwrap_or("UNKNOWN".to_string()),
        );

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

    async fn from_db(action_name: &str, db: &DBHandler) -> (Option<Action>, Option<Project>) {
        if is_anonymous_action(action_name) {
            (None, None)
        } else {
            let action = db.get_action(action_name).await.ok();
            let project_name = action
                .as_ref()
                .map(|a| a.project_name.as_str())
                .expect("Unknown project");

            let project = db.get_project(project_name).await.ok();
            (action, project)
        }
    }

    /// Main function for running an action
    pub async fn run_action<'a>(
        &'a mut self,
        http: &'a http::Api,
        db: &'a DBHandler,
    ) -> anyhow::Result<Vec<R>> {
        if let Err(msg) = check_input(
            self.name.as_deref(),
            self.verb.as_deref(),
            self.url.as_deref(),
        ) {
            eprintln!("{}", msg);
            anyhow::bail!("Invalid input");
        }

        // creating a new context hashmap for storing extracted values
        let ctx: HashMap<String, String> = match db.get_conf().await {
            Ok(ctx) => ctx.get_value(),
            Err(..) => HashMap::new(),
        };
        // check if action is chained
        self.prepare(self.chain.is_some())?;

        let (multi_bar, main_pb) =
            Self::init_progress_bars(self.chain.as_ref().map(|c| c.len()).unwrap() as u64);

        // create printer to print results
        let mut printer = Printer::new(self.quiet, self.clipboard, self.grep);

        // create a vector of results
        let mut action_results: Vec<R> = vec![];

        for (action_name, action_body, xtract_path, path_params, query_params) in itertools::izip!(
            self.chain.as_ref().expect("Internal error").iter(),
            self.body.as_ref().expect("Internal error").iter(),
            self.extract_path.as_ref().expect("Internal error").iter(),
            self.path_params.as_ref().expect("Internal error").iter(),
            self.query_params.as_ref().expect("Internal error").iter(),
        ) {
            // retrieve some information in the database
            let (action, project) = Self::from_db(action_name, db).await;

            // deals with configuration
            let mut xtended_ctx = project
                .as_ref()
                .map(|p| p.get_conf())
                .unwrap_or(HashMap::new());
            // update the configuration
            xtended_ctx.extend(ctx.clone().into_iter());

            // retrieve test url
            let main_project_url = project.as_ref().and_then(|p| p.test_url.as_deref());

            // all possible urls
            let action_url = action
                .as_ref()
                .map(|a| a.url.as_str())
                .unwrap_or_else(|| self.url.as_ref().unwrap());
            let computed_urls =
                get_computed_urls(path_params, main_project_url, action_url, &xtended_ctx);

            // compute body
            let body = get_body(
                action_body,
                action.as_ref().and_then(|a| a.static_body.as_deref()),
                action.as_ref().and_then(|a| a.body_example.as_deref()),
                &xtended_ctx,
            );

            // compute headers
            let computed_headers = get_str_as_interpolated_map(
                action.as_ref().map(|a| a.headers.as_str()).unwrap_or("{}"),
                &xtended_ctx,
            )
            .unwrap_or_else(|| {
                let headers_from_cli = parse_cli_conf_to_map(self.headers.as_ref());
                headers_from_cli
                    .into_iter()
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
                    .collect()
            });

            // all possible query params
            let computed_query_params = parse_multiple_conf_as_opt_with_grouping_and_interpolation(
                query_params,
                &xtended_ctx,
            );

            // run in concurrent mode
            let fetch_results = future::join_all(
                computed_urls
                    .iter()
                    .cartesian_product(computed_query_params)
                    .map(|(computed_url, query_params)| {
                        // clone needed variables
                        let extended_ctx = xtended_ctx.clone();
                        let mut action = action.clone();
                        let computed_headers = computed_headers.clone();
                        let body = body.clone();

                        let action_verb = action
                            .as_ref()
                            .map(|a| a.verb.clone())
                            .unwrap_or_else(|| self.verb.clone().unwrap());
                        // creating a progress bar for the current request
                        let pb = Self::add_progress_bar_for_request(
                            &multi_bar,
                            &action_verb,
                            computed_url,
                            query_params.as_ref(),
                        );

                        async move {
                            // fetch api
                            let fetch_result = http
                                .fetch(
                                    computed_url,
                                    &action_verb,
                                    &computed_headers,
                                    query_params.as_ref(),
                                    body.as_ref(),
                                )
                                .await;

                            // save history line
                            let fetch_result_ref = fetch_result.as_ref();
                            let _ = db
                                .insert_history(&History {
                                    id: None,
                                    action_name: action_name.to_string(),
                                    url: computed_url.to_string(),
                                    body: body.as_ref().map(|s| s.to_string()),
                                    headers: Some(
                                        serde_json::to_string(&computed_headers).unwrap(),
                                    ),
                                    response: fetch_result_ref.map(|r| r.response.clone()).ok(),
                                    status_code: fetch_result_ref.map(|r| r.status).unwrap_or(0u16),
                                    duration: fetch_result_ref
                                        .map(|r| r.duration.as_secs_f32())
                                        .unwrap_or(0f32),
                                    timestamp: None,
                                })
                                .await;

                            // upsert action if success
                            if let Ok(FetchResult {
                                response, status, ..
                            }) = &fetch_result
                            {
                                if let Some(action) = action.as_mut() {
                                    if *status >= 200 && *status < 300 {
                                        action.response_example = Some(response.clone());
                                        action.body_example = body.as_ref().map(|b| b.to_string());

                                        if db.upsert_action(action, true).await.is_err() {
                                            pb.println("Error inserting action");
                                        }
                                    }
                                }
                            }

                            // upsert ctx
                            if db
                                .insert_conf(&Context {
                                    value: serde_json::to_string(&extended_ctx)
                                        .expect("Error serializing context"),
                                })
                                .await
                                .is_err()
                            {
                                pb.println("Error inserting context");
                            }

                            // finish current pb
                            Self::finish_progress_bar(
                                &pb,
                                fetch_result.as_ref(),
                                &action_verb,
                                computed_url,
                                query_params.as_ref(),
                            );

                            // returning mixed of result etc...
                            R {
                                url: get_full_url(computed_url, query_params.as_ref()),
                                result: fetch_result,
                                ctx: extended_ctx,
                            }
                        }
                    }),
            )
            .await;

            let xtracted_path = get_xtracted_path(xtract_path, self.force, &xtended_ctx);

            for action_result in fetch_results {
                // handle result and print extracted data
                let _ = HttpResult::new(&action_result.result, &mut printer).handle_result(
                    xtracted_path.as_ref(),
                    &mut xtended_ctx,
                    &main_pb,
                );
                action_results.push(action_result);
            }

            // increment the main progress bar
            main_pb.inc(1);
        }

        // save as requested
        if self.save_as.is_some() {
            db.upsert_flow(self.save_as.as_ref().unwrap(), self, self.quiet)
                .await?;
        }

        // finishing progress bar
        main_pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {msg}")
                .unwrap(),
        );
        main_pb.finish_with_message(format!("{}", "Finished !".bold()));

        Ok(action_results)
    }
}
