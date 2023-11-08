use crate::commands::run::_http_result::HttpResult;
use crate::commands::run::_printer::Printer;
use crate::commands::run::_run_helper::check_input;
use crate::db::db_handler::DBHandler;
use crate::db::dto::{Action, Context, Project, TestSuite, TestSuiteInstance};
use crate::domain::DomainAction;
use crate::http;
use crate::http::FetchResult;
use crate::utils::{parse_cli_conf_to_map, val_or_join, SEP, SINGLE_INTERPOL_START};
use clap::Args;
use core::panic;
use crossterm::style::Stylize;
use indicatif::{MultiProgress, ProgressBar};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::to_string;
use std::collections::HashMap;
use std::time::Duration;

use super::_progress_bar::new_pb;
use super::_run_helper::{is_anonymous_action, merge_with};
use super::_test_checker::TestChecker;

pub struct R {
    pub url: String,
    pub result: anyhow::Result<FetchResult>,
    pub ctx: HashMap<String, String>,
}

pub struct CurrentActionData<'a> {
    name: &'a str,
    header: &'a str,
    body: &'a str,
    xtract_path: &'a str,
    path_params: &'a str,
    query_params: &'a str,
}

impl CurrentActionData<'_> {
    pub fn to_domain_action(
        &self,
        run_action_args: &RunActionArgs,
        project: Option<&Project>,
        ctx: &HashMap<String, String>,
    ) -> DomainAction {

        DomainAction::from_current_action_data(
            self.name,
            run_action_args
                .verb
                .as_ref()
                .expect("No verb defined !")
                .as_str(),
            run_action_args
                .url
                .as_ref()
                .expect("No url defined !")
                .as_str(),
            val_or_join(self.header, run_action_args.header.as_ref()).as_ref(),
            (
                self.body,
                run_action_args.url_encoded,
                run_action_args.form_data,
            ),
            val_or_join(self.xtract_path, run_action_args.extract_path.as_ref()).as_ref(),
            val_or_join(self.path_params, run_action_args.path_params.as_ref()).as_ref(),
            val_or_join(self.query_params, run_action_args.query_params.as_ref()).as_ref(),
            project,
            None,
            ctx,
        )
    }
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

    /// do not check ssl certificate
    #[arg(short = 'k', long)]
    #[serde(default)]
    pub(crate) insecure: bool,

    /// timeout on request
    #[arg(short, long)]
    #[serde(default)]
    pub(crate) timeout: Option<u64>,
}

impl RunActionArgs {
    pub async fn run_test_if_needed(
        &self,
        action_results: &[Vec<R>],
        ctx: &HashMap<String, String>,
        main_pb: &ProgressBar,
    ) -> Vec<bool> {
        // if expect run test check
        let last_results = action_results.last();
        let mut tests_is_success = vec![];
        if let Some(last_results) = last_results {
            let expected = parse_cli_conf_to_map(self.expect.as_ref());
            if let Some(expected) = &expected {
                tests_is_success = TestChecker::new(last_results, ctx, expected).check(
                    self.name.as_deref().unwrap_or("flow"),
                    main_pb,
                );
            }
        }
        tests_is_success
    }

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
                let ts = TestSuite {
                    id: None,
                    name: ts_name.clone(),
                    created_at: None,
                };
                db.upsert_test_suite(&ts).await?;
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
                    Ok(_) => println!("Test saved in {}", ts_name.clone().green()),
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
                                let contains_acc = data_vec.iter().any(|s| s.contains(SINGLE_INTERPOL_START));
                                if contains_acc {
                                    anyhow::bail!(
                                        "Chain, body and extract path must have the same length"
                                    );
                                }
                            }
                            let merged_data = data_vec.join(SEP);
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

    pub fn get_action_data(&self) -> Vec<CurrentActionData> {
        // check input and return an error if needed
        if let Err(msg) = check_input(self) {
            eprintln!("{}", msg);
            panic!("Invalid input");
        }
        // check if action is chained
        itertools::izip!(
            self.chain.as_ref().map(|c| c.iter()).unwrap(),
            self.header.as_ref().unwrap().iter(),
            self.body.as_ref().unwrap().iter(),
            self.extract_path.as_ref().unwrap().iter(),
            self.path_params.as_ref().unwrap().iter(),
            self.query_params.as_ref().unwrap().iter(),
        )
        .map(|d| CurrentActionData {
            name: d.0,
            header: d.1,
            body: d.2,
            xtract_path: d.3,
            path_params: d.4,
            query_params: d.5,
        })
        .collect_vec()
    }

    /// Main function for running an action
    #[async_recursion::async_recursion]
    pub async fn run_action<'a>(
        &'a mut self,
        http: &'a http::Api,
        db: &'a DBHandler,
        multi: Option<&'a MultiProgress>,
        pb: Option<&'a ProgressBar>,
    ) -> (Vec<R>, Vec<bool>) {
        // make a clone a the beginning as we mutate
        // this instance latter in prepare method
        let self_clone = self.clone();

        // check input and return an error if needed
        if let Err(msg) = check_input(self) {
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

        // main progress bar
        let main_pb = pb.cloned().unwrap_or_else(|| {
            let main_pb = multi_bar.add(new_pb(
                (self.chain.as_ref().map(|c| c.len()).unwrap_or(0) + 1) as u64,
            ));
            main_pb.enable_steady_tick(Duration::from_millis(100));
            main_pb
        });

        let mut action_results = vec![];

        // prepare the data
        let _ = self.prepare();

        // iterating over actions in current actions
        for current_action_data in self.get_action_data().into_iter() {
            // retrieve action from db if needed
            let (action, project) = (
                db.get_action(current_action_data.name).await.ok(),
                DomainAction::project_from_db(current_action_data.name, db).await,
            );

            // extend configuration if necessary
            ctx.extend(
                project
                    .as_ref()
                    .and_then(|p| p.get_project_conf().ok())
                    .unwrap_or(HashMap::new()),
            );

            // retrieve run action args
            let run_action_args_ac = action
                .as_ref()
                .map(|a| a.get_run_action_args().expect("Error loading action"))
                .unwrap_or(self.clone());

            let mut runnable_action =
                current_action_data.to_domain_action(&run_action_args_ac, project.as_ref(), &ctx);
            runnable_action.run_action_args = Some(run_action_args_ac);

            if let Some(run_action_args) = runnable_action.run_action_args.as_mut() {
                if !is_anonymous_action(&runnable_action.name) && run_action_args.chain.is_some() {
                    // inherit some sane configuration
                    run_action_args.save = None;
                    run_action_args.save_to_ts = None;
                    run_action_args.quiet = self.quiet;
                    run_action_args.grep = self.grep;
                    let (r, _) = run_action_args
                        .run_action(http, db, Some(&multi_bar), Some(&main_pb))
                        .await;
                    action_results.push(r);
                    main_pb.inc(1);
                    continue;
                }
            }

            if !runnable_action.can_be_run() {
                continue;
            }
            action_results.push(
                runnable_action
                    .run(action.as_ref(), db, http, &multi_bar)
                    .await
                    .into_iter()
                    .map(|(url, result)| {
                        let _ = HttpResult {
                            fetch_result: result.as_ref(),
                            printer: &mut printer,
                        }
                        .handle_result(
                            runnable_action.extract_path.as_ref(),
                            &mut ctx,
                            &main_pb,
                        );
                        R {
                            url,
                            result,
                            ctx: ctx.clone(),
                        }
                    })
                    .collect::<Vec<R>>(),
            );
            main_pb.inc(1);
        } // end for

        // saving current session context
        if db
            .insert_conf(&Context {
                value: serde_json::to_string(&ctx).expect("Error serializing context"),
            })
            .await
            .is_err()
        {
            main_pb.println("Error inserting context");
        }
        // if expect run test check
        let test_results = self
            .run_test_if_needed(&action_results, &ctx, &main_pb)
            .await;

        // finishing progress bar
        main_pb.finish();

        let _ = self.save_if_needed(db, &self_clone).await;
        let _ = self.save_to_ts_if_needed(db, &self_clone).await;

        (
            action_results.into_iter().flatten().collect_vec(),
            test_results,
        )

        /*

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
        */
    }
}
