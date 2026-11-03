use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::error::IconError;

pub type FetchFuture = Pin<Box<dyn Future<Output = Result<Vec<u8>, IconError>> + Send>>;

/// Downloads one URL's raw bytes. Object-safe via boxed futures so tests
/// can inject fakes without a network.
pub trait IconFetcher: Send + Sync {
    fn fetch(&self, url: String, max_bytes: usize) -> FetchFuture;
}

pub struct HttpFetcher {
    client: reqwest::Client,
}

impl HttpFetcher {
    pub fn new(timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(concat!("gitau/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client builds with rustls-tls");
        Self { client }
    }
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new(Duration::from_secs(10))
    }
}

impl IconFetcher for HttpFetcher {
    fn fetch(&self, url: String, max_bytes: usize) -> FetchFuture {
        let client = self.client.clone();
        Box::pin(async move {
            let mut response =
                client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|error| IconError::Network {
                        url: url.clone(),
                        message: error.to_string(),
                    })?;
            if !response.status().is_success() {
                return Err(IconError::Network {
                    url: url.clone(),
                    message: format!("HTTP {}", response.status()),
                });
            }

            if let Some(length) = response.content_length() {
                if length as usize > max_bytes {
                    return Err(IconError::TooLarge { url });
                }
            }

            // Stream with a hard cap regardless of advertised length.
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|error| IconError::Network {
                url: url.clone(),
                message: error.to_string(),
            })? {
                if bytes.len() + chunk.len() > max_bytes {
                    return Err(IconError::TooLarge { url });
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(bytes)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::sniff_image;

    /// Manual diagnostic: hits the real GitHub avatar endpoint through
    /// HttpFetcher. Run with:
    /// `cargo test -p icons http_fetcher_fetches -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "requires network"]
    async fn http_fetcher_fetches_github_avatar() {
        let fetcher = HttpFetcher::default();
        let bytes = fetcher
            .fetch(
                "https://github.com/octocat.png?size=128".to_owned(),
                5 * 1024 * 1024,
            )
            .await
            .expect("real fetch failed");
        assert_eq!(sniff_image(&bytes), Some("image/png"));
    }
}
