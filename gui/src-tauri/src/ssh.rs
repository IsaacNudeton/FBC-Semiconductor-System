//! SSH session management for fleet terminal
//!
//! Manages persistent interactive SSH sessions using russh.
//! Each session opens a PTY shell and streams I/O via Tauri events.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex};
use tauri::Emitter;

/// Commands sent from frontend to an SSH session's I/O task
pub enum SshInput {
    /// Terminal keystrokes / data
    Data(Vec<u8>),
    /// Graceful close
    Close,
}

/// Lightweight handle stored per active session
struct ActiveSession {
    host: String,
    user: String,
    input_tx: mpsc::Sender<SshInput>,
}

/// Serializable session info returned to frontend
#[derive(Debug, Clone, serde::Serialize)]
pub struct SshSessionInfo {
    pub id: u32,
    pub host: String,
    pub user: String,
}

/// Manages multiple concurrent SSH sessions.
/// All methods take `&self` — internal locking via tokio::sync::Mutex.
pub struct SshSessionManager {
    sessions: Mutex<HashMap<u32, ActiveSession>>,
    next_id: AtomicU32,
}

/// Minimal russh client handler — accept all server keys (trusted lab network)
struct SshHandler;

#[async_trait::async_trait]
impl russh::client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        self,
        _server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<(Self, bool), Self::Error> {
        Ok((self, true))
    }
}

impl SshSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            next_id: AtomicU32::new(1),
        }
    }

    /// Connect to an SSH host, open PTY shell, start streaming I/O.
    ///
    /// Returns a session_id used for subsequent write/disconnect calls.
    /// SSH output is streamed via Tauri events: `ssh:output:{session_id}`
    /// Session close is signaled via: `ssh:closed:{session_id}`
    pub async fn connect(
        &self,
        host: String,
        port: u16,
        user: String,
        password: String,
        app_handle: tauri::AppHandle,
    ) -> Result<u32, String> {
        let session_id = self.next_id.fetch_add(1, Ordering::Relaxed);

        // SSH client config — persistent session, no inactivity timeout
        let config = Arc::new(russh::client::Config {
            inactivity_timeout: None,
            ..<_>::default()
        });

        // Connect
        let mut session = russh::client::connect(
            config,
            (host.as_str(), port),
            SshHandler {},
        )
        .await
        .map_err(|e| format!("SSH connect to {}:{} failed: {}", host, port, e))?;

        // Authenticate — password or none (common for root on embedded Linux)
        if password.is_empty() {
            session
                .authenticate_none(&user)
                .await
                .map_err(|e| format!("Auth failed: {}", e))?;
        } else {
            session
                .authenticate_password(&user, &password)
                .await
                .map_err(|e| format!("Auth failed: {}", e))?;
        }

        // Open session channel → PTY → shell
        let mut channel = session
            .channel_open_session()
            .await
            .map_err(|e| format!("Channel open failed: {}", e))?;

        channel
            .request_pty(false, "xterm-256color", 80, 24, 0, 0, &[])
            .await
            .map_err(|e| format!("PTY request failed: {}", e))?;

        channel
            .request_shell(false)
            .await
            .map_err(|e| format!("Shell request failed: {}", e))?;

        // Convert channel to AsyncRead+AsyncWrite stream, split for concurrent I/O
        let stream = channel.into_stream();
        let (mut reader, mut writer) = tokio::io::split(stream);

        // Input channel from frontend
        let (input_tx, mut input_rx) = mpsc::channel::<SshInput>(256);

        // Event names for this session
        let output_event = format!("ssh:output:{}", session_id);
        let close_event = format!("ssh:closed:{}", session_id);

        // === Read task: SSH stdout → Tauri event → frontend xterm.js ===
        let read_app = app_handle.clone();
        let ev_out = output_event.clone();
        let ev_close = close_event.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => {
                        let _ = read_app.emit(&ev_close, "closed");
                        break;
                    }
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]).to_string();
                        let _ = read_app.emit(&ev_out, &text);
                    }
                    Err(e) => {
                        let _ = read_app.emit(&ev_close, format!("error: {}", e));
                        break;
                    }
                }
            }
        });

        // === Write task: frontend keystrokes → SSH stdin ===
        // Owns session handle for cleanup on close
        tokio::spawn(async move {
            while let Some(input) = input_rx.recv().await {
                match input {
                    SshInput::Data(data) => {
                        if writer.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    SshInput::Close => {
                        let _ = writer.shutdown().await;
                        break;
                    }
                }
            }
            // Disconnect SSH session cleanly
            let _ = session
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
        });

        // Store lightweight handle
        self.sessions.lock().await.insert(
            session_id,
            ActiveSession {
                host,
                user,
                input_tx,
            },
        );

        Ok(session_id)
    }

    /// Write data to a session (keystrokes from xterm.js)
    pub async fn write(&self, session_id: u32, data: String) -> Result<(), String> {
        let sessions = self.sessions.lock().await;
        let s = sessions
            .get(&session_id)
            .ok_or_else(|| format!("Session {} not found", session_id))?;
        s.input_tx
            .send(SshInput::Data(data.into_bytes()))
            .await
            .map_err(|_| "Session closed".to_string())
    }

    /// Disconnect a session
    pub async fn disconnect(&self, session_id: u32) -> Result<(), String> {
        if let Some(s) = self.sessions.lock().await.remove(&session_id) {
            let _ = s.input_tx.send(SshInput::Close).await;
        }
        Ok(())
    }

    /// List all active sessions
    pub async fn list(&self) -> Vec<SshSessionInfo> {
        self.sessions
            .lock()
            .await
            .iter()
            .map(|(id, s)| SshSessionInfo {
                id: *id,
                host: s.host.clone(),
                user: s.user.clone(),
            })
            .collect()
    }
}
