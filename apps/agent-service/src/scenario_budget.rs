//! Bound live HTTP/SQLite scenarios without changing libtest's thread scheduling.
use std::sync::{Condvar, Mutex};

const SCENARIOS: usize = 8;
static BUDGET: Budget = Budget::new();

struct Budget {
    active: Mutex<usize>,
    released: Condvar,
}
impl Budget {
    const fn new() -> Self {
        Self {
            active: Mutex::new(0),
            released: Condvar::new(),
        }
    }

    fn enter(&self) -> Guard<'_> {
        let mut active = self.active.lock().unwrap();
        while *active == SCENARIOS {
            active = self.released.wait(active).unwrap();
        }
        *active += 1;
        Guard(self)
    }
}

/// Declare first in a test so every scenario resource is dropped before release.
pub(crate) fn enter() -> Guard<'static> {
    BUDGET.enter()
}

pub(crate) struct Guard<'a>(&'a Budget);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        *self.0.active.lock().unwrap() -= 1;
        self.0.released.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn ninth_scenario_waits_until_a_permit_is_returned() {
        let budget = Budget::new();
        let mut guards: Vec<_> = (0..SCENARIOS).map(|_| budget.enter()).collect();
        std::thread::scope(|scope| {
            let (entered, entry) = mpsc::channel();
            let (attempting, attempt) = mpsc::channel();
            let budget = &budget;
            scope.spawn(move || {
                attempting.send(()).unwrap();
                let _guard = budget.enter();
                entered.send(()).unwrap();
            });
            attempt.recv_timeout(Duration::from_secs(2)).unwrap();
            assert!(matches!(
                entry.recv_timeout(Duration::from_millis(50)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ));
            drop(guards.pop());
            entry.recv_timeout(Duration::from_secs(2)).unwrap();
        });
        drop(guards);
        assert_eq!(*budget.active.lock().unwrap(), 0);
    }

    #[test]
    fn panic_returns_the_permit_without_poisoning_the_budget() {
        let budget = Budget::new();
        let result = std::panic::catch_unwind(|| {
            let _guard = budget.enter();
            panic!("scenario failed");
        });
        assert!(result.is_err());
        let guards: Vec<_> = (0..SCENARIOS).map(|_| budget.enter()).collect();
        assert_eq!(*budget.active.lock().unwrap(), SCENARIOS);
        drop(guards);
    }

    #[tokio::test]
    async fn two_servers_close_listeners_and_paused_connections_before_release() {
        use crate::protocol_test_support::{Root, Server};
        let _budget = enter();
        for server in [Server::new().await, Server::tls().await] {
            let root = Root::new();
            let mut client = server.client(&root, agent_client::OpenMode::Create);
            server.register(&mut client).await;
            server.data.lock().unwrap().claim_pause = Some(Default::default());
            let address = server.url.socket_addrs(|| None).unwrap()[0];
            let pending = client.claim();
            tokio::pin!(pending);
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    tokio::select! {
                        result = &mut pending => panic!("request unexpectedly completed: {:?}", result.err()),
                        _ = tokio::time::sleep(Duration::from_millis(1)) => {},
                    }
                    if server.data.lock().unwrap().claim_waiting {
                        break;
                    }
                }
            })
            .await
            .unwrap();
            // Also leave an incomplete HTTP request or TLS handshake on the listener.
            let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
            drop(server);
            assert!(tokio::net::TcpStream::connect(address).await.is_err());
            assert!(tokio::time::timeout(Duration::from_secs(2), pending)
                .await
                .unwrap()
                .is_err());
            use tokio::io::AsyncReadExt;
            let mut byte = [0];
            let closed = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut byte))
                .await
                .unwrap();
            assert!(matches!(closed, Ok(0) | Err(_)));
        }
    }
}
