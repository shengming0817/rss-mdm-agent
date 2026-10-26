//! Fixed user-scoped network checks, sharing the sole native runner and journal.
//! ref: tokio tokio/src/net/tcp/stream.rs@8a2408edcfa2b7f19c00428a9d3a8ea6652a083f (explicit bounded SocketAddr attempts).
use crate::local_execution::hash;
use agent_client::Error;
use execution_contract::*;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
pub(crate) mod compile;
pub(crate) const PROFILE: &str = "native-network-check-worker";
pub(crate) const ACTION: &str = "network.check";
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub target: NetworkTarget,
    pub run_as: RunAs,
    pub session: SessionRequirement,
    pub finish_by_unix_ms: u64,
}
/// Only a fixed service compiler can construct this admission proof.
pub(crate) struct Permit(FrozenExecution);
impl Permit {
    pub(crate) fn new(plan: FrozenExecution) -> Result<Self, Error> {
        let p = plan.spec();
        if !matches!(p.request.authority, Authority::Local { .. })
            || !matches!(p.request.initiator, Initiator::Human { .. })
            || !matches!(p.run_as, RunAs::User { .. })
            || p.launch.interpreter.profile.id.as_str() != PROFILE
            || p.request.operation.action.as_str() != ACTION
            || p.budget.max_attempts != 1
            || p.budget.total_timeout_ms != 30_000
            || p.budget.total_output_bytes != 65536
        {
            return Err(Error::Denied);
        }
        Ok(Self(plan))
    }
    pub(crate) fn verify(&self, plan: &FrozenExecution, now: u64) -> Result<(), Error> {
        if plan.digest() != self.0.digest() || now >= plan.spec().validity.expires_at_unix_ms {
            return Err(Error::Denied);
        }
        Ok(())
    }
}
pub(crate) fn normalize(target: &NetworkTarget) -> Result<NetworkTarget, Error> {
    if target.port == 0
        || target.host.is_empty()
        || target.host.len() > 253
        || target
            .host
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '/' | '\\' | '@' | '?' | '#' | '%' | ';'))
    {
        return Err(Error::Protocol);
    }
    let host = if let Ok(ip) = target.host.parse::<std::net::IpAddr>() {
        ip.to_string()
    } else {
        let host = url::Host::parse(&target.host).map_err(|_| Error::Protocol)?;
        match host {
            url::Host::Domain(d)
                if d.split('.').all(|v| {
                    !v.is_empty()
                        && v.len() <= 63
                        && !v.starts_with('-')
                        && !v.ends_with('-')
                        && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                }) =>
            {
                d
            }
            url::Host::Ipv4(ip) => ip.to_string(),
            url::Host::Ipv6(ip) => ip.to_string(),
            _ => return Err(Error::Protocol),
        }
    };
    Ok(NetworkTarget {
        host,
        port: target.port,
    })
}
pub(crate) fn resource(
    initiator: &Initiator,
    target: &NetworkTarget,
) -> Result<String, Error> {
    Ok(format!(
        "network-check:{}",
        crate::collected_data::digest(&(crate::account_identity::local_owner(initiator)?, target))?.as_str()
    ))
}
fn millis(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}
async fn probe(target: NetworkTarget) -> Result<NetworkCheckResult, Error> {
    let target = normalize(&target)?;
    let mut result = NetworkCheckResult {
        target: target.clone(),
        collected_at_unix_ms: crate::local_settings::now()?,
        dns: NetworkProbe {
            address: None,
            finding: NetworkFinding::NotRequired,
            duration_ms: 0,
        },
        tcp: vec![],
    };
    let start = Instant::now();
    let addresses: Vec<std::net::SocketAddr> =
        if let Ok(ip) = target.host.parse::<std::net::IpAddr>() {
            vec![std::net::SocketAddr::new(ip, target.port)]
        } else {
            let lookup = tokio::time::timeout(
                Duration::from_secs(5),
                tokio::net::lookup_host((target.host.as_str(), target.port)),
            )
            .await;
            result.dns.duration_ms = millis(start);
            match lookup {
                Ok(Ok(values)) => {
                    let mut addresses = Vec::new();
                    for address in values.take(16) {
                        if !addresses.contains(&address) {
                            addresses.push(address);
                        }
                        if addresses.len() == 4 {
                            break;
                        }
                    }
                    result.dns.finding = if addresses.is_empty() {
                        NetworkFinding::DnsFailed
                    } else {
                        NetworkFinding::Reachable
                    };
                    addresses
                }
                Ok(Err(_)) => {
                    result.dns.finding = NetworkFinding::DnsFailed;
                    vec![]
                }
                Err(_) => {
                    result.dns.finding = NetworkFinding::TimedOut;
                    vec![]
                }
            }
        };
    for address in addresses {
        let start = Instant::now();
        let finding = match tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::TcpStream::connect(address),
        )
        .await
        {
            Ok(Ok(stream)) => {
                drop(stream);
                NetworkFinding::Reachable
            }
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                NetworkFinding::Refused
            }
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::TimedOut => NetworkFinding::TimedOut,
            Ok(Err(_)) => NetworkFinding::Unreachable,
            Err(_) => NetworkFinding::TimedOut,
        };
        let reached = finding == NetworkFinding::Reachable;
        result.tcp.push(NetworkProbe {
            address: Some(address.to_string()),
            finding,
            duration_ms: millis(start),
        });
        if reached {
            break;
        }
    }
    Ok(result)
}
/// Fixed worker entry; only protected service material supplies its input.
pub fn run(path: &Path) -> Result<i32, Error> {
    let bytes = execution_runner::staging::read_worker_input(path, 65536)
        .map_err(crate::error::app_error)?;
    let input: Input = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    execution_runner::staging::verify_worker_identity(&input.run_as, &input.session)
        .map_err(crate::error::app_error)?;
    let remaining = input
        .finish_by_unix_ms
        .checked_sub(crate::local_settings::now()?)
        .filter(|n| *n > 0 && *n <= 30_000)
        .ok_or(Error::Expired)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let outcome = runtime.block_on(async {
        tokio::time::timeout(Duration::from_millis(remaining), probe(input.target)).await
    });
    // DNS uses blocking OS resolution. The worker process owns remaining resolver threads;
    // never wait indefinitely for them after the diagnostic budget has expired.
    runtime.shutdown_timeout(Duration::from_millis(100));
    let result = outcome.map_err(|_| Error::Expired)??;
    let output = serde_json::to_vec(&result).map_err(|_| Error::Protocol)?;
    if output.len() > 60 * 1024 {
        return Err(Error::Capacity);
    }
    std::io::stdout().write_all(&output)?;
    std::io::stdout().flush()?;
    eprintln!(
        "{}",
        serde_json::to_string(&CollectionCompletion {
            closed: true,
            input_sha256: hash(&bytes)?.as_str().into(),
            output_sha256: hash(&output)?.as_str().into(),
            native_output_bytes: output.len() as u64
        })
        .map_err(|_| Error::Protocol)?
    );
    Ok(0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destinations_are_values_not_urls_or_commands() {
        for host in [
            "https://example.com",
            "foo;touch",
            "foo bar",
            "foo:80",
            "a..b",
            "-a",
            "user@host",
        ] {
            assert!(
                normalize(&NetworkTarget {
                    host: host.into(),
                    port: 80
                })
                .is_err(),
                "{host}"
            );
        }
        for host in ["example.com", "localhost", "127.0.0.1", "::1"] {
            assert!(normalize(&NetworkTarget {
                host: host.into(),
                port: 443
            })
            .is_ok());
        }
        assert!(normalize(&NetworkTarget {
            host: "localhost".into(),
            port: 0
        })
        .is_err());
    }
    #[tokio::test]
    async fn physical_tcp_findings_distinguish_reachable_and_refused() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let target = NetworkTarget {
            host: "127.0.0.1".into(),
            port,
        };
        let result = probe(target.clone()).await.unwrap();
        assert_eq!(result.dns.finding, NetworkFinding::NotRequired);
        assert_eq!(result.tcp[0].finding, NetworkFinding::Reachable);
        drop(listener);
        assert_eq!(
            probe(target).await.unwrap().tcp[0].finding,
            NetworkFinding::Refused
        );
    }
}
