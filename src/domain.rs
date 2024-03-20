use std::{
    borrow::{Borrow, Cow},
    collections::{HashMap, HashSet},
    time::Duration,
};

use crate::{
    commands::run::{
        _http_result::HttpResult,
        _printer::Printer,
        _progress_bar::{add_progress_bar_for_request, finish_progress_bar},
        _run_helper::{self, get_body, get_computed_urls, get_xtracted_path, ANONYMOUS_ACTION},
        _test_checker::{TestChecker, UnaryTestResult},
        action::R,
    },
    db::{
        db_trait::Db,
        dto::{Action, History, Project},
    },
    http::{self, Api, FetchResult},
    python::{run_python_post_script, run_python_pre_script, PyRequest, Request},
    utils::{
        contains_interpolation, format_query, get_full_url, get_str_as_interpolated_map,
        map_contains_interpolation, parse_cli_conf_to_map,
        parse_multiple_conf_as_opt_with_grouping_and_interpolation, Interpol,
    },
};

use futures::future;
use indicatif::{MultiProgress, ProgressBar};
use itertools::Itertools;
use pyo3::PyErr;
use serde::{Deserialize, Serialize};

/// Extract name from optional action
pub fn get_action_name(action: Option<&Action>) -> &str {
    let action_name_opt = action.as_ref().and_then(|a| a.name.as_deref());
    action_name_opt.unwrap_or(ANONYMOUS_ACTION)
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Body {
    pub(crate) body: String,
    pub(crate) url_encoded: bool,
    pub(crate) form_data: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DomainActions {
    pub(crate) actions: Vec<DomainAction>,
}

impl From<Vec<DomainAction>> for DomainActions {
    fn from(actions: Vec<DomainAction>) -> Self {
        DomainActions { actions }
    }
}

impl From<&Vec<DomainAction>> for DomainActions {
    fn from(actions: &Vec<DomainAction>) -> Self {
        DomainActions {
            actions: actions.to_vec(),
        }
    }
}

fn default_timeout() -> u64 {
    10
}

fn default_insecure() -> bool {
    false
}

fn default_pre_script() -> Option<String> {
    Some(
        r#"import sys
sys.path.append('./venv/lib/python3.11/site-packages')
print("hello world")
"#
        .to_string(),
    )
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DomainAction {
    pub(crate) verb: String,
    pub(crate) headers: Option<HashMap<String, String>>,
    pub(crate) url: String,
    pub(crate) path_params: Option<Vec<HashMap<String, String>>>,
    pub(crate) query_params: Option<Vec<HashMap<String, String>>>,
    pub(crate) body: Option<Body>,
    pub(crate) extract_path: Option<HashMap<String, Option<String>>>,
    pub(crate) expect: Option<HashMap<String, String>>,

    #[serde(default = "default_pre_script")]
    pub(crate) pre_script: Option<String>,

    pub(crate) post_script: Option<String>,

    #[serde(default = "default_insecure")]
    pub(crate) insecure: bool,

    #[serde(default = "default_timeout")]
    pub(crate) timeout: u64,
}

impl DomainAction {
    /// Merge two domain actions into a new one
    /// basically cloning elements of other if they are not empty
    /// and if they are empty, cloning elements of self
    ///
    /// # Example
    /// ```rust
    /// use crate::domain::DomainAction;
    /// let action1 = DomainAction {
    ///    verb: "GET".to_string(),
    ///     ..Default::default()
    /// };
    /// let action2 = DomainAction {
    ///   verb: "POST".to_string(),
    ///  ..Default::default()
    /// };
    /// let merged = action1.merge_with(&action2);
    ///```
    pub fn merge_with(&self, other: &DomainAction) -> DomainAction {
        // merge two domain actions into a new one
        // the new one will have the same name as the first one
        DomainAction {
            verb: if !other.verb.is_empty() {
                other.verb.clone()
            } else {
                self.verb.clone()
            },
            headers: other.headers.clone().or(self.headers.clone()),
            url: if !other.url.is_empty() {
                other.url.clone()
            } else {
                self.url.clone()
            },
            path_params: if other.path_params.is_some() {
                other.path_params.clone()
            } else {
                self.path_params.clone()
            },
            query_params: if other.query_params.is_some() {
                other.query_params.clone()
            } else {
                self.query_params.clone()
            },
            body: if other.body.is_some() {
                other.body.clone()
            } else {
                self.body.clone()
            },
            extract_path: other.extract_path.clone().or(self.extract_path.clone()),
            expect: other.expect.clone().or(self.expect.clone()),
            pre_script: other.pre_script.clone().or(self.pre_script.clone()),
            post_script: other.post_script.clone().or(self.post_script.clone()),
            insecure: other.insecure,
            timeout: other.timeout,
        }
    }

    /// Check if an action can be run
    /// an action can be run if it does not contain any interpolation
    /// in its url, body, headers or query_params
    /// # Example
    /// ```rust
    /// use crate::domain::DomainAction;
    /// let action = DomainAction {
    ///   verb: "GET".to_string(),
    ///   ..Default::default()
    /// };
    /// assert!(action.can_be_run());
    /// ```
    pub fn can_be_run(&self, urls: &HashSet<String>) -> bool {
        let mut can_be_ran = true;
        if let Some(url) = urls.iter().next() {
            if contains_interpolation(url, Interpol::SimpleInterpol) {
                can_be_ran = false;
            }
        }
        if let Some(ref body) = self.body {
            if contains_interpolation(&body.body, Interpol::MultiInterpol) {
                can_be_ran = false;
            }
        }
        if let Some(ref header) = self.headers {
            if map_contains_interpolation(header, Interpol::MultiInterpol) {
                can_be_ran = false;
            }
        }
        if let Some(ref query_params) = self.query_params {
            if query_params
                .iter()
                .map(|m| map_contains_interpolation(m, Interpol::MultiInterpol))
                .any(|v| v)
            {
                can_be_ran = false
            }
        }
        can_be_ran
    }

    async fn insert_history_line(
        &self,
        action_name: &str,
        computed_url: &str,
        fetch_result: anyhow::Result<&FetchResult, &anyhow::Error>,
        db: &dyn Db,
    ) -> anyhow::Result<i64> {
        let f = fetch_result.as_ref();
        db.insert_history(&History {
            id: None,
            action_name: action_name.to_string(),
            url: computed_url.to_string(),
            body: self
                .body
                .as_ref()
                .map(|s| serde_json::from_str(&s.body).unwrap()),
            headers: Some(serde_json::to_string(&self.headers).unwrap()),
            response: f.map(|r| r.response.clone()).ok(),
            status_code: f.map(|r| r.status).unwrap_or(0u16),
            duration: f.map(|r| r.duration.as_secs_f32()).unwrap_or(0f32),
            created_at: None,
        })
        .await
    }

    pub async fn upsert_action(
        &self,
        fetch_result: anyhow::Result<&FetchResult, &anyhow::Error>,
        action_opt: Option<Action>,
        db: &dyn Db,
    ) -> anyhow::Result<()> {
        match fetch_result.ok().zip(action_opt) {
            Some((f @ FetchResult { response, .. }, ref mut action)) => {
                if f.is_success() {
                    action.response_example = serde_json::from_str(response).ok();
                    action.body_example = self
                        .body
                        .as_ref()
                        .and_then(|s| serde_json::from_str(&s.body).ok());
                    return db.upsert_action(action).await;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// retrieve project from db
    /// return None if anonymous action
    /// return Default project if no project found
    pub async fn project_from_db(project_name: Option<&str>, db: &dyn Db) -> Option<Project> {
        match project_name.as_ref() {
            Some(p_name) => db.get_project(p_name).await.ok(),
            None => db.get_project("default").await.ok(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_current_action_data(
        verb: &str,
        run_action_args_url: &str,
        header: &str,
        body: (&str, bool, bool),
        xtract_path: &str,
        path_params: &str,
        query_params: &str,
        expect: Option<&Vec<String>>,
        project: Option<&Project>,
        insecure: bool,
        timeout: u64,
        ctx: &HashMap<String, String>,
    ) -> DomainAction {
        DomainAction {
            verb: verb.to_string(),
            headers: get_str_as_interpolated_map(header, ctx, Interpol::MultiInterpol),
            url: _run_helper::get_full_url(
                project.as_ref().map(|p| p.main_url.as_str()),
                run_action_args_url,
            )
            .into_owned(),
            path_params: parse_multiple_conf_as_opt_with_grouping_and_interpolation(
                path_params,
                ctx,
                Interpol::MultiInterpol,
            ),
            body: get_body(body.0, ctx).map(|b| b.into_owned()).map(|b| Body {
                body: b,
                url_encoded: body.1,
                form_data: body.2,
            }),
            query_params: parse_multiple_conf_as_opt_with_grouping_and_interpolation(
                query_params,
                ctx,
                Interpol::MultiInterpol,
            ),
            extract_path: get_xtracted_path(xtract_path, true, ctx),
            expect: parse_cli_conf_to_map(expect),
            pre_script: default_pre_script(),
            post_script: None,
            insecure,
            timeout,
        }
    }

    /// run python script if any on the action request
    pub fn run_hook(
        &self,
        python_script: &str,
        url: &str,
        query_params: Option<&HashMap<String, String>>,
    ) -> Result<(Request, String), PyErr> {
        // run prescript if any
        let pyrequest = run_python_pre_script(
            python_script,
            url,
            &self.verb,
            self.headers.as_ref().unwrap_or(&HashMap::new()),
            query_params,
            self.body.as_ref().map(|b| &b.body).map(Cow::from),
        );
        if pyrequest.is_err() {
            println!("Error running python script: {:?}", pyrequest);
        }
        pyrequest
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_with_tests(
        &self,
        action_opt: Option<&Action>,
        ctx: &mut HashMap<String, String>,
        db: &dyn Db,
        http: &Api,
        printer: &mut Printer,
        multi_progress: &MultiProgress,
        main_pb: &ProgressBar,
    ) -> (Vec<R>, Vec<Vec<UnaryTestResult>>) {
        let mut fetch_results = self
            .run(action_opt, db, http, printer, multi_progress)
            .await
            .into_iter()
            .map(|(url, result, script_output)| {
                // print information on screen if needed
                let _ = HttpResult {
                    fetch_result: result.as_ref(),
                    printer,
                }
                .handle_result(self.extract_path.as_ref(), ctx, main_pb);
                // returning result
                R {
                    url,
                    result,
                    script_output,
                    ctx: ctx.clone(),
                }
            })
            .collect_vec();

        if let Some(ref expected) = self.expect {
            let test_results = TestChecker {
                fetch_results: &mut fetch_results,
                ctx,
                expected,
                printer,
            }
            .check(get_action_name(action_opt), main_pb);
            return (fetch_results, test_results);
        }
        // not test returning empty values
        (fetch_results, vec![])
    }

    pub async fn run(
        &self,
        action_opt: Option<&Action>,
        db: &dyn Db,
        http: &Api,
        printer: &Printer,
        multi_progress: &MultiProgress,
    ) -> Vec<(String, anyhow::Result<http::FetchResult>, String)> {
        let computed_urls = get_computed_urls(self.path_params.as_ref(), &self.url);

        // check if it can be run
        if !self.can_be_run(&computed_urls) {
            printer.p_info(|| println!("Action cannot be run due to missing information"));
            return vec![(
                self.url.clone(),
                Err(anyhow::anyhow!(
                    "Action cannot be run due to missing information"
                )),
                "".to_string(),
            )];
        }

        // wrapping up query params
        let qp = match self.query_params.as_ref() {
            Some(query_params) => query_params.iter().map(Some).collect::<Vec<_>>(),
            None => vec![None],
        };
        future::join_all(computed_urls.iter().cartesian_product(&qp).map(
            |(computed_url, query_params)| {
                let action_cloned = action_opt.cloned();

                let (scripted_request, mut script_output) = self
                    .run_hook(
                        self.pre_script.as_ref().unwrap(),
                        computed_url,
                        *query_params,
                    )
                    .unwrap();

                // add a progress bar
                let pb = add_progress_bar_for_request(
                    multi_progress,
                    &format_query(
                        &scripted_request.verb,
                        &scripted_request.url,
                        scripted_request.query_params.as_ref(),
                    ),
                );
                pb.enable_steady_tick(Duration::from_millis(100));
                async move {
                    // fetch api
                    let fetch_result = http
                        .fetch(
                            &scripted_request.url,
                            &scripted_request.verb,
                            &scripted_request.headers,
                            scripted_request.query_params.as_ref(),
                            scripted_request.body.as_ref().map(|b| Body {
                                body: b.clone(),
                                url_encoded: self
                                    .body
                                    .as_ref()
                                    .map(|b| b.url_encoded)
                                    .unwrap_or(false),
                                form_data: self.body.as_ref().map(|b| b.form_data).unwrap_or(false),
                            }),
                        )
                        .await;
                    // save history line, let it silent if it fails
                    if let Err(e) = self
                        .insert_history_line(
                            get_action_name(action_opt),
                            computed_url,
                            fetch_result.as_ref(),
                            db,
                        )
                        .await
                    {
                        pb.println(format!("[ERROR] history line insertion failed: {}", e));
                    }

                    if action_cloned.is_some() {
                        let _ = self
                            .upsert_action(fetch_result.as_ref(), action_cloned, db)
                            .await;
                    }

                    // run post script if any
                    if let Some(post_script) = &self.post_script {
                        match fetch_result.as_ref() {
                            Ok(result) => {
                                let post_script_output =
                                    run_python_post_script(post_script, result)
                                        .unwrap_or("".to_string());
                                script_output.push_str(&post_script_output);
                            }
                            Err(e) => {
                                pb.println(format!("[ERROR] post script failed: {}", e));
                            }
                        }
                    }

                    finish_progress_bar(
                        &pb,
                        fetch_result.as_ref(),
                        &format_query(&self.verb, computed_url, *query_params),
                    );
                    // returning mixed of result etc...

                    (
                        get_full_url(computed_url, *query_params),
                        fetch_result,
                        script_output,
                    )
                }
            },
        ))
        .await
    }
}
