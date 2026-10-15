//! Host-only pooled HTTP transport. Source receives neither URLs nor socket capabilities.
use crate::{buffer::QuoteBuffer, error::TaskError, provider::Provider, tool::Reply};
use bytes::{Bytes, BytesMut};
use http_body_util::{BodyExt, Empty};
use hyper::{Request, Uri};
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::Semaphore;

type PooledClient = Client<hyper_rustls::HttpsConnector<HttpConnector>, Empty<Bytes>>;
/// One service pool and admission limit, reusable by independently authorized tasks.
pub struct HttpService {
    client: PooledClient,
    permits: Arc<Semaphore>,
    body_bytes: usize,
    timeout: Duration,
    copied_bytes: AtomicUsize,
    responses: AtomicUsize,
}
impl HttpService {
    pub fn new(
        in_flight: usize,
        body_bytes: usize,
        timeout: Duration,
    ) -> Result<Arc<Self>, TaskError> {
        if in_flight == 0 || body_bytes == 0 || timeout.is_zero() {
            return Err(TaskError::Input);
        }
        let connector = HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .build();
        let mut builder = Client::builder(TokioExecutor::new());
        builder.pool_max_idle_per_host(in_flight);
        builder.pool_idle_timeout(Duration::from_secs(30));
        Ok(Arc::new(Self {
            client: builder.build(connector),
            permits: Arc::new(Semaphore::new(in_flight)),
            body_bytes,
            timeout,
            copied_bytes: AtomicUsize::new(0),
            responses: AtomicUsize::new(0),
        }))
    }
    pub fn available_permits(&self) -> usize {
        self.permits.available_permits()
    }
    /// Payload bytes copied while coalescing DATA chunks, not TLS/network copies.
    pub fn copied_bytes(&self) -> usize {
        self.copied_bytes.load(Ordering::Relaxed)
    }
    pub fn responses(&self) -> usize {
        self.responses.load(Ordering::Relaxed)
    }
    async fn fetch(&self, uri: Uri, args: [i64; 2], count: usize) -> Result<Bytes, TaskError> {
        // No unbounded queue. Permit lives in the detached operation until timeout/completion.
        let _permit = self
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| TaskError::Budget)?;
        tokio::time::timeout(self.timeout, async {
            let mut request = Request::builder().uri(uri);
            for (i, arg) in args[..count].iter().enumerate() {
                request = request.header(format!("x-minipy-arg-{i}"), arg.to_string());
            }
            let request = request
                .body(Empty::<Bytes>::new())
                .map_err(|e| TaskError::Provider(e.to_string()))?;
            let response = self
                .client
                .request(request)
                .await
                .map_err(|e| TaskError::Provider(e.to_string()))?;
            if !response.status().is_success() {
                return Err(TaskError::Provider(format!(
                    "HTTP status {}",
                    response.status()
                )));
            }
            if response
                .headers()
                .get(hyper::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .is_some_and(|len| len > self.body_bytes as u64)
            {
                return Err(TaskError::Budget);
            }
            let mut body = response.into_body();
            let mut chunks = Vec::new();
            let mut total = 0usize;
            while let Some(frame) = body.frame().await {
                let frame = frame.map_err(|e| TaskError::Provider(e.to_string()))?;
                if let Ok(data) = frame.into_data() {
                    if data.is_empty() {
                        continue;
                    }
                    if data.len() > self.body_bytes.saturating_sub(total) || chunks.len() >= 4096 {
                        return Err(TaskError::Budget);
                    }
                    total += data.len();
                    chunks.push(data);
                }
            }
            let bytes = if chunks.len() == 1 {
                chunks.pop().unwrap()
            } else {
                let mut merged = BytesMut::with_capacity(total);
                for data in chunks {
                    merged.extend_from_slice(&data);
                }
                self.copied_bytes.fetch_add(total, Ordering::Relaxed);
                merged.freeze()
            };
            self.responses.fetch_add(1, Ordering::Relaxed);
            Ok(bytes)
        })
        .await
        .map_err(|_| TaskError::Provider("HTTP timeout".into()))?
    }
}
#[cfg(test)]
mod http_collect_cost_tests {
    use super::*;

    #[test]
    fn report_fetch_future_size_for_collection_candidate() {
        let service = HttpService::new(1, 1024, Duration::from_secs(1)).unwrap();
        let uri: Uri = "http://127.0.0.1/".parse().unwrap();
        let future = service.fetch(uri, [0, 0], 0);
        println!("http_fetch_future_size={}", std::mem::size_of_val(&future));
    }
}
#[derive(Clone)]
pub struct Endpoint {
    pub id: u32,
    pub uri: Uri,
    pub arguments: usize,
    /// false = checked signed-i64 response token, true = lazy JSON owner.
    pub json: bool,
}
/// A task capability set over the shared pool. Constructed only by the Host.
pub struct HttpProvider {
    service: Arc<HttpService>,
    endpoints: Vec<Endpoint>,
}
impl HttpProvider {
    pub fn new(service: Arc<HttpService>, endpoints: Vec<Endpoint>) -> Result<Self, TaskError> {
        let mut ids = std::collections::HashSet::new();
        if endpoints.len() > crate::tool::MAX_TOOLS
            || endpoints.iter().any(|e| {
                e.id < 2
                    || !ids.insert(e.id)
                    || e.arguments > 2
                    || !matches!(e.uri.scheme_str(), Some("http" | "https"))
                    || e.uri.authority().is_none()
            })
        {
            return Err(TaskError::Input);
        }
        Ok(Self { service, endpoints })
    }
}
impl Provider for HttpProvider {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
    fn allows_tool(&self, id: u32) -> bool {
        self.endpoints.iter().any(|e| e.id == id)
    }
    async fn call_tool(&self, id: u32, args: [i64; 2], count: usize) -> Result<Reply, TaskError> {
        let e = self
            .endpoints
            .iter()
            .find(|e| e.id == id)
            .ok_or(TaskError::Permission)?;
        if count != e.arguments {
            return Err(TaskError::Input);
        }
        let bytes = self.service.fetch(e.uri.clone(), args, count).await?;
        if e.json {
            Ok(Reply::Json(bytes))
        } else {
            sonic_rs::from_slice::<i64>(&bytes)
                .map(Reply::I64)
                .map_err(|_| TaskError::Json("expected signed-i64 Tool reply"))
        }
    }
}
