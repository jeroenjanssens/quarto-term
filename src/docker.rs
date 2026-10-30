use portable_pty::CommandBuilder;

use crate::error::TermError;
use crate::protocol::{Config, DockerConfig, PullPolicy};

pub fn check_docker_available() -> Result<(), TermError> {
    let out = std::process::Command::new("docker")
        .arg("info")
        .arg("--format")
        .arg("{{.ServerVersion}}")
        .stderr(std::process::Stdio::null())
        .output();
    match out {
        // Some Docker versions exit 0 with an empty server version when the
        // daemon is unreachable, so require both
        Ok(o) if o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty() => {
            Ok(())
        }
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
    match docker.pull {
        PullPolicy::Never => return Ok(()),
        PullPolicy::Always => {}
        PullPolicy::Missing => {
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
    // stdout carries the JSON response, so send pull progress to stderr
    pull.stdout(std::io::stderr());
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

    // Re-apply the session env through `env` so it survives image entrypoints
    // that are bash scripts: non-interactive bash drops PS1/PS2 before it
    // execs the shell, which breaks prompt detection.
    if !config.env.is_empty() {
        cmd.arg("env");
        let mut env: Vec<_> = config.env.iter().collect();
        env.sort();
        for (k, v) in env {
            cmd.arg(format!("{k}={v}"));
        }
    }

    cmd.arg(&config.shell);
    for arg in &config.shell_args {
        cmd.arg(arg);
    }

    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::BatchRequest;

    fn argv(config_json: &str) -> Vec<String> {
        let json = format!(r#"{{"config":{config_json},"cells":[]}}"#);
        let req: BatchRequest = serde_json::from_str(&json).unwrap();
        let docker = req.config.docker.clone().unwrap();
        build_docker_command(&docker, &req.config)
            .get_argv()
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn position(args: &[String], value: &str) -> usize {
        args.iter()
            .position(|a| a == value)
            .unwrap_or_else(|| panic!("{value:?} not in {args:?}"))
    }

    #[test]
    fn minimal_command() {
        let args = argv(r#"{"shell":"bash","docker":{"image":"bash:5"}}"#);
        assert_eq!(&args[..5], ["docker", "run", "--rm", "-i", "-t"]);
        assert_eq!(args.last().unwrap(), "bash");
        assert!(position(&args, "bash:5") < position(&args, "bash"));
    }

    #[test]
    fn options_become_flags() {
        let args = argv(
            r#"{"shell":"bash","docker":{
                "image":"bash:5",
                "platform":"linux/amd64",
                "name":"qt",
                "workdir":"/work",
                "user":"1000:1000",
                "network":"none",
                "memory":"512m",
                "cpus":"2",
                "ports":["8080:80"],
                "volumes":["/host:/data"],
                "args":["--init"]
            }}"#,
        );
        for (flag, value) in [
            ("--platform", "linux/amd64"),
            ("--name", "qt"),
            ("--workdir", "/work"),
            ("--user", "1000:1000"),
            ("--network", "none"),
            ("--memory", "512m"),
            ("--cpus", "2"),
            ("-p", "8080:80"),
            ("-v", "/host:/data"),
        ] {
            let i = position(&args, flag);
            assert_eq!(args[i + 1], value, "value after {flag}");
        }
        assert!(position(&args, "--init") < position(&args, "bash:5"));
    }

    #[test]
    fn shell_args_follow_shell() {
        let args = argv(
            r#"{"shell":"bash","shell_args":["--norc","--noprofile"],"docker":{"image":"bash:5"}}"#,
        );
        assert_eq!(&args[args.len() - 3..], ["bash", "--norc", "--noprofile"]);
    }

    #[test]
    fn env_passed_to_container_and_reapplied_after_entrypoint() {
        let args = argv(
            r#"{"shell":"bash","env":{"PS2":"> ","PS1":"$ "},"docker":{
                "image":"bash:5",
                "env":{"FOO":"bar"}
            }}"#,
        );
        let image = position(&args, "bash:5");
        let before_image = &args[..image];
        assert!(before_image.windows(2).any(|w| w == ["--env", "FOO=bar"]));
        assert!(before_image.windows(2).any(|w| w == ["--env", "PS1=$ "]));
        // Session env is re-applied through `env`, in sorted order, so it
        // survives entrypoints that are bash scripts.
        assert_eq!(&args[image + 1..], ["env", "PS1=$ ", "PS2=> ", "bash"]);
    }

    #[test]
    fn no_env_wrapper_without_session_env() {
        let args = argv(r#"{"shell":"bash","docker":{"image":"bash:5"}}"#);
        let image = position(&args, "bash:5");
        assert_eq!(&args[image + 1..], ["bash"]);
    }
}
