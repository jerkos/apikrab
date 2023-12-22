use crate::commands::run::action::RunActionArgs;
use crate::db::db_trait::Db;
use crate::db::dto::{Action, Project};
use async_recursion::async_recursion;
use async_trait::async_trait;
use indicatif::{MultiProgress, ProgressBar};
use postman_collection::v2_1_0::{self, Url};
use postman_collection::v2_1_0::{HeaderUnion, Items, RequestUnion};

use super::import::Import;

fn replace_postman_path(path: &str) -> &str {
    if let Some(stripped) = path.strip_prefix(':') {
        stripped
    } else {
        path
    }
}

pub struct PostmanImporter<'a, T: Db> {
    pub db_handler: &'a T,
}

impl<'a, T: Db + Send + Sync + 'a> PostmanImporter<'a, T> {
    pub fn get_url(url: &Url) -> Option<String> {
        let r = match url {
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
                            v2_1_0::PathElement::String(s) => replace_postman_path(s).to_string(),
                            v2_1_0::PathElement::PathClass(uv_cls) => {
                                uv_cls.value.as_ref().unwrap_or(&"".to_string()).to_string()
                            }
                        })
                        .collect(),
                })
                .map(|v| v.join("/"))
                .unwrap_or("".to_string()),
        };
        Some(r)
    }

    pub fn get_headers(header: &HeaderUnion) -> Option<Vec<String>> {
        let r = match header {
            HeaderUnion::HeaderArray(h_cls) => h_cls
                .iter()
                .map(|h| vec![h.key.clone(), h.value.clone()].join(":"))
                .collect(),
            HeaderUnion::String(s) => vec![s.clone()],
        };
        Some(r)
    }

    pub fn items_to_action(item: &Items, current_name: &str, project_name: &str) -> Option<Action> {
        match item.request.as_ref()? {
            RequestUnion::RequestClass(r_cls) => {
                let run_action_args = RunActionArgs {
                    url: r_cls.url.as_ref().and_then(Self::get_url),
                    verb: r_cls.method.clone(),
                    header: r_cls.header.as_ref().and_then(Self::get_headers),
                    ..Default::default()
                };
                Some(Action {
                    name: Some(current_name.to_string()),
                    run_action_args: Some(run_action_args),
                    project_name: Some(project_name.to_string()),
                    ..Default::default()
                })
            }
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
            let name: &String = item.id.as_ref().unwrap_or(fallback);
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
impl<'a, T: Db + Send + Sync> Import for PostmanImporter<'a, T> {
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
