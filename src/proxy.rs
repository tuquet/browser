use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Structured diagnostic result from a proxy pre-flight probe
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProxyProbeResult {
    /// Target proxy connection URL (e.g. socks5://127.0.0.1:1080)
    pub proxy_url: String,
    /// Protocol type (socks5, http, https, unknown)
    pub protocol: String,
    /// Whether the proxy connection succeeded
    pub alive: bool,
    /// Round-trip time latency in milliseconds
    pub rtt_ms: u64,
    /// Public egress IP address seen by remote targets
    pub egress_ip: Option<String>,
    /// Two-letter ISO country code of the egress IP (e.g. VN, US, SG)
    pub country: Option<String>,
    /// Cloudflare edge data center code (e.g. HKG, SIN, SJC)
    pub colo: Option<String>,
    /// Error message if probe failed
    pub error: Option<String>,
}

pub struct ProxyProbe;

impl ProxyProbe {
    /// Detects proxy protocol from URL scheme
    pub fn detect_protocol(url: &str) -> String {
        let lower = url.trim().to_lowercase();
        if lower.starts_with("socks5://") || lower.starts_with("socks5h://") {
            "socks5".to_string()
        } else if lower.starts_with("https://") {
            "https".to_string()
        } else if lower.starts_with("http://") {
            "http".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// Probes proxy connectivity, measuring latency and resolving egress IP
    pub async fn probe(proxy_url: &str, timeout_secs: u64) -> ProxyProbeResult {
        let protocol = Self::detect_protocol(proxy_url);
        let start = Instant::now();

        // 1. Build reqwest proxy
        let proxy = match reqwest::Proxy::all(proxy_url) {
            Ok(p) => p,
            Err(e) => {
                return ProxyProbeResult {
                    proxy_url: proxy_url.to_string(),
                    protocol,
                    alive: false,
                    rtt_ms: 0,
                    egress_ip: None,
                    country: None,
                    colo: None,
                    error: Some(format!("Invalid proxy URL format: {}", e)),
                };
            }
        };

        // 2. Build HTTP client with target timeout
        let client = match reqwest::Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_secs(timeout_secs))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return ProxyProbeResult {
                    proxy_url: proxy_url.to_string(),
                    protocol,
                    alive: false,
                    rtt_ms: 0,
                    egress_ip: None,
                    country: None,
                    colo: None,
                    error: Some(format!("Failed to build HTTP proxy client: {}", e)),
                };
            }
        };

        // 3. Query Cloudflare cdn-cgi/trace
        let trace_url = "https://cloudflare.com/cdn-cgi/trace";
        match client.get(trace_url).send().await {
            Ok(resp) => {
                let rtt_ms = start.elapsed().as_millis() as u64;
                if resp.status().is_success() {
                    let text = resp.text().await.unwrap_or_default();
                    let mut ip = None;
                    let mut loc = None;
                    let mut colo = None;

                    for line in text.lines() {
                        if let Some(val) = line.strip_prefix("ip=") {
                            ip = Some(val.trim().to_string());
                        } else if let Some(val) = line.strip_prefix("loc=") {
                            loc = Some(val.trim().to_string());
                        } else if let Some(val) = line.strip_prefix("colo=") {
                            colo = Some(val.trim().to_string());
                        }
                    }

                    ProxyProbeResult {
                        proxy_url: proxy_url.to_string(),
                        protocol,
                        alive: true,
                        rtt_ms,
                        egress_ip: ip,
                        country: loc,
                        colo,
                        error: None,
                    }
                } else {
                    ProxyProbeResult {
                        proxy_url: proxy_url.to_string(),
                        protocol,
                        alive: false,
                        rtt_ms,
                        egress_ip: None,
                        country: None,
                        colo: None,
                        error: Some(format!("Proxy returned HTTP status {}", resp.status())),
                    }
                }
            }
            Err(err) => {
                let rtt_ms = start.elapsed().as_millis() as u64;
                ProxyProbeResult {
                    proxy_url: proxy_url.to_string(),
                    protocol,
                    alive: false,
                    rtt_ms,
                    egress_ip: None,
                    country: None,
                    colo: None,
                    error: Some(Self::format_error(&err)),
                }
            }
        }
    }

    fn format_error(err: &reqwest::Error) -> String {
        if err.is_timeout() {
            "Connection timed out (proxy server unreachable or slow)".to_string()
        } else if err.is_connect() {
            "Connection refused (proxy server is offline or port is closed)".to_string()
        } else {
            err.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_protocol() {
        assert_eq!(ProxyProbe::detect_protocol("socks5://127.0.0.1:1080"), "socks5");
        assert_eq!(ProxyProbe::detect_protocol("SOCKS5H://127.0.0.1:1080"), "socks5");
        assert_eq!(ProxyProbe::detect_protocol("http://127.0.0.1:8118"), "http");
        assert_eq!(ProxyProbe::detect_protocol("https://proxy.example.com:443"), "https");
        assert_eq!(ProxyProbe::detect_protocol("tcp://unknown:1234"), "unknown");
    }

    #[tokio::test]
    async fn test_probe_invalid_url() {
        let res = ProxyProbe::probe("not a valid proxy url", 1).await;
        assert!(!res.alive);
        assert!(res.error.is_some());
    }

    #[tokio::test]
    async fn test_probe_offline_proxy_fails_cleanly() {
        // Non-existent loopback port
        let res = ProxyProbe::probe("socks5://127.0.0.1:59999", 1).await;
        assert!(!res.alive);
        assert!(res.error.is_some());
    }
}
