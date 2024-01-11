use async_trait::async_trait;
use tokio::{fs::File, io::AsyncReadExt};

use crate::db::dto::Project;

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
