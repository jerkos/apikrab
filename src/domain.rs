use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    time::Duration,
};

use crate::{
    commands::run::{
        _progress_bar::{add_progress_bar_for_request, finish_progress_bar},
        _run_helper::{get_body, get_computed_urls, get_xtracted_path},
        action::RunActionArgs,
    },
    db::{
        db_trait::Db,
        dto::{Action, History, Project},
    },
    http::{self, Api, FetchResult},
    python::{run_python_pre_script, PyRequest},
    utils::{
        contains_interpolation, format_query, get_full_url, get_str_as_interpolated_map,
        map_contains_interpolation, parse_multiple_conf_as_opt_with_grouping_and_interpolation,
        Interpol,
    },
};

use futures::future;
use indicatif::MultiProgress;
use itertools::Itertools;
use pyo3::PyErr;

#[derive(Debug)]
pub struct DomainAction {
    pub(crate) name: String,
    pub(crate) verb: String,
    pub(crate) headers: Option<HashMap<String, String>>,
    pub(crate) urls: HashSet<String>,
    pub(crate) query_params: Vec<Option<HashMap<String, String>>>,
    pub(crate) body: (Option<String>, bool, bool),
    pub(crate) extract_path: Option<HashMap<String, Option<String>>>,
    pub(crate) run_action_args: Option<RunActionArgs>,
}

impl DomainAction {
    /// check if an action can be run
    pub fn can_be_run(&self) -> bool {
        let mut can_be_ran = true;
        if let Some(url) = self.urls.iter().next() {
            if contains_interpolation(url, Interpol::SimpleInterpol) {
                can_be_ran = false;
            }
        }
        if let Some(ref body) = self.body.0 {
            if contains_interpolation(body, Interpol::MultiInterpol) {
                can_be_ran = false;
            }
        }
        if let Some(ref header) = self.headers {
            if map_contains_interpolation(header, Interpol::MultiInterpol) {
                can_be_ran = false;
            }
        }
        if self
            .query_params
            .iter()
            .filter_map(|opt| {
                opt.as_ref()
                    .map(|vv| map_contains_interpolation(vv, Interpol::MultiInterpol))
            })
            .any(|v| v)
        {
            can_be_ran = false
        }
        can_be_ran
    }

    async fn insert_history_line(
        &self,
        computed_url: &str,
        fetch_result: anyhow::Result<&FetchResult, &anyhow::Error>,
        db: &Box<dyn Db>,
    ) -> anyhow::Result<i64> {
        let f = fetch_result.as_ref();
        db.insert_history(&History {
            id: None,
            action_name: self.name.clone(),
            url: computed_url.to_string(),
            body: self
                .body
                .0
                .as_ref()
                .map(|s| serde_json::from_str(s).unwrap()),
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
        db: &Box<dyn Db>,
    ) -> anyhow::Result<()> {
        match fetch_result.ok().zip(action_opt) {
            Some((f @ FetchResult { response, .. }, ref mut action)) => {
                if f.is_success() {
                    action.response_example = serde_json::from_str(response).ok();
                    action.body_example = self
                        .body
                        .0
                        .as_ref()
                        .and_then(|s| serde_json::from_str(s).ok());
                    return db.upsert_action(action).await;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// retrieve action from db
    /// return None if anonymous action
    /*
    async fn action_from_db<T: Db>(action_name: &str, db: &T) -> Option<Action> {
        if is_anonymous_action(action_name) {
            None
        } else {
            db.get_action(action_name).await.ok()
        }
    }
    */

    /// retrieve project from db
    /// return None if anonymous action
    /// return None if no project found
    pub async fn project_from_db(project_name: Option<&str>, db: &Box<dyn Db>) -> Option<Project> {
        match project_name.as_ref() {
            Some(p_name) => db.get_project(p_name).await.ok(),
            None => db.get_project("default").await.ok(),
        }
        /*
        if is_anonymous_action(action_name) {
            None
        } else {
            let action = Self::action_from_db(action_name, db).await;
            let project_name = action
                .as_ref()
                .and_then(|a| a.project_name.as_deref())
                .unwrap_or("__DEFAULT__");

            db.get_project(project_name).await.ok()
        }
        */
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_current_action_data(
        action_name: &str,
        verb: &str,
        run_action_args_url: &str,
        header: &str,
        body: (&str, bool, bool),
        xtract_path: &str,
        path_params: &str,
        query_params: &str,
        project: Option<&Project>,
        run_action_args: Option<RunActionArgs>,
        ctx: &HashMap<String, String>,
    ) -> DomainAction {
        DomainAction {
            name: action_name.to_string(),
            verb: verb.to_string(),
            headers: get_str_as_interpolated_map(header, ctx, Interpol::MultiInterpol),
            urls: get_computed_urls(
                path_params,
                project.as_ref().map(|p| p.main_url.as_str()),
                run_action_args_url,
                ctx,
            ),
            body: (
                get_body(body.0, ctx).map(|b| b.into_owned()),
                body.1,
                body.2,
            ),
            query_params: parse_multiple_conf_as_opt_with_grouping_and_interpolation(
                query_params,
                ctx,
                Interpol::MultiInterpol,
            ),
            extract_path: get_xtracted_path(xtract_path, true, ctx),
            run_action_args,
        }
    }

    pub fn run_hook(
        &self,
        python_script: &str,
        url: &str,
        query_params: Option<&HashMap<String, String>>,
    ) -> Result<PyRequest, PyErr> {
        // run prescript if any
        let pyrequest = run_python_pre_script(
            python_script,
            url,
            &self.verb,
            self.headers.as_ref().unwrap_or(&HashMap::new()),
            query_params,
            self.body.0.as_ref().map(Cow::from),
        );
        if pyrequest.is_err() {
            println!("Error running python script: {:?}", pyrequest);
        }
        pyrequest
    }

    pub async fn run(
        &self,
        action_opt: Option<&Action>,
        db: &Box<dyn Db>,
        http: &Api,
        multi_progress: &MultiProgress,
    ) -> Vec<(String, anyhow::Result<http::FetchResult>)> {
        future::join_all(self.urls.iter().cartesian_product(&self.query_params).map(
            |(computed_url, query_params)| {
                let action_cloned = action_opt.cloned();

                let scripted_request = self
                    .run_hook(&r#""#, computed_url, query_params.as_ref())
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
                            (
                                scripted_request.body.as_ref().map(Cow::from),
                                self.body.1,
                                self.body.2,
                            ),
                        )
                        .await;
                    // save history line, let it silent if it fails
                    if let Err(e) = self
                        .insert_history_line(computed_url, fetch_result.as_ref(), db)
                        .await
                    {
                        pb.println(format!("[ERROR] {}", e));
                    }

                    let _ = self
                        .upsert_action(fetch_result.as_ref(), action_cloned, db)
                        .await;

                    finish_progress_bar(
                        &pb,
                        fetch_result.as_ref(),
                        &format_query(&self.verb, computed_url, query_params.as_ref()),
                    );
                    // returning mixed of result etc...
                    (
                        get_full_url(computed_url, query_params.as_ref()),
                        fetch_result,
                    )
                }
            },
        ))
        .await
    }
}
