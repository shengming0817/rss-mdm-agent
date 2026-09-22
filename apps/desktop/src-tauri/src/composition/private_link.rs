//! Fixed V1 Native/Execution lanes over inherited standard pipes.
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const MAX: usize = 512 * 1024;
const MAGIC: [u8; 4] = [82, 83, 83, 1];

pub fn start(
    mut input: impl AsyncRead + Unpin + Send + 'static,
    mut output: impl AsyncWrite + Unpin + Send + 'static,
) -> (DuplexStream, DuplexStream, CancellationToken) {
    let stop = CancellationToken::new();
    let (native, native_peer) = tokio::io::duplex(MAX * 2);
    let (execution, execution_peer) = tokio::io::duplex(MAX * 2);
    let mut incoming = Vec::new();
    let mut outgoing = Vec::new();
    for peer in [native_peer, execution_peer] {
        let (mut reader, mut writer) = tokio::io::split(peer);
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(2);
        incoming.push(tx);
        let cancelled = stop.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancelled.cancelled() => break,
                    frame = rx.recv() => match frame {
                        Some(frame) => {
                            tokio::select! {
                                _ = cancelled.cancelled() => break,
                                result = writer.write_all(&frame) => if result.is_err() { break; }
                            }
                        }
                        None => break,
                    }
                }
            }
            cancelled.cancel();
        });
        let (tx, rx) = mpsc::channel::<Vec<u8>>(2);
        outgoing.push(rx);
        let cancelled = stop.clone();
        tokio::spawn(async move {
            let mut buffer = vec![0u8; MAX];
            loop {
                let count = tokio::select! {
                    _ = cancelled.cancelled() => break,
                    read = reader.read(&mut buffer) => match read { Ok(n) if n > 0 => n, _ => break }
                };
                tokio::select! {
                    _ = cancelled.cancelled() => break,
                    sent = tx.send(buffer[..count].to_vec()) => if sent.is_err() { break; }
                }
            }
            cancelled.cancel();
        });
    }
    let cancelled = stop.clone();
    tokio::spawn(async move {
        let result = async {
            loop {
                let mut header = [0u8; 9];
                input.read_exact(&mut header).await?;
                let size = u32::from_be_bytes(header[5..9].try_into().unwrap()) as usize;
                if header[..4] != MAGIC || header[4] > 1 || size == 0 || size > MAX {
                    return Err::<(), _>(std::io::Error::other("invalid private frame"));
                }
                let mut data = vec![0; size];
                input.read_exact(&mut data).await?;
                incoming[header[4] as usize]
                    .try_send(data)
                    .map_err(|_| std::io::Error::other("private lane full"))?;
            }
        };
        tokio::select! { _ = cancelled.cancelled() => {}, _ = result => {} }
        cancelled.cancel();
    });
    let mut execution_rx = outgoing.pop().unwrap();
    let mut native_rx = outgoing.pop().unwrap();
    let cancelled = stop.clone();
    tokio::spawn(async move {
        loop {
            let (id, data) = tokio::select! {
                biased;
                _ = cancelled.cancelled() => break,
                data = native_rx.recv() => (0, data),
                data = execution_rx.recv() => (1, data),
            };
            let Some(data) = data else { break };
            let mut frame = Vec::with_capacity(data.len() + 9);
            frame.extend_from_slice(&MAGIC);
            frame.push(id);
            frame.extend_from_slice(&(data.len() as u32).to_be_bytes());
            frame.extend_from_slice(&data);
            tokio::select! {
                _ = cancelled.cancelled() => break,
                sent = output.write_all(&frame) => if sent.is_err() { break; }
            }
        }
        cancelled.cancel();
    });
    (native, execution, stop)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn protocol_matches_node_and_rejects_legacy() {
        let (wire, mut peer) = tokio::io::duplex(MAX * 2);
        let (read, write) = tokio::io::split(wire);
        let (mut native, _execution, stop) = start(read, write);
        native.write_all(b"hello").await.unwrap();
        let mut frame = [0; 14];
        peer.read_exact(&mut frame).await.unwrap();
        assert_eq!(&frame[..9], &[82, 83, 83, 1, 0, 0, 0, 0, 5]);
        assert_eq!(&frame[9..], b"hello");
        peer.write_all(b"{\"legacy\":true}\n").await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), stop.cancelled())
            .await
            .unwrap();
    }
}
