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

/// The image's ENTRYPOINT and CMD, as reported by `docker image inspect`.
pub fn image_entrypoint_and_cmd(image: &str) -> Option<(Vec<String>, Vec<String>)> {
    let out = std::process::Command::new("docker")
        .args(["image", "inspect", "--format", "{{json .Config}}", image])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let config: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let list = |key: &str| -> Vec<String> {
        config[key]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };
    Some((list("Entrypoint"), list("Cmd")))
}

/// Explain a failed Docker session start in terms of the image's
/// ENTRYPOINT/CMD and the `shell` / `docker.entrypoint` options.
pub fn startup_hint(docker: &DockerConfig, config: &Config) -> Option<String> {
    let (entrypoint, cmd) = image_entrypoint_and_cmd(&docker.image)?;
    startup_hint_for(docker, config, &entrypoint, &cmd)
}

fn startup_hint_for(
    docker: &DockerConfig,
    config: &Config,
    entrypoint: &[String],
    cmd: &[String],
) -> Option<String> {
    let image_entrypoint_used = docker.entrypoint.is_none() && !entrypoint.is_empty();
    match config.shell {
        Some(ref shell) if image_entrypoint_used => Some(format!(
            "image {} has ENTRYPOINT {:?}, which receives `{}` as arguments; \
             remove `shell` to start the image's own shell, or set `docker.entrypoint:` \
             (empty) to clear the entrypoint",
            docker.image, entrypoint, shell
        )),
        Some(_) => None,
        None => {
            let mut command: Vec<String> = Vec::new();
            if docker.entrypoint.is_none() {
                command.extend(entrypoint.iter().cloned());
            } else if let Some(ref e) = docker.entrypoint {
                if !e.is_empty() {
                    command.push(e.clone());
                }
            }
            command.extend(cmd.iter().cloned());
            Some(format!(
                "no `shell` is set, so the image's default command ran: {:?}; \
                 set `shell` (or `profile`) if that isn't an interactive shell, and \
                 `prompt` or `prompt-regex` if its prompt isn't {:?}",
                command, config.prompt
            ))
        }
    }
}

/// Build the `docker run` command.
///
/// `image_cmd` is the image's CMD; it's only used when no `shell` is set.
pub fn build_docker_command(
    docker: &DockerConfig,
    config: &Config,
    image_cmd: Option<&[String]>,
) -> CommandBuilder {
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

    if let Some(ref entrypoint) = docker.entrypoint {
        cmd.arg(format!("--entrypoint={entrypoint}"));
    }

    for arg in &docker.args {
        cmd.arg(arg);
    }

    cmd.arg(&docker.image);

    // The command after the image: the configured shell, or (without one)
    // the image's own CMD, which then runs through its ENTRYPOINT as usual.
    // Without a shell and without a CMD, nothing is appended.
    let command: Vec<String> = match config.shell {
        Some(ref shell) => std::iter::once(shell.clone())
            .chain(config.shell_args.iter().cloned())
            .collect(),
        None => image_cmd.map(|c| c.to_vec()).unwrap_or_default(),
    };
    if command.is_empty() {
        return cmd;
    }

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

    for arg in command {
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
        build_docker_command(&docker, &req.config, None)
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
    fn no_shell_uses_image_default_command() {
        // Without a shell, nothing follows the image: its ENTRYPOINT/CMD runs
        let args = argv(r#"{"env":{"PS1":"$ "},"docker":{"image":"bash:5"}}"#);
        assert_eq!(args.last().unwrap(), "bash:5");
        // The session env is still passed with --env
        let image = position(&args, "bash:5");
        assert!(args[..image].windows(2).any(|w| w == ["--env", "PS1=$ "]));
    }

    #[test]
    fn no_shell_wraps_image_cmd_with_session_env() {
        // The image's CMD runs through `env` so PS1/PS2 survive entrypoint scripts
        let json = r#"{"config":{"env":{"PS1":"$ "},"docker":{"image":"bash:5"}},"cells":[]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        let docker = req.config.docker.clone().unwrap();
        let cmd = vec!["bash".to_string()];
        let args: Vec<String> = build_docker_command(&docker, &req.config, Some(&cmd))
            .get_argv()
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let image = position(&args, "bash:5");
        assert_eq!(&args[image + 1..], ["env", "PS1=$ ", "bash"]);
    }

    #[test]
    fn explicit_shell_ignores_image_cmd() {
        let json = r#"{"config":{"shell":"sh","docker":{"image":"bash:5"}},"cells":[]}"#;
        let req: BatchRequest = serde_json::from_str(json).unwrap();
        let docker = req.config.docker.clone().unwrap();
        let cmd = vec!["bash".to_string()];
        let args: Vec<String> = build_docker_command(&docker, &req.config, Some(&cmd))
            .get_argv()
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args.last().unwrap(), "sh");
    }

    #[test]
    fn empty_entrypoint_clears_image_entrypoint() {
        let args = argv(r#"{"docker":{"image":"bash:5","entrypoint":"","args":["--init"]}}"#);
        let flag = position(&args, "--entrypoint=");
        assert!(flag < position(&args, "--init"), "entrypoint comes before docker args");
        assert!(flag < position(&args, "bash:5"));
    }

    #[test]
    fn custom_entrypoint() {
        let args = argv(r#"{"shell":"bash","docker":{"image":"bash:5","entrypoint":"/usr/bin/env"}}"#);
        assert!(position(&args, "--entrypoint=/usr/bin/env") < position(&args, "bash:5"));
        assert_eq!(args.last().unwrap(), "bash");
    }

    #[test]
    fn no_entrypoint_flag_by_default() {
        let args = argv(r#"{"shell":"bash","docker":{"image":"bash:5"}}"#);
        assert!(!args.iter().any(|a| a.starts_with("--entrypoint")));
    }

    fn config(json: &str) -> Config {
        let json = format!(r#"{{"config":{json},"cells":[]}}"#);
        serde_json::from_str::<BatchRequest>(&json).unwrap().config
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn hint_for_shell_passed_to_shell_entrypoint() {
        let c = config(r#"{"shell":"zsh","docker":{"image":"img"}}"#);
        let hint = startup_hint_for(c.docker.as_ref().unwrap(), &c, &strings(&["/usr/bin/zsh"]), &[]).unwrap();
        assert!(hint.contains("ENTRYPOINT"), "{hint}");
        assert!(hint.contains("docker.entrypoint"), "{hint}");
    }

    #[test]
    fn no_hint_when_entrypoint_is_cleared() {
        let c = config(r#"{"shell":"zsh","docker":{"image":"img","entrypoint":""}}"#);
        assert!(startup_hint_for(c.docker.as_ref().unwrap(), &c, &strings(&["/usr/bin/zsh"]), &[]).is_none());
    }

    #[test]
    fn hint_for_image_default_command() {
        let c = config(r#"{"docker":{"image":"python:3"}}"#);
        let hint = startup_hint_for(c.docker.as_ref().unwrap(), &c, &[], &strings(&["python3"])).unwrap();
        assert!(hint.contains("python3"), "{hint}");
        assert!(hint.contains("`shell`"), "{hint}");
    }

    #[test]
    fn no_env_wrapper_without_session_env() {
        let args = argv(r#"{"shell":"bash","docker":{"image":"bash:5"}}"#);
        let image = position(&args, "bash:5");
        assert_eq!(&args[image + 1..], ["bash"]);
    }
}
