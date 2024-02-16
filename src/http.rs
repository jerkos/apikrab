use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;
use std::fs;
use std::str::FromStr;
use std::time::{Duration, Instant};
use strum::{Display, EnumString};

use reqwest::multipart::{Form, Part};
use reqwest::Method;

use crate::domain::Body;

#[derive(Debug, Clone, EnumString, Display)]
pub enum Verb {
    #[strum(serialize = "GET")]
    Get,
    #[strum(serialize = "POST")]
    Post,
    #[strum(serialize = "PUT")]
    Put,
    #[strum(serialize = "DELETE")]
    Delete,
    #[strum(serialize = "OPTIONS")]
    Options,
}

#[derive(Debug, Clone)]
pub struct FetchResult {
    pub headers: HashMap<String, String>,
    pub response: String,
    pub status: u16,
    pub duration: Duration,
}

impl FetchResult {
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

pub struct Api {
    pub(crate) client: reqwest::Client,
}

impl Api {
    pub fn new(timeout: Option<u64>, disable_cert_validation: bool) -> Self {
        Self {
            client: reqwest::ClientBuilder::new()
                .danger_accept_invalid_certs(disable_cert_validation)
                .timeout(
                    timeout
                        .map(Duration::from_secs)
                        .unwrap_or(Duration::from_secs(10)),
                )
                .build()
                .expect("Error building reqwest client"),
        }
    }

    pub async fn fetch(
        &self,
        url: &str,
        verb: &str,
        headers: &HashMap<String, String>,
        query_params: Option<&HashMap<String, String>>,
        body: Option<Body>, //(Option<Cow<'_, str>>, bool, bool),
    ) -> anyhow::Result<FetchResult> {
        // building request
        let mut builder = match Verb::from_str(verb)? {
            Verb::Post => self.client.post(url),
            Verb::Put => self.client.put(url),
            Verb::Get => self.client.get(url),
            Verb::Delete => self.client.delete(url),
            Verb::Options => self.client.request(Method::OPTIONS, url),
        };
        // query params
        if let Some(qp) = query_params.as_ref() {
            builder = builder.query(qp);
        }

        // Add custom headers
        let mut header_map = HeaderMap::new();
        for (k, v) in headers {
            header_map.insert(
                k.parse::<HeaderName>().expect("Unparseable header name"),
                HeaderValue::from_bytes(v.as_bytes()).expect("Unparseable header value"),
            );
        }
        builder = builder.headers(header_map);

        // body

        if let Some(body) = body.as_ref() {
            let is_url_encoded = body.url_encoded;
            let is_form_data = body.form_data;
            let b = &body.body;
            builder = match (is_url_encoded, is_form_data) {
                (true, true) => panic!("Cannot have both url encoded and form data"),
                (false, false) => builder.body(b.clone()),
                (true, false) => builder.form(&serde_json::from_str::<HashMap<String, String>>(b)?),
                (false, true) => {
                    let mut form = Form::new();
                    for (part_name, v) in serde_json::from_str::<HashMap<String, String>>(b)? {
                        // handle file upload
                        if v.starts_with('@') {
                            let file_path = v.trim_start_matches('@');
                            form = form.part(
                                part_name,
                                Part::bytes(fs::read(file_path)?).file_name(file_path.to_string()),
                            );
                            continue;
                        }
                        form = form.text(part_name, v);
                    }
                    builder.multipart(form)
                }
            }
        };
        // launching request
        let start = Instant::now();
        let response = builder.send().await?;
        let duration = start.elapsed();

        // getting status and response
        let response_status = response.status().as_u16();
        let response_headers = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str()
                        .map(|val| val.to_string())
                        .unwrap_or("".to_string()),
                )
            })
            .collect::<HashMap<String, String>>();
        let response_text = response.text().await?;

        let fetch_result = FetchResult {
            headers: response_headers,
            response: response_text,
            status: response_status,
            duration,
        };

        Ok(fetch_result)
    }
}
