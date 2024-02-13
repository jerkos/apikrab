use crate::commands::run::_printer::Printer;
use crate::commands::run::_run_helper::check_input;
use crate::db::db_trait::Db;
use crate::db::dto::{Action, Context, Project, TestSuite, TestSuiteInstance};
use crate::domain::DomainAction;
use crate::http;
use crate::http::FetchResult;
use crate::utils::{val_or_join, SEP, SINGLE_INTERPOL_START};
use clap::Args;
use crossterm::style::Stylize;
use indicatif::{MultiProgress, ProgressDrawTarget};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::exit;
use std::time::Duration;

use super::_progress_bar::new_pb;
use super::_run_helper::ANONYMOUS_ACTION;

#[derive(Debug)]
pub struct R {
    pub url: String,
    pub result: anyhow::Result<FetchResult>,
    pub ctx: HashMap<String, String>,
}

pub struct CurrentActionData<'a> {
    name: &'a str,
    project: Option<&'a str>,
    header: &'a str,
    body: &'a str,
    xtract_path: &'a str,
    path_params: &'a str,
    query_params: &'a str,
}

impl CurrentActionData<'_> {
    fn get_verb_and_url(run_action_args: &RunActionArgs) -> (&str, &str) {
        let verb = run_action_args.verb.as_deref().unwrap_or("");
        let url = run_action_args.url.as_deref().unwrap_or("");
        (verb, url)
    }

    pub fn to_domain_action(
        &self,
        run_action_args: &RunActionArgs,
        project: Option<&Project>,
        ctx: &HashMap<String, String>,
    ) -> DomainAction {
        let (verb, url) = Self::get_verb_and_url(run_action_args);
        DomainAction::from_current_action_data(
            verb,
            url,
            val_or_join(self.header, run_action_args.header.as_ref()).as_ref(),
            (
                self.body,
                run_action_args.url_encoded,
                run_action_args.form_data,
            ),
            val_or_join(self.xtract_path, run_action_args.extract_path.as_ref()).as_ref(),
            val_or_join(self.path_params, run_action_args.path_params.as_ref()).as_ref(),
            val_or_join(self.query_params, run_action_args.query_params.as_ref()).as_ref(),
            run_action_args.expect.as_ref(),
            project,
            ctx,
        )
    }
}

/// Asynchronously retrieves an `Action` and a `Project` based on the provided current action data.
///
/// This function attempts to retrieve an `Action` from the database using the name and project
/// from the current action data. If the retrieval is successful, it returns `Some(Action)`. If
/// there is an error during the retrieval, it returns `None`.
///
/// It also attempts to retrieve a `Project` if `current_action_data.project` is `Some`. If it is,
/// it retrieves the `Project` from the database using `DomainAction::project_from_db` and returns
/// it. If `current_action_data.project` is `None`, it returns `None`.
///
/// # Arguments
///
/// * `db` - A dynamic reference to a `Db` trait object that provides database functionality.
/// * `current_action_data` - A reference to the current action data.
///
/// # Returns
///
/// * `(Option<Action>, Option<Project>)` - A tuple containing an `Option` that may contain an
///   `Action` and an `Option` that may contain a `Project`.
///
/// # Example
///
/// ```no_run
/// # async fn run() -> (Option<Action>, Option<Project>) {
/// # let db: &dyn Db = &Database::new();
/// # let current_action_data = CurrentActionData::new();
/// let (action, project) = get_action_and_project(db, &current_action_data).await;
/// # (action, project)
/// # }
/// ```
async fn get_action_and_project(
    db: &dyn Db,
    current_action_data: &CurrentActionData<'_>,
) -> (Option<Action>, Option<Project>) {
    (
        db.get_action(current_action_data.name, current_action_data.project)
            .await
            .ok(),
        if current_action_data.project.is_none() {
            None
        } else {
            DomainAction::project_from_db(current_action_data.project, db).await
        },
    )
}

#[derive(Args, Serialize, Deserialize, Debug, Clone, Default)]
pub struct RunActionArgs {
    /// action name optional
    pub(crate) name: Option<String>,

    #[arg(long)]
    pub(crate) project: Option<String>,

    #[arg(short, long)]
    pub(crate) url: Option<String>,

    #[arg(long, value_parser = ["GET", "POST", "PUT", "DELETE"])]
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
    pub(crate) clipboard: bool,

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
    /// Asynchronously saves the provided actions to an `Action` if `self.save` is `Some`.
    ///
    /// This function checks if `self.save` is `Some`. If it is, it creates a new `Action` with the
    /// name from `self.save` and the provided actions, and upserts it into the database. The `Action`
    /// is created with `None` for `id`, `body_example`, `response_example`, and `project_name`, and
    /// the current UTC time for `created_at` and `updated_at`.
    ///
    /// If `self.save` is `None`, the function does nothing and returns `Ok(())`.
    ///
    /// # Arguments
    ///
    /// * `db` - A dynamic reference to a `Db` trait object that provides database functionality.
    /// * `actions` - A slice of `DomainAction` objects to be saved to the `Action`.
    ///
    /// # Returns
    ///
    /// * `anyhow::Result<()>` - Returns `Ok(())` if the operation is successful. If there is an error
    ///   during the operation, it returns `Err(e)`, where `e` is the error.
    ///
    /// # Errors
    ///
    /// This function will return an error if there is a problem upserting the `Action` into the
    /// database.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn run() -> anyhow::Result<()> {
    /// # let db: &dyn Db = &Database::new();
    /// # let actions: &[DomainAction] = &[];
    /// # let obj = Object::new();
    /// obj.save_if_needed(db, actions).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn save_if_needed(
        &self,
        db: &dyn Db,
        actions: &[DomainAction],
    ) -> anyhow::Result<()> {
        if let Some(action_name) = &self.save {
            let r = db
                .upsert_action(&Action {
                    id: None,
                    name: Some(action_name.clone()),
                    actions: actions.to_owned(),
                    body_example: None,
                    response_example: None,
                    project_name: self.project.clone(),
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    updated_at: Some(chrono::Utc::now().naive_utc()),
                })
                .await;

            match r {
                Ok(_) => println!("Action {} saved", action_name.clone().green()),
                Err(e) => println!("Error saving action {}", e),
            }
        };
        Ok(())
    }

    /// Asynchronously saves the provided actions to a test suite if needed.
    ///
    /// This function checks if `self.save_to_ts` is `Some`. If it is, it creates a new `TestSuite`
    /// with the name from `self.save_to_ts` and upserts it into the database. It then creates a new
    /// `TestSuiteInstance` with the provided actions and upserts it into the database as well.
    ///
    /// If `self.save_to_ts` is `None`, the function does nothing and returns `Ok(())`.
    ///
    /// # Arguments
    ///
    /// * `db` - A dynamic reference to a `Db` trait object that provides database functionality.
    /// * `actions` - A slice of `DomainAction` objects to be saved to the test suite.
    ///
    /// # Returns
    ///
    /// * `anyhow::Result<()>` - Returns `Ok(())` if the operation is successful. If there is an error
    ///   during the operation, it returns `Err(e)`, where `e` is the error.
    ///
    /// # Errors
    ///
    /// This function will return an error if there is a problem upserting the `TestSuite` or
    /// `TestSuiteInstance` into the database.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn run() -> anyhow::Result<()> {
    /// # let db: &dyn Db = &Database::new();
    /// # let actions: &[DomainAction] = &[];
    /// # let obj = Object::new();
    /// obj.save_to_ts_if_needed(db, actions).await?;
    /// # Ok(())
    /// # }
    async fn save_to_ts_if_needed(
        &self,
        db: &dyn Db,
        actions: &[DomainAction],
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
                        actions: actions.to_owned(),
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
                                let contains_acc =
                                    data_vec.iter().any(|s| s.contains(SINGLE_INTERPOL_START));
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
                .unwrap_or(ANONYMOUS_ACTION.to_string()),
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

    /// Get all action data
    /// If the action is chained, we need to get all data
    /// for each action, otherwise we just need to get the data
    /// for the current action
    pub fn get_action_data(&self) -> Vec<CurrentActionData> {
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
            project: self.project.as_deref(),
            header: d.1,
            body: d.2,
            xtract_path: d.3,
            path_params: d.4,
            query_params: d.5,
        })
        .collect_vec()
    }

    /// Returns a vector of `DomainAction` objects based on the provided parameters.
    ///
    /// This function checks if `runnable_actions_from_db` is `Some`. If it is, and if it contains
    /// exactly one action, it merges this action with the current action data and returns a vector
    /// containing the merged action. If `runnable_actions_from_db` contains more than one action,
    /// it returns them as a vector.
    ///
    /// If `runnable_actions_from_db` is `None`, it creates a new action from the current action data
    /// and returns it in a vector.
    ///
    /// # Arguments
    ///
    /// * `runnable_actions_from_db` - An `Option` that contains a reference to a vector of
    ///   `DomainAction` objects, or `None`.
    /// * `current_action_data` - The current action data.
    /// * `project` - An `Option` that contains a `Project` object, or `None`.
    /// * `ctx` - A reference to a `HashMap` that maps `String` keys to `String` values.
    ///
    /// # Returns
    ///
    /// * `Vec<DomainAction>` - A vector of `DomainAction` objects.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # let runnable_actions_from_db: Option<&Vec<DomainAction>> = None;
    /// # let current_action_data = CurrentActionData::new();
    /// # let project: Option<Project> = None;
    /// # let ctx: &HashMap<String, String> = &HashMap::new();
    /// # let obj = Object::new();
    /// let actions = obj.get_runnable_actions(runnable_actions_from_db, current_action_data, project, ctx);
    /// ```
    fn get_runnable_actions(
        &self,
        runnable_actions_from_db: Option<&Vec<DomainAction>>,
        current_action_data: CurrentActionData<'_>,
        project: Option<Project>,
        ctx: &HashMap<String, String>,
    ) -> Vec<DomainAction> {
        match &runnable_actions_from_db {
            Some(runnable_actions) => {
                // need to merge with current action if runnable action has length one
                if runnable_actions.len() == 1 {
                    let merged = runnable_actions[0].merge_with(
                        &current_action_data.to_domain_action(self, project.as_ref(), ctx),
                    );
                    vec![merged]
                } else {
                    runnable_actions.to_vec()
                }
            }
            None => {
                // got an anonymous action so we need to create a new one
                vec![current_action_data.to_domain_action(self, project.as_ref(), ctx)]
            }
        }
    }

    /// Run the action
    /// If the action is chained, we need to run each action
    /// and return the result of each action
    pub async fn run_action<'a>(
        &'a mut self,
        http: &'a http::Api,
        db: &dyn Db,
        display_pb: bool,
    ) -> anyhow::Result<()> {
        // check input and return an error if needed
        if let Err(msg) = check_input(self) {
            eprintln!("{}", msg);
            exit(1);
        }

        // creating a new context hashmap for storing extracted values
        let mut ctx: HashMap<String, String> = match db.get_conf().await {
            Ok(ctx) => ctx.get_value(),
            Err(_) => HashMap::new(),
        };
        // create printer to print results
        let mut printer = Printer::new(self.quiet, self.clipboard, self.grep);

        // creating progress bars here
        let multi_bar = MultiProgress::new();
        if !display_pb {
            multi_bar.set_draw_target(ProgressDrawTarget::hidden());
        }

        // main progress bar
        let main_pb = multi_bar.add(new_pb(
            (self.chain.as_ref().map(|c| c.len()).unwrap_or(0) + 1) as u64,
        ));

        if !display_pb {
            main_pb.set_draw_target(ProgressDrawTarget::hidden());
        }
        main_pb.enable_steady_tick(Duration::from_millis(100));

        // prepare the data
        self.prepare()?;

        // vector to gather all actions
        let mut actions = vec![];

        // iterating possible action if it is a chained action
        for current_action_data in self.get_action_data().into_iter() {
            // retrieve action from db if needed
            let (action, project) = get_action_and_project(db, &current_action_data).await;

            // extend configuration if necessary
            ctx.extend(
                project
                    .as_ref()
                    .and_then(|p| p.get_project_conf().ok())
                    .unwrap_or(HashMap::new()),
            );

            // retrieve action stored in db
            let runnable_actions_from_db = action.as_ref().map(|a| &a.actions);

            // merge if needed with current action
            let runnable_actions = self.get_runnable_actions(
                runnable_actions_from_db,
                current_action_data,
                project,
                &ctx,
            );

            // keeping track of all computed actions
            actions.extend(runnable_actions.iter().cloned());

            // running each actions
            for runnable_action in runnable_actions.iter() {
                // check if it can be run
                if !runnable_action.can_be_run() {
                    printer.p_info(|| println!("Action cannot be run due to missing information"));
                    continue;
                }

                // run the action and push the result in the stack
                let _ = runnable_action
                    .run_with_tests(
                        action.as_ref(),
                        &mut ctx,
                        db,
                        http,
                        &mut printer,
                        &multi_bar,
                        &main_pb,
                    )
                    .await;

                main_pb.inc(1);
            } // end for
        } // end for

        // saving current session context
        if db
            .insert_conf(&Context { value: ctx.clone() })
            .await
            .is_err()
        {
            main_pb.println("Error inserting context");
        }

        // finishing progress bar
        main_pb.finish();

        // post result actions
        self.save_if_needed(db, &actions).await?;
        self.save_to_ts_if_needed(db, &actions).await?;

        Ok(())
    }
}
