use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};

use reqwest::multipart::{Form, Part};
use reqwest::Method;

#[derive(Debug, Clone)]
pub struct FetchResult {
    pub response: String,
    pub status: u16,
    pub duration: Duration,
}

impl FetchResult {
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

static URL_ENCODED: &str = "application/x-www-form-urlencoded";
static FORM_DATA: &str = "multipart/form-data";
static APPLICATION_JSON: &str = "application/json";

pub struct Api {
    pub(crate) client: reqwest::Client,
}

impl Api {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn fetch(
        &self,
        url: &str,
        verb: &str,
        headers: &HashMap<String, String>,
        query_params: Option<&HashMap<String, String>>,
        body: Option<&Cow<'_, str>>,
    ) -> anyhow::Result<FetchResult> {
        // building request
        let mut builder = match verb {
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "GET" => self.client.get(url),
            "DELETE" => self.client.delete(url),
            "OPTIONS" => self.client.request(Method::OPTIONS, url),
            _ => panic!("Unsupported verb: {}", verb),
        };

        // query params
        if query_params.is_some() {
            builder = builder.query(query_params.as_ref().unwrap());
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
        let content_type = headers
            .get::<str>(reqwest::header::CONTENT_TYPE.as_ref())
            .cloned()
            .unwrap_or(APPLICATION_JSON.to_string());
        let is_url_encoded = content_type == URL_ENCODED;
        let is_form_data = content_type == FORM_DATA;

        builder = match (is_url_encoded, is_form_data) {
            (true, true) => panic!("Cannot have both url encoded and form data"),
            (false, false) => {
                if body.is_some() {
                    builder = builder.body(body.as_ref().unwrap().to_string());
                }
                builder
            }
            (true, false) => builder.form(&serde_json::from_str::<HashMap<String, String>>(
                body.as_ref().unwrap(),
            )?),
            (false, true) => {
                let mut form = Form::new();
                let body = body.as_ref().unwrap();
                for (part_name, v) in serde_json::from_str::<HashMap<String, String>>(body)? {
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
        };

        // launching request
        let start = Instant::now();
        let response = builder.send().await?;
        let duration = start.elapsed();

        // getting status and response
        let status = response.status();
        let text: String = response.text().await?;

        let fetch_result = FetchResult {
            response: text,
            status: status.as_u16(),
            duration,
        };

        // return results
        Ok(fetch_result)
    }
}
