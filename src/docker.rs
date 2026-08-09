use portable_pty::CommandBuilder;

use crate::error::TermError;
use crate::protocol::{Config, DockerConfig};

pub fn check_docker_available() -> Result<(), TermError> {
    let out = std::process::Command::new("docker")
        .arg("info")
        .arg("--format")
        .arg("{{.ServerVersion}}")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match out {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err(TermError::SpawnFailed(
            "docker daemon is not running or not accessible".to_string(),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(TermError::SpawnFailed(
            "docker not found in PATH".to_string(),
        )),
        Err(e) => Err(TermError::SpawnFailed(format!("docker check failed: {e}"))),
    }
}

pub fn maybe_pull_image(docker: &DockerConfig, verbose: bool) -> Result<(), TermError> {
    match docker.pull.as_str() {
        "never" => return Ok(()),
        "always" => {}
        _ => {
            let mut check = std::process::Command::new("docker");
            check.args(["image", "inspect", &docker.image]);
            check.stdout(std::process::Stdio::null());
            check.stderr(std::process::Stdio::null());
            let status = check
                .status()
                .map_err(|e| TermError::SpawnFailed(format!("docker image inspect failed: {e}")))?;
            if status.success() {
                return Ok(());
            }
        }
    }
    if verbose {
        eprintln!("quarto-term: pulling image {}", docker.image);
    }
    let mut pull = std::process::Command::new("docker");
    pull.arg("pull");
    if let Some(ref platform) = docker.platform {
        pull.arg("--platform");
        pull.arg(platform);
    }
    pull.arg(&docker.image);
    let status = pull
        .status()
        .map_err(|e| TermError::SpawnFailed(format!("docker pull failed: {e}")))?;
    if !status.success() {
        return Err(TermError::SpawnFailed(format!(
            "failed to pull image '{}': docker pull exited with {}",
            docker.image, status
        )));
    }
    Ok(())
}

pub fn build_docker_command(docker: &DockerConfig, config: &Config) -> CommandBuilder {
    let mut cmd = CommandBuilder::new("docker");
    cmd.arg("run");
    cmd.arg("--rm");
    cmd.arg("-i");
    cmd.arg("-t");

    if let Some(ref platform) = docker.platform {
        cmd.arg("--platform");
        cmd.arg(platform);
    }
    if let Some(ref name) = docker.name {
        cmd.arg("--name");
        cmd.arg(name);
    }
    if let Some(ref workdir) = docker.workdir {
        cmd.arg("--workdir");
        cmd.arg(workdir);
    }
    if let Some(ref user) = docker.user {
        cmd.arg("--user");
        cmd.arg(user);
    }
    if let Some(ref network) = docker.network {
        cmd.arg("--network");
        cmd.arg(network);
    }
    if let Some(ref memory) = docker.memory {
        cmd.arg("--memory");
        cmd.arg(memory);
    }
    if let Some(ref cpus) = docker.cpus {
        cmd.arg("--cpus");
        cmd.arg(cpus);
    }
    for port in &docker.ports {
        cmd.arg("-p");
        cmd.arg(port);
    }
    for vol in &docker.volumes {
        cmd.arg("-v");
        cmd.arg(vol);
    }
    for (k, v) in &docker.env {
        cmd.arg("--env");
        cmd.arg(format!("{k}={v}"));
    }
    for (k, v) in &config.env {
        cmd.arg("--env");
        cmd.arg(format!("{k}={v}"));
    }
    cmd.arg("--env");
    cmd.arg("TERM=xterm-256color");
    cmd.arg("--env");
    cmd.arg("COLORTERM=truecolor");
    cmd.arg("--env");
    cmd.arg("LC_ALL=en_US.UTF-8");

    for arg in &docker.args {
        cmd.arg(arg);
    }

    cmd.arg(&docker.image);
    cmd.arg(&config.shell);
    for arg in &config.shell_args {
        cmd.arg(arg);
    }

    cmd
}
