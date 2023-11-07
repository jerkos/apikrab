use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    time::Duration,
};

use crate::{
    commands::run::{
        _progress_bar::{add_progress_bar_for_request, finish_progress_bar},
        _run_helper::{
            get_body, get_computed_urls, get_xtracted_path, is_anonymous_action,
        },
        action::RunActionArgs,
    },
    db::{
        db_handler::DBHandler,
        dto::{Action, History, Project},
    },
    http::{self, Api, FetchResult},
    utils::{
        format_query, get_full_url, get_str_as_interpolated_map, parse_cli_conf_to_map,
        parse_multiple_conf_as_opt_with_grouping_and_interpolation, Interpol,
    },
};

use futures::future;
use indicatif::MultiProgress;
use itertools::Itertools;

#[derive(Debug)]
pub struct DomainAction {
    pub(crate) name: String,
    project_name: Option<String>,
    verb: String,
    headers: Option<HashMap<String, String>>,
    urls: HashSet<String>,
    query_params: Vec<Option<HashMap<String, String>>>,
    body: (Option<String>, bool, bool),
    pub(crate) extract_path: Option<HashMap<String, Option<String>>>,
    pub(crate) expected: Option<HashMap<String, String>>,
    pub(crate) run_action_args: Option<RunActionArgs>,
}

impl DomainAction {
    pub fn can_be_run(&self) -> bool {
        return true;
    }

    async fn insert_history_line(
        &self,
        computed_url: &str,
        fetch_result: anyhow::Result<&FetchResult, &anyhow::Error>,
        db: &DBHandler,
    ) -> anyhow::Result<i64> {
        let f = fetch_result.as_ref();
        db.insert_history(&History {
            id: None,
            action_name: self.name.clone(),
            url: computed_url.to_string(),
            body: self.body.0.as_ref().map(|s| s.to_string()),
            headers: Some(serde_json::to_string(&self.headers).unwrap()),
            response: f.map(|r| r.response.clone()).ok(),
            status_code: f.map(|r| r.status).unwrap_or(0u16),
            duration: f.map(|r| r.duration.as_secs_f32()).unwrap_or(0f32),
            timestamp: None,
        })
        .await
    }

    /// retrieve action from db
    /// return None if anonymous action
    async fn action_from_db(action_name: &str, db: &DBHandler) -> Option<Action> {
        if is_anonymous_action(action_name) {
            None
        } else {
            db.get_action(action_name).await.ok()
        }
    }

    /// retrieve project from db
    /// return None if anonymous action
    /// return None if no project found
    pub async fn project_from_db(action_name: &str, db: &DBHandler) -> Option<Project> {
        if is_anonymous_action(action_name) {
            None
        } else {
            let action = Self::action_from_db(action_name, db).await;
            let project_name = action
                .as_ref()
                .map(|a| a.project_name.as_ref().map(|p| p.as_str()))
                .flatten()
                .unwrap_or("__DEFAULT__");

            db.get_project(project_name).await.ok()
        }
    }

    pub fn from_run_args_data(
        action_name: &str,
        verb: &str,
        run_action_args_url: &str,
        header: &str,
        body: (&str, bool, bool),
        xtract_path: &str,
        path_params: &str,
        query_params: &str,
        expected: Option<&Vec<String>>,
        project: Option<&Project>,
        run_action_args: Option<RunActionArgs>,
        ctx: &HashMap<String, String>,
    ) -> DomainAction {

        DomainAction {
            name: action_name.to_string(),
            project_name: None,
            verb: verb.to_string(),
            headers: get_str_as_interpolated_map(&header, &ctx, Interpol::MultiInterpol),
            urls: get_computed_urls(
                &path_params,
                project.as_ref().map(|p| p.main_url.as_str()),
                run_action_args_url,
                ctx,
            ),
            body: (get_body(body.0, None, None, ctx).map(|b| b.into_owned()), body.1, body.2),
            query_params: parse_multiple_conf_as_opt_with_grouping_and_interpolation(
                query_params,
                &ctx,
                Interpol::MultiInterpol,
            ),
            extract_path: get_xtracted_path(&xtract_path, true, &ctx),
            expected: parse_cli_conf_to_map(expected),
            run_action_args,
        }
    }

    pub async fn run(
        &self,
        db: &DBHandler,
        http: &Api,
        ctx: &HashMap<String, String>,
        multi_progress: &MultiProgress,
    ) -> Vec<(String, anyhow::Result<http::FetchResult>)> {
        future::join_all(self.urls.iter().cartesian_product(&self.query_params).map(
            |(computed_url, query_params)| {
                // clone needed variables
                let extended_ctx = ctx.clone();

                // add a progress bar
                let pb = add_progress_bar_for_request(
                    multi_progress,
                    &format_query(&self.verb, computed_url, query_params.as_ref()),
                );
                pb.enable_steady_tick(Duration::from_millis(100));

                async move {
                    // fetch api
                    let fetch_result = http
                        .fetch(
                            computed_url,
                            &self.verb,
                            self.headers.as_ref().unwrap_or(&HashMap::new()),
                            query_params.as_ref(),
                            (self.body.0.as_ref().map(|v| Cow::from(v)), self.body.1, self.body.2),
                        )
                        .await;
                    // save history line, let it silent if it fails
                    let _ = self
                        .insert_history_line(computed_url, fetch_result.as_ref(), db)
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
