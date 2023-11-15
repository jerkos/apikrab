use crate::commands::run::action::RunActionArgs;
use crate::db::db_handler::DBHandler;
use crate::db::dto::{Action, Project};
use async_trait::async_trait;
use openapiv3::{OpenAPI, Operation};

use super::import::Import;

pub struct OpenapiV3Importer<'a> {
    pub db_handler: &'a DBHandler,
}

impl<'a> OpenapiV3Importer<'a> {
    pub fn get_action(op: &Operation, path: &str, verb: &str, project_name: &str) -> Action {
        let run_action_args = RunActionArgs {
            url: Some(path[1..].to_string()),
            verb: Some(verb.to_string()),
            ..Default::default()
        };

        Action {
            id: None,
            name: op
                .operation_id
                .clone()
                .map(|_| format!("{}-{}", verb, path)),
            run_action_args: serde_json::to_string(&run_action_args).ok(),
            project_name: Some(project_name.to_string()),
            ..Default::default()
        }
    }
}

#[async_trait]
impl<'a> Import for OpenapiV3Importer<'a> {
    async fn import(&self, input: &str, project: &mut Project) -> anyhow::Result<()> {
        let openapi: OpenAPI = serde_json::from_str(input)?;

        // upsert project first
        self.db_handler.upsert_project(project).await?;

        // add all actions
        for (path, path_item) in openapi.paths {
            let item = path_item.as_item();

            if let Some(element) = item {
                let verbs = vec!["GET", "POST", "PUT", "DELETE"];
                let operations = vec![
                    element.get.as_ref(),
                    element.post.as_ref(),
                    element.put.as_ref(),
                    element.delete.as_ref(),
                ];

                for (verb, op) in verbs.iter().zip(operations.iter()) {
                    if let Some(op) = op {
                        let action = Self::get_action(op, &path, verb, &project.name);
                        self.db_handler.upsert_action(&action).await?;
                    }
                }
            }
        }
        Ok(())
    }
}
