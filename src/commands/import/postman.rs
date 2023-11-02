use crate::commands::import::openapi::Import;
use crate::db::db_handler::DBHandler;
use crate::db::dto::{Action, Project};
use async_recursion::async_recursion;
use async_trait::async_trait;
use indicatif::{MultiProgress, ProgressBar};
use postman_collection::v2_1_0;
use postman_collection::v2_1_0::{HeaderUnion, Items, RequestUnion};
use std::collections::HashMap;

fn replace_postman_path(path: &str) -> &str {
    if let Some(stripped) = path.strip_prefix(':') {
        stripped
    } else {
        path
    }
}

pub struct PostmanImporter<'a> {
    pub db_handler: &'a DBHandler,
}

impl<'a> PostmanImporter<'a> {
    pub fn items_to_action(item: &Items, current_name: &str, project_name: &str) -> Option<Action> {
        item.request.as_ref()?;

        match &item.request.as_ref().unwrap() {
            RequestUnion::RequestClass(r_cls) => Some(Action {
                name: item
                    .id
                    .as_ref()
                    .map(String::from)
                    .unwrap_or(current_name.to_string()),
                url: r_cls
                    .url
                    .as_ref()
                    .map(|url| match url {
                        v2_1_0::Url::String(s) => s.clone(),
                        v2_1_0::Url::UrlClass(u_cls) => u_cls
                            .path
                            .as_ref()
                            .map(|p| match p {
                                v2_1_0::UrlPath::String(s) => s
                                    .split('/')
                                    .map(|s| replace_postman_path(s).to_string())
                                    .collect::<Vec<String>>(),
                                v2_1_0::UrlPath::UnionArray(p_cls) => p_cls
                                    .iter()
                                    .map(|p| match p {
                                        v2_1_0::PathElement::String(s) => {
                                            replace_postman_path(s).to_string()
                                        }
                                        v2_1_0::PathElement::PathClass(uv_cls) => uv_cls
                                            .value
                                            .as_ref()
                                            .unwrap_or(&"".to_string())
                                            .to_string(),
                                    })
                                    .collect(),
                            })
                            .map(|v| v.join("/"))
                            .unwrap_or("".to_string()),
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
                            let r = h_cls
                                .iter()
                                .map(|h| (h.key.clone(), h.value.clone()))
                                .collect::<HashMap<String, String>>();
                            serde_json::to_string(&r).unwrap_or("{}".to_string())
                        }
                        HeaderUnion::String(s) => s.clone(),
                    })
                    .unwrap_or("{}".to_string()),
                project_name: project_name.to_string(),
                ..Default::default()
            }),
            RequestUnion::String(_) => None,
        }
    }

    #[async_recursion]
    pub async fn insert_actions(
        &self,
        items: &[Items],
        name: &str,
        project_name: &str,
        p: Option<ProgressBar>,
    ) {
        let mut m: Option<MultiProgress> = None;
        if p.is_none() {
            m = Some(MultiProgress::new());
        }
        let p1 = p.unwrap_or_else(|| {
            m.as_ref()
                .unwrap()
                .add(indicatif::ProgressBar::new(items.len() as u64))
        });
        for (i, item) in items.iter().enumerate() {
            let fallback = &item
                .name
                .as_ref()
                .map(|n| n.to_ascii_lowercase().replace(' ', "-"))
                .unwrap_or(format!("{}-{}", name, i));
            let name = item.id.as_ref().unwrap_or(fallback);
            if item.item.is_some() {
                let sub_p = m.as_ref().unwrap().add(indicatif::ProgressBar::new(
                    item.item.as_ref().unwrap().len() as u64,
                ));
                self.insert_actions(item.item.as_ref().unwrap(), name, project_name, Some(sub_p))
                    .await;
            } else if let Some(action) = Self::items_to_action(item, name, project_name) {
                let r = self.db_handler.upsert_action(&action).await;
                if r.is_err() {
                    println!("error upserting action: {}", r.err().unwrap());
                }
            }
            p1.inc(1);
        }
    }
}

#[async_trait]
impl<'a> Import for PostmanImporter<'a> {
    async fn import(&self, input: &str, project: &mut Project) -> anyhow::Result<()> {
        let spec = serde_json::from_str::<v2_1_0::Spec>(input);
        if spec.is_err() {
            return Err(anyhow::anyhow!(
                "Error loading file or url: {}",
                spec.err().unwrap()
            ));
        }
        let collection = spec.unwrap();

        // upsert project first
        self.db_handler.upsert_project(project).await?;

        self.insert_actions(&collection.item, &collection.info.name, &project.name, None)
            .await;
        Ok(())
    }
}
