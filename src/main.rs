use anyhow::{Context, Result};
use reqwest::blocking::multipart::{Form, Part};
use serde_json::json;
use std::env;
use std::process::{Command, Stdio};
use std::time::Instant;

fn main() -> Result<()> {
    let mut args = env::args().peekable();

    // skip the first arg, which is the executable name
    let _ = args.next();

    let mut job_name = None;
    let mut webhook_url = None;

    while args.peek().map(|a| a.starts_with("--")).unwrap_or(false) {
        let arg = args.next().unwrap();
        let value = args.next().unwrap();

        match &arg[2..] {
            "name" => {
                job_name = Some(value);
            }
            "url" => {
                webhook_url = Some(value);
            }
            _ => anyhow::bail!("Unknown option: {}", arg),
        }
    }

    let Some(job_name) = job_name else {
        anyhow::bail!("Missing job name");
    };

    let Some(webhook_url) = webhook_url else {
        anyhow::bail!("Missing webhook url");
    };

    let Some(command) = args.next() else {
        anyhow::bail!("Missing command");
    };

    let command_args = args.collect::<Vec<_>>();

    let start_time = Instant::now();

    let output = Command::new(&command)
        .args(&command_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute command")?;

    let duration = start_time.elapsed().as_secs_f64();
    let exit_code = output.status.code().unwrap_or(1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let payload = json!({
        "username": job_name,
        "content": format!("`{} {}` took `{:.2}s` to execute with exit code `{}`", command, command_args.join(" "), duration, exit_code),
        "attachments": [
            {
              "id": 0,
              "filename": "stdout.txt"
            },
            {
              "id": 1,
              "filename": "stderr.txt"
            }
        ]
    });

    let form = Form::new()
        .part(
            "payload_json",
            Part::text(payload.to_string()).mime_str("application/json")?,
        )
        .part(
            "files[0]",
            Part::text(stdout)
                .mime_str("text/plain")?
                .file_name("stdout.txt"),
        )
        .part(
            "files[1]",
            Part::text(stderr)
                .mime_str("text/plain")?
                .file_name("stderr.txt"),
        );

    let client = reqwest::blocking::Client::new();
    let response = client.post(webhook_url).multipart(form).send()?;
    if !response.status().is_success() {
        anyhow::bail!("Failed to send report: {}", response.text()?);
    }

    std::process::exit(exit_code);
}
