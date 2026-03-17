use models::SshTunnelConfig;
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};
use tokio::{
    net::TcpListener,
    process::{Child, Command},
    runtime::Handle,
    time::{Duration, sleep},
};

#[derive(Clone)]
pub struct OpenedSshTunnel {
    pub local_port: u16,
    handle: Arc<SshTunnelHandle>,
}

struct SshTunnelHandle {
    child: Mutex<Option<Child>>,
}

static SSH_TUNNELS: OnceLock<Mutex<HashMap<String, Arc<SshTunnelHandle>>>> = OnceLock::new();

pub async fn open_ssh_tunnel(
    config: &SshTunnelConfig,
    remote_host: &str,
    remote_port: u16,
) -> Result<OpenedSshTunnel, String> {
    if !config.is_configured() {
        return Err("SSH tunnel requires host and username".to_string());
    }

    let ssh_host = config.host.trim();
    let ssh_user = config.username.trim();
    let remote_host = remote_host.trim();
    if remote_host.is_empty() {
        return Err("Database host is empty, nothing to tunnel to".to_string());
    }

    let local_port = allocate_local_port().await?;
    let forward_spec = format!("127.0.0.1:{local_port}:{remote_host}:{remote_port}");

    let mut command = Command::new("ssh");
    command
        .kill_on_drop(true)
        .arg("-N")
        .arg("-L")
        .arg(forward_spec)
        .arg("-p")
        .arg(config.effective_port().to_string())
        .arg("-l")
        .arg(ssh_user)
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ExitOnForwardFailure=yes")
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg("-o")
        .arg("ServerAliveInterval=30")
        .arg("-o")
        .arg("ServerAliveCountMax=3");

    if !config.private_key_path.trim().is_empty() {
        command
            .arg("-i")
            .arg(config.private_key_path.trim())
            .arg("-o")
            .arg("IdentitiesOnly=yes");
    }

    command
        .arg(ssh_host)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to start ssh tunnel process: {err}"))?;

    for _ in 0..12 {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child
                    .wait_with_output()
                    .await
                    .map_err(|err| format!("failed to read ssh tunnel error output: {err}"))?;
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let details = if !stderr.is_empty() {
                    stderr
                } else if !stdout.is_empty() {
                    stdout
                } else {
                    format!("ssh exited with status {status}")
                };
                return Err(format!("ssh tunnel failed: {details}"));
            }
            Ok(None) => sleep(Duration::from_millis(150)).await,
            Err(err) => return Err(format!("failed to monitor ssh tunnel process: {err}")),
        }
    }

    Ok(OpenedSshTunnel {
        local_port,
        handle: Arc::new(SshTunnelHandle {
            child: Mutex::new(Some(child)),
        }),
    })
}

pub fn register_ssh_tunnel(session_name: String, tunnel: OpenedSshTunnel) {
    let mut tunnels = ssh_tunnels_guard();
    if let Some(previous) = tunnels.insert(session_name, tunnel.handle) {
        shutdown_tunnel(previous);
    }
}

pub fn release_ssh_tunnel(session_name: &str) {
    let mut tunnels = ssh_tunnels_guard();
    if let Some(tunnel) = tunnels.remove(session_name) {
        shutdown_tunnel(tunnel);
    }
}

async fn allocate_local_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| format!("failed to allocate local SSH tunnel port: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("failed to resolve local SSH tunnel port: {err}"))?
        .port();
    drop(listener);
    Ok(port)
}

fn ssh_tunnels() -> &'static Mutex<HashMap<String, Arc<SshTunnelHandle>>> {
    SSH_TUNNELS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ssh_tunnels_guard() -> MutexGuard<'static, HashMap<String, Arc<SshTunnelHandle>>> {
    match ssh_tunnels().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn shutdown_tunnel(tunnel: Arc<SshTunnelHandle>) {
    let child = match tunnel.child.lock() {
        Ok(mut guard) => guard.take(),
        Err(_) => None,
    };

    let Some(mut child) = child else {
        return;
    };

    let _ = child.start_kill();
    if let Ok(handle) = Handle::try_current() {
        handle.spawn(async move {
            let _ = child.wait().await;
        });
    }
}

impl Drop for SshTunnelHandle {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.child.lock()
            && let Some(child) = guard.as_mut()
        {
            let _ = child.start_kill();
        }
    }
}
