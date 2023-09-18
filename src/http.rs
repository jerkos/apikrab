use crossterm::style::Stylize;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::commands::run::_printer::Printer;
use crate::db::db_handler::DBHandler;
use crate::db::dto::History;

#[derive(Debug, Clone)]
pub struct FetchResult {
    pub response: String,
    pub status: u16,
    pub duration: Duration,
}

pub struct Api<'a> {
    client: reqwest::Client,
    pub db_handler: &'a DBHandler,
}

impl<'a> Api<'a> {
    pub fn new(db_handler: &'a DBHandler) -> Self {
        Self {
            client: reqwest::Client::new(),
            db_handler,
        }
    }

    pub async fn save_history_line(
        &self,
        action_name: &str,
        url: &str,
        headers: &HashMap<String, String>,
        body: &Option<String>,
        fetch_result: &FetchResult,
    ) -> anyhow::Result<()> {
        // insert history line !
        self.db_handler
            .insert_history(&History {
                id: None,
                action_name: action_name.to_string(),
                url: url.to_string(),
                body: body.clone(),
                headers: Some(serde_json::to_string(headers)?),
                response: Some(fetch_result.response.clone()),
                status_code: fetch_result.status,
                duration: fetch_result.duration.as_secs_f32(),
                timestamp: None,
            })
            .await?;

        Ok(())
    }

    pub async fn fetch(
        &self,
        action_name: &str,
        url: &str,
        verb: &str,
        headers: &HashMap<String, String>,
        query_params: &Option<HashMap<String, String>>,
        body: &Option<String>,
        printer: &Printer,
    ) -> anyhow::Result<FetchResult> {
        printer.p_info(|| {
            println!(
                "{} {}?{}",
                verb.yellow(),
                url.red(),
                query_params
                    .as_ref()
                    .map(|v| v
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<String>>()
                        .join("&"))
                    .unwrap_or("".to_string())
                    .green()
            )
        });

        let form = headers
            .get("Content-Type")
            .map(|v| v == "application/x-www-form-urlencoded");

        // building request
        let mut builder = match verb {
            "POST" => match form {
                Some(true) => self.client.post(url).form(
                    &serde_json::from_str::<HashMap<String, String>>(
                        body.as_ref()
                            .map(|v| v.as_str())
                            .expect("A body was expected"),
                    )
                    .expect("Expected body for POST request"),
                ),
                _ => self
                    .client
                    .post(url)
                    .body(body.clone().expect("Expected body for POST request")),
            },
            "GET" => self.client.get(url),
            _ => panic!("Unsupported verb: {}", verb),
        };

        // query params
        if query_params.is_some() {
            builder = builder.query(query_params.as_ref().unwrap());
        }

        // headers
        let mut header_map = HeaderMap::new();
        for (k, v) in headers {
            header_map.insert(
                k.parse::<HeaderName>().expect("Unparseable header name"),
                HeaderValue::from_bytes(v.as_bytes()).expect("Unparseable header value"),
            );
        }

        // launching request
        let start = Instant::now();
        let response = builder.headers(header_map).send().await?;
        let duration = start.elapsed();
        printer.p_info(|| println!("Request took: {:?}", duration));

        // getting status and response
        let status = response.status();
        let text: String = response.text().await?;

        let fetch_result = FetchResult {
            response: text,
            status: status.as_u16(),
            duration,
        };
        // insert history line
        self.save_history_line(action_name, url, headers, body, &fetch_result)
            .await?;

        // return results
        Ok(fetch_result)
    }
}
