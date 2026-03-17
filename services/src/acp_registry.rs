use models::{AcpLaunchRequest, AcpRegistryAgent};
use serde::Deserialize;
use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

const ACP_REGISTRY_URL: &str =
    "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";

#[derive(Debug, Deserialize)]
struct RegistryDocument {
    agents: Vec<RegistryAgentRecord>,
}

#[derive(Debug, Deserialize)]
struct RegistryAgentRecord {
    id: String,
    name: String,
    version: String,
    description: String,
    distribution: RegistryDistribution,
}

#[derive(Debug, Deserialize)]
struct RegistryDistribution {
    #[serde(default)]
    binary: std::collections::HashMap<String, RegistryBinaryTarget>,
}

#[derive(Clone, Debug, Deserialize)]
struct RegistryBinaryTarget {
    archive: String,
    cmd: String,
    #[serde(default)]
    args: Vec<String>,
}

pub async fn load_acp_registry_agents() -> Result<Vec<AcpRegistryAgent>, String> {
    let registry = fetch_registry().await?;
    let platform_key = current_platform_key()?;

    let mut agents = registry
        .agents
        .into_iter()
        .filter_map(|agent| {
            let target = agent.distribution.binary.get(&platform_key)?;
            let install_root = install_root(&agent.id, &agent.version).ok()?;
            let command_path = resolve_installed_command(&install_root, &target.cmd);
            Some(AcpRegistryAgent {
                id: agent.id,
                name: agent.name,
                version: agent.version,
                description: agent.description,
                installed: command_path.is_some(),
            })
        })
        .collect::<Vec<_>>();

    agents.sort_by(|left, right| {
        (right.id == "opencode")
            .cmp(&(left.id == "opencode"))
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(agents)
}

pub async fn install_acp_registry_agent(
    agent_id: String,
    cwd: String,
) -> Result<AcpLaunchRequest, String> {
    let registry = fetch_registry().await?;
    let platform_key = current_platform_key()?;
    let agent = registry
        .agents
        .into_iter()
        .find(|agent| agent.id == agent_id)
        .ok_or_else(|| format!("ACP registry agent `{agent_id}` not found"))?;
    let target = agent
        .distribution
        .binary
        .get(&platform_key)
        .cloned()
        .ok_or_else(|| {
            format!(
                "ACP agent `{}` is not available for {platform_key}",
                agent.id
            )
        })?;

    let install_root = install_root(&agent.id, &agent.version)?;
    let command_path = match resolve_installed_command(&install_root, &target.cmd) {
        Some(command_path) => command_path,
        None => {
            download_and_extract(&target.archive, &install_root).await?;
            let command_path =
                resolve_installed_command(&install_root, &target.cmd).ok_or_else(|| {
                    format!(
                        "Installed ACP agent `{}` but command was not found",
                        agent.id
                    )
                })?;
            ensure_executable(&command_path)?;
            command_path
        }
    };

    Ok(AcpLaunchRequest {
        command: command_path.display().to_string(),
        args: join_args(&target.args),
        cwd,
    })
}

async fn fetch_registry() -> Result<RegistryDocument, String> {
    reqwest::get(ACP_REGISTRY_URL)
        .await
        .map_err(|err| format!("Failed to fetch ACP registry: {err}"))?
        .error_for_status()
        .map_err(|err| format!("ACP registry returned an error: {err}"))?
        .json::<RegistryDocument>()
        .await
        .map_err(|err| format!("Failed to parse ACP registry: {err}"))
}

fn current_platform_key() -> Result<String, String> {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "windows",
        other => return Err(format!("ACP registry does not support OS `{other}`")),
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        other => {
            return Err(format!(
                "ACP registry does not support architecture `{other}`"
            ));
        }
    };
    Ok(format!("{os}-{arch}"))
}

fn install_root(agent_id: &str, version: &str) -> Result<PathBuf, String> {
    let base_dir = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .ok_or_else(|| "Failed to resolve local data directory".to_string())?;
    Ok(base_dir
        .join("showel")
        .join("acp")
        .join("registry")
        .join(agent_id)
        .join(version))
}

fn resolve_installed_command(install_root: &Path, cmd: &str) -> Option<PathBuf> {
    let relative = cmd.strip_prefix("./").unwrap_or(cmd);
    let direct = install_root.join(relative);
    if direct.is_file() {
        return Some(direct);
    }

    let file_name = Path::new(relative).file_name()?.to_owned();
    walkdir::WalkDir::new(install_root)
        .into_iter()
        .filter_map(Result::ok)
        .find_map(|entry| {
            if entry.file_type().is_file() && entry.file_name() == file_name {
                Some(entry.into_path())
            } else {
                None
            }
        })
}

async fn download_and_extract(archive_url: &str, install_root: &Path) -> Result<(), String> {
    let bytes = reqwest::get(archive_url)
        .await
        .map_err(|err| format!("Failed to download ACP agent archive: {err}"))?
        .error_for_status()
        .map_err(|err| format!("ACP agent download failed: {err}"))?
        .bytes()
        .await
        .map_err(|err| format!("Failed to read ACP agent archive: {err}"))?;

    let install_root = install_root.to_path_buf();
    let archive_url = archive_url.to_string();
    tokio::task::spawn_blocking(move || extract_archive(&archive_url, &bytes, &install_root))
        .await
        .map_err(|err| format!("ACP agent installation task failed: {err}"))?
}

fn extract_archive(archive_url: &str, bytes: &[u8], install_root: &Path) -> Result<(), String> {
    if install_root.exists() {
        std::fs::remove_dir_all(install_root)
            .map_err(|err| format!("Failed to reset ACP install directory: {err}"))?;
    }
    std::fs::create_dir_all(install_root)
        .map_err(|err| format!("Failed to create ACP install directory: {err}"))?;

    if archive_url.ends_with(".tar.gz") {
        let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes));
        let mut archive = tar::Archive::new(decoder);
        for entry in archive
            .entries()
            .map_err(|err| format!("Failed to read tar archive: {err}"))?
        {
            let mut entry = entry.map_err(|err| format!("Failed to read tar entry: {err}"))?;
            entry
                .unpack_in(install_root)
                .map_err(|err| format!("Failed to extract tar entry: {err}"))?;
        }
        return Ok(());
    }

    if archive_url.ends_with(".zip") {
        let reader = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|err| format!("Failed to read zip archive: {err}"))?;

        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|err| format!("Failed to read zip entry: {err}"))?;
            let Some(path) = entry.enclosed_name().map(|path| path.to_path_buf()) else {
                continue;
            };
            let target = install_root.join(path);

            if entry.is_dir() {
                std::fs::create_dir_all(&target)
                    .map_err(|err| format!("Failed to create zip directory: {err}"))?;
                continue;
            }

            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("Failed to create zip parent directory: {err}"))?;
            }

            let mut file = std::fs::File::create(&target)
                .map_err(|err| format!("Failed to create extracted file: {err}"))?;
            std::io::copy(&mut entry, &mut file)
                .map_err(|err| format!("Failed to extract zip file: {err}"))?;
        }
        return Ok(());
    }

    Err(format!("Unsupported ACP archive format: {archive_url}"))
}

fn join_args(args: &[String]) -> String {
    args.iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_escape(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if arg
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '/' | '.'))
    {
        return arg.to_string();
    }

    format!("'{}'", arg.replace('\'', "'\"'\"'"))
}

fn ensure_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = std::fs::metadata(path)
            .map_err(|err| format!("Failed to inspect installed ACP command: {err}"))?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions)
            .map_err(|err| format!("Failed to mark ACP command executable: {err}"))?;
    }

    Ok(())
}
