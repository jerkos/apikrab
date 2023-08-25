pub struct Api {
    client: reqwest::Client,
}

impl Api {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn fetch(
        &self,
        url: &str,
        api_key: &str,
        verb: &str,
        body: &Option<String>,
    ) -> anyhow::Result<String> {
        let builder = match verb {
            "POST" => self
                .client
                .post(url)
                .body(body.clone().expect("Expected body for POST request")),
            "GET" => self.client.get(url),
            _ => panic!("Unsupported verb: {}", verb),
        };
        let response = builder.header("apiKey", api_key).send().await?;
        response.error_for_status_ref()?;
        let text: String = response.text().await?;
        Ok(text)
    }
}
