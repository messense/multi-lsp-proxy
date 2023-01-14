use std::fs;
use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde_json::Value;
use tokio::{
    io::{self, AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout, Command},
    sync::{broadcast, mpsc},
};

use self::config::LspConfig;
use tracing::{debug, error};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

mod config;

async fn read_content_length<T>(reader: &mut BufReader<T>) -> Result<usize>
where
    BufReader<T>: AsyncBufRead,
    BufReader<T>: AsyncBufReadExt,
    T: Unpin,
{
    let mut content_length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        if let Some(content) = line.strip_prefix("Content-Length: ") {
            content_length = content
                .trim()
                .parse()
                .context("Failed to parse Content-Length")?;
        } else if line.strip_prefix("Content-Type: ").is_some() {
            // ignored.
        } else if line == "\r\n" {
            break;
        } else {
            bail!("Failed to get Content-Length from LSP data.")
        }
    }
    Ok(content_length)
}

async fn read_message<T>(reader: &mut BufReader<T>) -> Result<Value>
where
    BufReader<T>: AsyncBufRead,
    BufReader<T>: AsyncBufReadExt,
    T: Unpin,
{
    let content_length = read_content_length(reader).await?;
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).await.unwrap();
    serde_json::from_slice(&body).context("Failed to parse input as LSP data")
}

async fn proxy_stdin(mut stdin: ChildStdin, mut input: broadcast::Receiver<String>) {
    while let Ok(message) = input.recv().await {
        stdin
            .write_all(format!("Content-Length: {}\r\n\r\n", message.len()).as_bytes())
            .await
            .unwrap();
        stdin.write_all(message.as_bytes()).await.unwrap();
    }
}

async fn proxy_stdout(mut stdout: BufReader<ChildStdout>, tx: mpsc::Sender<Value>) {
    loop {
        let message = read_message(&mut stdout).await.unwrap();
        if let Err(_) = tx.send(message).await {
            error!("send error, receiver dropped");
        }
    }
}

async fn run(config: LspConfig) -> Result<()> {
    // setup tracing
    if let Some(log_file) = config.log_file.as_ref() {
        let directory = log_file.parent().unwrap();
        let file_name = log_file.file_name().unwrap();
        let file_appender = tracing_appender::rolling::never(directory, file_name);
        let env_filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::DEBUG.into())
            .from_env_lossy();
        tracing_subscriber::fmt()
            .with_writer(file_appender)
            .with_env_filter(env_filter)
            .init();
    }

    let (tx, _rx) = broadcast::channel(100);
    let mut child_processes = Vec::new();
    let mut child_rxs = Vec::with_capacity(config.languages.len());
    for lang in &config.languages {
        // spawn LSP server command
        let mut cmd = Command::new(&lang.command);
        cmd.args(&lang.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn {} binary.", &lang.command.display()))?;
        let child_stdin = child.stdin.take().unwrap();
        let child_stdout = BufReader::new(child.stdout.take().unwrap());

        let (child_tx, child_rx) = mpsc::channel(100);
        child_rxs.push(child_rx);

        let rx = tx.subscribe();
        tokio::spawn(async move {
            proxy_stdin(child_stdin, rx).await;
        });
        tokio::spawn(async move { proxy_stdout(child_stdout, child_tx).await });

        // Keep child process alive
        child_processes.push(child);
    }

    tokio::spawn(async move {
        let mut stdout = io::stdout();
        loop {
            for rx in &mut child_rxs {
                if let Some(value) = rx.recv().await {
                    let message = serde_json::to_string(&value).unwrap();
                    stdout
                        .write_all(format!("Content-Length: {}\r\n\r\n", message.len()).as_bytes())
                        .await
                        .unwrap();
                    stdout.write_all(message.as_bytes()).await.unwrap();
                }
            }
        }
    });

    // LSP server main loop
    // Read new command, send to all child LSP servers
    // and merge responses
    let mut stdin = BufReader::new(io::stdin());
    loop {
        let content_length = read_content_length(&mut stdin).await?;
        let mut body = vec![0u8; content_length];
        stdin.read_exact(&mut body).await.unwrap();
        let raw = String::from_utf8(body)?;
        // let request: Request = serde_json::from_str(&raw).unwrap();
        debug!(request = %raw, "incoming lsp request");
        tx.send(raw.clone()).unwrap();
    }
}

#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    /// configuration file path
    #[arg(short = 'c', long, default_value = "config.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_content = fs::read_to_string(&cli.config)?;
    let lsp_config: LspConfig = toml_edit::easy::from_str(&config_content)?;
    run(lsp_config).await?;
    Ok(())
}
