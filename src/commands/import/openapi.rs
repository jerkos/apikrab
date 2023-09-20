use crate::db::db_handler::DBHandler;
use crate::db::dto::{Action, Project};
use async_trait::async_trait;
use openapiv3::{OpenAPI, Operation};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

#[async_trait]
pub trait Import {
    /// load a file or url
    async fn load(&self, file_or_url: &str) -> anyhow::Result<String> {
        let mut r = File::open(file_or_url).await;

        match r {
            Ok(ref mut file) => {
                let mut contents = String::new();
                if (file.read_to_string(&mut contents).await).is_ok() {
                    Ok(contents)
                } else {
                    Err(anyhow::anyhow!(
                        "Error loading file or url: {}",
                        file_or_url
                    ))
                }
            }
            Err(_) => {
                if let Ok(text) = reqwest::get(file_or_url).await?.text().await {
                    Ok(text)
                } else {
                    Err(anyhow::anyhow!(
                        "Error loading file or url: {}",
                        file_or_url
                    ))
                }
            }
        }
    }

    /// import main function
    async fn import(&self, input: &str, project: &mut Project) -> anyhow::Result<()>;
}

pub struct OpenapiV3Importer<'a> {
    pub db_handler: &'a DBHandler,
}

impl<'a> OpenapiV3Importer<'a> {
    pub fn get_action(op: &Operation, path: &str, verb: &str) -> Action {
        Action {
            url: path[1..].to_string(),
            headers: "{}".to_string(),
            verb: verb.to_string(),
            name: op
                .operation_id
                .clone()
                .unwrap_or(format!("{}-{}", verb, path)),
            ..Default::default()
        }
    }
}

#[async_trait]
impl<'a> Import for OpenapiV3Importer<'a> {
    async fn import(&self, input: &str, project: &mut Project) -> anyhow::Result<()> {
        let openapi: OpenAPI = serde_json::from_str(input)?;

        // small check that we have a server url
        if project.test_url.is_none() && project.prod_url.is_none() && openapi.servers.is_empty() {
            return Err(anyhow::anyhow!(
                "No test_url, prod_url or servers found in openapi file"
            ));
        }

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
                        let action = Self::get_action(op, &path, verb);
                        self.db_handler.upsert_action(&action, false).await?;
                    }
                }
            }
        }
        Ok(())
    }
}
