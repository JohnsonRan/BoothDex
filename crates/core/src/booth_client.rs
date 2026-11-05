//! Booth HTTP access shared by the worker (crawls, patient retries) and the API (refresh-on-open, one try):
//! one user agent, one retry/backoff policy, request counters.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130 Safari/537.36";

#[derive(Default)]
pub struct Counters {
    pub requests: AtomicU64,
    pub errors: AtomicU64,
    pub rate_limited: AtomicU64,
}

pub struct BoothClient {
    client: reqwest::Client,
    attempts: u32,
    pub counters: Counters,
}

impl BoothClient {
    /// `attempts` >= 1; 429, 5xx and network errors are retried with backoff until it runs out.
    pub fn new(attempts: u32, timeout: Duration) -> Result<Self> {
        Ok(Self::from_reqwest(reqwest::Client::builder().user_agent(UA).timeout(timeout).build()?, attempts))
    }

    pub fn from_reqwest(client: reqwest::Client, attempts: u32) -> Self {
        Self { client, attempts: attempts.max(1), counters: Counters::default() }
    }

    /// GET with retry/backoff on 429, 5xx and network errors. `Ok(None)` = 404.
    pub async fn get(&self, url: &str) -> Result<Option<String>> {
        let last = self.attempts - 1;
        for attempt in 0..self.attempts {
            self.counters.requests.fetch_add(1, Ordering::Relaxed);
            match self.client.get(url).send().await {
                Ok(r) if r.status() == 404 => return Ok(None),
                Ok(r) if r.status().is_success() => return Ok(Some(r.text().await?)),
                Ok(r) if r.status() == 429 || r.status().is_server_error() => {
                    if r.status() == 429 {
                        self.counters.rate_limited.fetch_add(1, Ordering::Relaxed);
                    } else {
                        self.counters.errors.fetch_add(1, Ordering::Relaxed);
                    }
                    if attempt == last {
                        break;
                    }
                    let wait = r.headers().get("retry-after").and_then(|v| v.to_str().ok()?.parse().ok()).unwrap_or(2u64.pow(attempt + 1));
                    tokio::time::sleep(Duration::from_secs(wait.min(300))).await;
                }
                Ok(r) => {
                    self.counters.errors.fetch_add(1, Ordering::Relaxed);
                    bail!("{} on {url}", r.status());
                }
                Err(e) => {
                    self.counters.errors.fetch_add(1, Ordering::Relaxed);
                    if attempt == last {
                        bail!("{e:?} on {url}");
                    }
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt + 1))).await;
                }
            }
        }
        bail!("giving up on {url}")
    }

    /// Item JSON from the English site, so the category name matches the archive ("3D Clothing").
    /// `Ok(None)` = Booth says the item is gone.
    pub async fn item(&self, id: i64) -> Result<Option<Value>> {
        match self.get(&format!("https://booth.pm/en/items/{id}.json")).await? {
            Some(body) => Ok(Some(serde_json::from_str(&body).with_context(|| format!("item {id}"))?)),
            None => Ok(None),
        }
    }
}
