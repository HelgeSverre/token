//! On-demand ownership of one local llama-server. Only the inline worker
//! touches the child; generation cancellation does not discard this owner.

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::process::Stdio;
use std::time::Duration;

use reqwest::{Client, Url};
use token::completion::provider::ProviderError;
use token::config::ProviderConfig;
use tokio::process::{Child, Command};
use tokio::time::Instant;

const POLL: Duration = Duration::from_millis(200);

struct Running {
    child: Child,
    deadline: Instant,
    ready: bool,
}

#[derive(Default)]
pub(super) struct InlineServer {
    config: Option<ProviderConfig>,
    running: Option<Running>,
    failed: bool,
}

impl InlineServer {
    pub(super) async fn configure(&mut self, config: Option<&ProviderConfig>) {
        if self.config.as_ref() != config {
            self.stop().await;
            self.config = config.cloned();
            self.failed = false;
        }
    }

    pub(super) fn running(&self) -> bool {
        self.running.is_some()
    }

    pub(super) async fn stop(&mut self) {
        if let Some(mut running) = self.running.take() {
            // kill() also reaps. The drop guard remains the fallback if the OS
            // cannot complete teardown within the editor's exit budget.
            let _ = tokio::time::timeout(Duration::from_secs(1), running.child.kill()).await;
        }
    }

    /// Reap crashes and enforce startup limits even after generation is canceled.
    pub(super) async fn poll(&mut self) -> Result<(), ProviderError> {
        let error = self
            .running
            .as_mut()
            .and_then(|running| match running.child.try_wait() {
                Ok(None) if !running.ready && Instant::now() >= running.deadline => {
                    Some("startup timed out; check the model and retry explicitly")
                }
                Ok(None) => None,
                _ => Some("process exited; check the executable/model and retry explicitly"),
            });
        if let Some(error) = error {
            self.failed = true;
            self.stop().await;
            return Err(ProviderError::LocalServer(error));
        }
        Ok(())
    }

    pub(super) async fn ready(
        &mut self,
        client: &Client,
        explicit: bool,
    ) -> Result<(), ProviderError> {
        if let Err(error) = self.poll().await {
            if !explicit {
                return Err(error);
            }
        }
        if self.running.as_ref().is_some_and(|running| running.ready) {
            return Ok(());
        }
        let Some(config) = &self.config else {
            return Ok(());
        };
        let (mut command, health) = command(config)?;
        let local = config
            .local_server
            .as_ref()
            .ok_or(ProviderError::LocalServer(
                "missing local_server configuration",
            ))?;
        if self.running.is_none() {
            if self.failed && !explicit {
                return Err(ProviderError::LocalServer(
                    "startup stopped after failure; retry explicitly",
                ));
            }
            self.failed = true;
            // Refuse to adopt, replace or kill an existing listener. This probe
            // is released before spawn because llama-server binds its own socket.
            let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, health.port().unwrap_or(80));
            drop(TcpListener::bind(address).map_err(|_| {
                ProviderError::LocalServer(
                    "port is occupied or unavailable; choose another URL port",
                )
            })?);
            let child = command.spawn().map_err(|_| {
                ProviderError::LocalServer(
                    "cannot spawn executable; check its path and permissions",
                )
            })?;
            self.running = Some(Running {
                child,
                deadline: Instant::now() + Duration::from_millis(local.startup_timeout_ms),
                ready: false,
            });
            self.failed = false;
        }

        loop {
            self.poll().await?;
            let Some(running) = &self.running else {
                return Err(ProviderError::LocalServer("process is unavailable"));
            };
            if running.ready {
                return Ok(());
            }
            // Status alone is sufficient; do not buffer backend bodies or log
            // them. The shared client disables redirects and environment proxies.
            if let Ok(response) = client.get(health.clone()).timeout(POLL).send().await {
                if response.status() == reqwest::StatusCode::OK {
                    self.poll().await?;
                    if let Some(running) = &mut self.running {
                        running.ready = true;
                    }
                    return Ok(());
                }
            }
            tokio::time::sleep(POLL).await;
        }
    }
}

fn command(config: &ProviderConfig) -> Result<(Command, Url), ProviderError> {
    token::completion::fim::validate_config(config)?;
    let invalid = |message| ProviderError::Configuration(message);
    let local = config
        .local_server
        .as_ref()
        .ok_or_else(|| invalid("missing local_server"))?;
    let mut url = Url::parse(&config.url).map_err(|_| invalid("invalid local server URL"))?;
    let mut command = Command::new(&local.executable);
    command
        .arg("--model")
        .arg(&local.model_path)
        .args(["--host", "127.0.0.1", "--port"])
        .arg(url.port().unwrap_or(80).to_string())
        .arg("--ctx-size")
        .arg(local.context_size.to_string())
        .arg("--offline")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    if let Some(layers) = local.gpu_layers {
        command.arg("--n-gpu-layers").arg(layers.to_string());
    }
    // Do not inherit llama.cpp settings that can select other models, enable
    // network tools, change listeners, or write logs. Ordinary OS/GPU environment
    // stays available. The configured arguments are the entire server policy.
    for (name, _) in std::env::vars_os() {
        if name
            .to_str()
            .and_then(|name| name.get(..6))
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("LLAMA_"))
        {
            command.env_remove(name);
        }
    }
    if let Some(name) = &config.api_key_env {
        if let Some(value) = std::env::var_os(name) {
            command.env("LLAMA_API_KEY", value);
        }
    }
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    url.set_path("/health");
    Ok((command, url))
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::config::TransportKind;

    #[test]
    fn managed_configuration_is_opt_in_round_trips_and_stays_local() {
        let config = ProviderConfig::default();
        assert!(config.local_server.is_none());
        let mut config: ProviderConfig = serde_yaml::from_str(
            "local_server: {executable: /tools/llama-server, model_path: /models/code.gguf}",
        )
        .unwrap();
        #[cfg(windows)]
        {
            let local = config.local_server.as_mut().unwrap();
            local.executable = r"C:\tools\llama-server.exe".into();
            local.model_path = r"C:\models\code.gguf".into();
        }
        assert_eq!(
            serde_yaml::from_str::<ProviderConfig>(&serde_yaml::to_string(&config).unwrap())
                .unwrap(),
            config
        );
        let (cmd, health) = command(&config).unwrap();
        assert_eq!(health.as_str(), "http://127.0.0.1:8012/health");
        assert!(cmd.as_std().get_args().any(|arg| arg == "--offline"));
        for url in [
            "http://localhost:8012",
            "https://127.0.0.1:8012",
            "http://192.0.2.1:8012",
            "http://127.0.0.1:8012/proxy",
            "http://127.0.0.1:0",
            "http://user@127.0.0.1:8012",
            "http://127.0.0.1:8012?token=secret",
        ] {
            config.url = url.into();
            assert!(command(&config).is_err(), "{url}");
        }
        config.url = ProviderConfig::default().url;
        config.transport = TransportKind::Ollama;
        assert!(command(&config).is_err());
        config.transport = TransportKind::LlamaCpp;
        config.local_server.as_mut().unwrap().executable = "relative-server".into();
        assert!(command(&config).is_err());
    }
}
