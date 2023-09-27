use crate::commands::import::openapi::Import;
use crate::db::db_handler::DBHandler;
use crate::db::dto::{Action, Project};
use async_recursion::async_recursion;
use async_trait::async_trait;
use postman_collection::v2_1_0::{HeaderUnion, Items, RequestUnion};
use postman_collection::PostmanCollection;

pub struct PostmanImporter<'a> {
    pub db_handler: &'a DBHandler,
}

impl<'a> PostmanImporter<'a> {
    pub fn items_to_action(item: &Items, current_name: &str, i: usize) -> Option<Action> {
        if item.request.is_none() {
            return None;
        }

        match &item.request.as_ref().unwrap() {
            RequestUnion::RequestClass(r_cls) => Some(Action {
                name: item
                    .id
                    .as_ref()
                    .unwrap_or(&format!("{}-{}", current_name, i))
                    .to_string(),
                url: r_cls
                    .url
                    .as_ref()
                    .map(|url| match url {
                        postman_collection::v2_1_0::Url::String(s) => s.clone(),
                        postman_collection::v2_1_0::Url::UrlClass(u_cls) => {
                            u_cls.raw.as_ref().unwrap().clone()
                        }
                    })
                    .unwrap_or("".to_string()),
                verb: r_cls
                    .method
                    .as_ref()
                    .map(String::from)
                    .unwrap_or("".to_string()),
                headers: r_cls
                    .header
                    .as_ref()
                    .map(|header| match header {
                        HeaderUnion::HeaderArray(h_cls) => {
                            serde_json::to_string(&h_cls).unwrap_or("".to_string())
                        }
                        HeaderUnion::String(s) => s.clone(),
                    })
                    .unwrap_or("{}".to_string()),
                ..Default::default()
            }),
            RequestUnion::String(_) => None,
        }
    }

    #[async_recursion]
    pub async fn insert_actions(&self, items: &Vec<Items>, name: &str) {
        for (i, item) in items.iter().enumerate() {
            let f = format!("{}-{}", name.to_string(), i);
            let name = item.id.as_ref().unwrap_or(&f);
            if item.item.is_some() {
                self.insert_actions(item.item.as_ref().unwrap(), &name)
                    .await;
            } else {
                if let Some(action) = Self::items_to_action(item, &name, i) {
                    let _ = self.db_handler.upsert_action(&action, false).await;
                }
            }
        }
    }
}

#[async_trait]
impl<'a> Import for PostmanImporter<'a> {
    async fn import(&self, input: &str, project: &mut Project) -> anyhow::Result<()> {
        let spec = postman_collection::from_reader(input.as_bytes());
        if spec.is_err() {
            return Err(anyhow::anyhow!(
                "Error loading file or url: {}",
                spec.err().unwrap()
            ));
        }
        let spec = spec.unwrap();

        // small check that we have a server url
        if project.test_url.is_none() && project.prod_url.is_none() {
            return Err(anyhow::anyhow!(
                "No test_url, prod_url or servers found in postman collection file"
            ));
        }

        // upsert project first
        self.db_handler.upsert_project(project).await?;

        // add all actions
        match spec {
            PostmanCollection::V1_0_0(collection) => {
                println!("{:?}", collection);
                return Err(anyhow::anyhow!("Postman v1 not supported"));
            }
            PostmanCollection::V2_0_0(collection) => {
                println!("{:?}", collection.info.version);
                //self.insert_actions(&collection.item, &collection.info.name).await;
            }
            PostmanCollection::V2_1_0(collection) => {
                println!("{:?}", collection.info.version);
                self.insert_actions(&collection.item, &collection.info.name)
                    .await;
            }
        }
        Ok(())
    }
}
