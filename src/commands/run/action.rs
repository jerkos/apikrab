use crate::commands::run::_http_result::HttpResult;
use crate::commands::run::_printer::Printer;
use crate::commands::run::_run_helper::check_input;
use crate::db::db_handler::DBHandler;
use crate::db::dto::{Action, TestSuiteInstance};
use crate::domain::DomainAction;
use crate::http;
use crate::http::FetchResult;
use crate::utils::parse_cli_conf_to_map;
use clap::Args;
use core::panic;
use crossterm::style::Stylize;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::to_string;
use std::collections::HashMap;
use std::time::Duration;

use super::_progress_bar::init_progress_bars;
use super::_run_helper::merge_with;
use super::_test_checker::TestChecker;

pub struct R {
    pub url: String,
    pub result: anyhow::Result<FetchResult>,
    pub ctx: HashMap<String, String>,
}

#[derive(Args, Serialize, Deserialize, Debug, Clone, Default)]
pub struct RunActionArgs {
    /// action name optional
    pub(crate) name: Option<String>,

    #[arg(short, long)]
    pub(crate) url: Option<String>,

    #[arg(short, long, value_parser = ["GET", "POST", "PUT", "DELETE"])]
    pub(crate) verb: Option<String>,

    /// path params separated by a ,
    #[arg(short, long)]
    pub(crate) path_params: Option<Vec<String>>,

    /// query params separated by a ,
    #[arg(short, long)]
    pub(crate) query_params: Option<Vec<String>>,

    /// body of the action
    #[arg(short, long)]
    pub(crate) body: Option<Vec<String>>,

    /// multipart form data
    #[arg(short = 'H', long)]
    pub(crate) header: Option<Vec<String>>,

    /// url encoded body
    #[arg(long)]
    pub(crate) form_data: bool,

    /// optional headers
    #[arg(long)]
    pub(crate) url_encoded: bool,

    /// extract path of the response
    #[arg(short, long)]
    pub(crate) extract_path: Option<Vec<String>>,

    /// chain with another action
    #[arg(short, long)]
    pub(crate) chain: Option<Vec<String>>,

    /// add expectation
    #[arg(short = 'E', long)]
    pub(crate) expect: Option<Vec<String>>,

    /// save command line as test suite step
    #[arg(long)]
    pub(crate) save_to_ts: Option<String>,

    /// save command line as flow
    #[arg(long)]
    pub(crate) save: Option<String>,

    /// save action to project of form action_name:project_name
    //#[arg(long)]
    //pub(crate) save_to_project: Option<String>,

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
    pub async fn save_if_needed(
        &self,
        db: &DBHandler,
        run_action_args: &RunActionArgs,
    ) -> anyhow::Result<()> {
        if let Some(action_name) = &self.save {
            let mut merged = run_action_args.clone();
            if let Some(current_action_name) = &self.name {
                let action = db.get_action(current_action_name).await?;
                let run_action_args_from_db = action.get_run_action_args()?;
                merged = merge_with(&run_action_args_from_db, run_action_args);
            }
            let r = db
                .upsert_action(&Action {
                    id: None,
                    name: Some(action_name.clone()),
                    run_action_args: Some(serde_json::to_string(&merged)?),
                    body_example: None,
                    response_example: None,
                    project_name: None,
                    created_at: None,
                    updated_at: None,
                })
                .await;

            match r {
                Ok(_) => println!("Action {} saved", action_name.clone().green()),
                Err(e) => println!("Error saving action {}", e),
            }
        };
        Ok(())
    }

    async fn save_to_ts_if_needed(
        &self,
        db: &DBHandler,
        self_clone: &RunActionArgs,
    ) -> anyhow::Result<()> {
        // save as requested
        match &self.save_to_ts {
            Some(ts_name) => {
                // ensuring test suite exists
                db.upsert_test_suite(ts_name).await?;
                // add test instance
                let r = db
                    .upsert_test_suite_instance(&TestSuiteInstance {
                        id: None,
                        test_suite_name: ts_name.clone(),
                        run_action_args: to_string(&self_clone).unwrap(),
                        created_at: None,
                        updated_at: None,
                    })
                    .await;
                match &r {
                    Ok(_) => println!("{}", format!("Test saved in {}", ts_name.clone().green())),
                    Err(e) => println!("Error saving test {}", e),
                }
                r
            }
            None => Ok(()),
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
    pub fn prepare(&mut self) -> anyhow::Result<()> {
        let is_chained_cmd = self.chain.is_some();
        let chain_len = self.chain.as_mut().map(|v| v.len() + 1).unwrap_or(1);
        let mut data = HashMap::from([
            ("body", &mut self.body),
            ("header", &mut self.header),
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

    pub fn get_infos(&self) -> Vec<(&String, &String, &String, &String, &String, &String)> {
        // check input and return an error if needed
        if let Err(msg) = check_input(&self) {
            eprintln!("{}", msg);
            panic!("Invalid input");
        }
        // check if action is chained
        itertools::izip!(
            self.chain.as_ref().map(|c| c.into_iter()).unwrap(),
            self.header.as_ref().expect("Internal error").into_iter(),
            self.body.as_ref().expect("Internal error").into_iter(),
            self.extract_path
                .as_ref()
                .expect("Internal error")
                .into_iter(),
            self.path_params
                .as_ref()
                .expect("Internal error")
                .into_iter(),
            self.query_params
                .as_ref()
                .expect("Internal error")
                .into_iter(),
        )
        .collect_vec()
    }

    /// Main function for running an action
    #[async_recursion::async_recursion]
    pub async fn run_action<'a>(
        &'a mut self,
        http: &'a http::Api,
        db: &'a DBHandler,
        multi: Option<&'a MultiProgress>,
    ) -> (Vec<R>, Vec<bool>) {
        // make a clone a the beginning as we mutate
        // this instance latter in prepare method
        let self_clone = self.clone();

        // check input and return an error if needed
        if let Err(msg) = check_input(&self) {
            eprintln!("{}", msg);
            panic!("Invalid input");
        }

        // creating a new context hashmap for storing extracted values
        let mut ctx: HashMap<String, String> = match db.get_conf().await {
            Ok(ctx) => ctx.get_value(),
            Err(_) => HashMap::new(),
        };
        // create printer to print results
        let mut printer = Printer::new(self.quiet, self.clipboard, self.grep);

        // creating progress bars here
        let multi_bar = multi.cloned().unwrap_or(MultiProgress::new());

        let main_pb = multi_bar.add(
            ProgressBar::new((self.chain.as_ref().map(|c| c.len()).unwrap_or(0) + 1) as u64)
                .with_style(
                    ProgressStyle::default_bar()
                        .template(
                            "{spinner} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
                        )
                        .unwrap(),
                ),
        );
        main_pb.enable_steady_tick(Duration::from_millis(100));

        // get actions to be ran
        let mut actions = DomainAction::from_run_args(self, db, &ctx).await;
        // storing results here
        let mut action_results = vec![];

        for action in &mut actions {
            // i.e if action does not contain any interpolated value
            if let Some(run_action_args) = &mut action.run_action_args {
                if run_action_args.chain.is_some() {
                    let (r, _) = run_action_args.run_action(http, db, multi).await;
                    action_results.push(r);
                    continue;
                }
            }
            if !action.can_be_run() {
                continue;
            }
            action_results.push(
                action
                    .run(db, http, &mut ctx, &multi_bar)
                    .await
                    .into_iter()
                    .map(|(url, result)| {
                        let _ = HttpResult {
                            fetch_result: result.as_ref(),
                            printer: &mut printer,
                        }
                        .handle_result(
                            action.extract_path.as_ref(),
                            &mut ctx,
                            &main_pb,
                        );
                        R {
                            url: url.to_string(),
                            result,
                            ctx: ctx.clone(),
                        }
                    })
                    .collect::<Vec<R>>(),
            );
        } // end for

        // if expect run test check
        let last_results = action_results.last();
        let last_action = actions.last().unwrap();
        let mut tests_is_success = vec![];
        if let Some(last_results) = last_results {
            let expected = if self.expect.is_some() {
                parse_cli_conf_to_map(self.expect.as_ref())
            } else {
                last_action.expected.clone()
            };
            if let Some(expected) = &expected {
                tests_is_success = TestChecker::new(last_results, &ctx, expected).check(
                    self.name.as_ref().map(|n| n.as_str()).unwrap_or("flow"),
                    &main_pb,
                );
            }
        }

        // finishing progress bar
        main_pb.finish_and_clear();

        let _ = self.save_if_needed(db, &self_clone).await;
        let _ = self.save_to_ts_if_needed(db, &self_clone).await;

        (
            action_results.into_iter().flatten().collect_vec(),
            tests_is_success,
        )

        /*
            // deals with configuration
            ctx.extend(project
                .as_ref()
                .map(|p| p.get_project_conf().ok())
                .flatten()
                .unwrap_or(HashMap::new()));

            // try to find a project url
            let main_project_url = project.as_ref().map(|p| p.main_url.as_str());

            // check if it can be run
            let mut can_be_ran = true;

            if computed_urls.len() == 1 && computed_urls.iter().next().unwrap().contains('{') {
                can_be_ran = false;
            }

            // compute body
            let body = get_body(
                action_body,
                action.as_ref().and_then(|a| a.static_body.as_deref()),
                action.as_ref().and_then(|a| a.body_example.as_deref()),
                &ctx,
            );

            if let Some(b) = body.as_ref() {
                if b.contains("{{") {
                    can_be_ran = false;
                }
            }

            // compute headers
            let computed_headers = get_str_as_interpolated_map(
                action.as_ref().map(|a| a.headers.as_str()).unwrap_or("{}"),
                &ctx,
                Interpol::MultiInterpol,
            )
            .unwrap_or_else(|| {
                let headers_from_cli = parse_cli_conf_to_map(self.header.as_ref());
                headers_from_cli
                    .into_iter()
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
                    .collect()
            });

            if computed_headers.values().any(|v| v.contains("{{")) {
                can_be_ran = false;
            }

            // all possible query params
            let computed_query_params = parse_multiple_conf_as_opt_with_grouping_and_interpolation(
                query_params,
                &ctx,
                Interpol::MultiInterpol,
            );

            if computed_query_params
                .iter()
                .filter_map(|opt| opt.as_ref().map(|vv| vv.values().any(|v| v.contains("{{"))))
                .any(|v| v)
            {
                can_be_ran = false
            }

            if can_be_ran {
                // run in concurrent mod
                let fetch_results = future::join_all(
                    computed_urls
                        .iter()
                        .cartesian_product(computed_query_params)
                        .map(|(computed_url, query_params)| {
                            // clone needed variables
                            let extended_ctx = ctx.clone();
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
                                        status_code: fetch_result_ref
                                            .map(|r| r.status)
                                            .unwrap_or(0u16),
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
                                            action.body_example =
                                                body.as_ref().map(|b| b.to_string());

                                            if db.upsert_action(action).await.is_err() {
                                                pb.println(format!("{}", "Error upserting action".red()));
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
                                (get_full_url(computed_url, query_params.as_ref()), fetch_result)
                            }
                        }),
                )
                .await;


                // parse extracted path from cli
                let xtracted_path = get_xtracted_path(xtract_path, self.force, &ctx);

                for action_result in fetch_results {
                    // handle result and print extracted data
                    let _ = HttpResult::new(&action_result.1, &mut printer).handle_result(
                        xtracted_path.as_ref(),
                        &mut ctx,
                        &main_pb,
                    );
                    action_results.push(
                        R {
                        url: action_result.0,
                        result: action_result.1,
                        ctx: ctx.clone(),
                    });
                }

                // increment the main progress bar
                main_pb.inc(1);
            }
        }

        // save as requested
        if let Some(ts_name) = &self.save_to_ts {
            // ensuring test suite exists
            db.upsert_test_suite(ts_name).await?;
            // add test instance
            let r = db.upsert_test_suite_instance(&TestSuiteInstance {
                id: None,
                test_suite_name: ts_name.clone(),
                action: to_string(&self_clone).unwrap(),
            }).await;
            match r {
                Ok(_) => main_pb.println(format!("{}", format!("Test saved in {}", ts_name.clone().green()))),
                Err(e) => main_pb.println(format!("Error saving test {}", e))
            }
        }

        // save to flow
        if let Some(flow_name) = &self.save_to_flow {
            let r = db.upsert_flow(flow_name, &self_clone).await;
            match r {
                Ok(_) => main_pb.println(format!("{}", format!("Flow {} saved", flow_name.clone().green()))),
                Err(e) => main_pb.println(format!("Error saving flow {}", e))
            }
        }
        */
    }
}
