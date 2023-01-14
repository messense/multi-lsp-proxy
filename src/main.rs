use std::fs;
use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde_json::Value;
use tokio::{
    io::{self, AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::{mpsc, oneshot},
};

use self::config::LspConfig;

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

async fn proxy(mut child: Child, mut input: mpsc::Receiver<(String, oneshot::Sender<Value>)>) {
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    while let Some((message, tx)) = input.recv().await {
        stdin
            .write_all(format!("Content-Length: {}\r\n\r\n", message.len()).as_bytes())
            .await
            .unwrap();
        stdin.write_all(message.as_bytes()).await.unwrap();
        let resp = read_message(&mut stdout).await.unwrap();
        tx.send(resp).unwrap();
    }
}

async fn run(config: LspConfig) -> Result<()> {
    let mut child_txs = Vec::with_capacity(config.languages.len());
    for lang in &config.languages {
        // spawn LSP server command
        let mut cmd = Command::new(&lang.command);
        cmd.args(&lang.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn {} binary.", &lang.command.display()))?;

        let (tx, rx) = mpsc::channel(100);
        child_txs.push(tx);
        tokio::spawn(async move {
            proxy(child, rx).await;
        });
    }

    let mut stdin = BufReader::new(io::stdin());
    let mut stdout = io::stdout();
    // LSP server main loop
    // Read new command, send to all child LSP servers
    // and merge responses
    loop {
        let content_length = read_content_length(&mut stdin).await?;
        let mut body = vec![0u8; content_length];
        stdin.read_exact(&mut body).await.unwrap();
        let raw = String::from_utf8(body)?;
        for tx in &mut child_txs {
            let (resp_tx, resp_rx) = oneshot::channel();
            tx.send((raw.clone(), resp_tx)).await.unwrap();

            let res = resp_rx.await.unwrap();
            let message = serde_json::to_string(&res).unwrap();
            stdout
                .write_all(format!("Content-Length: {}\r\n\r\n", message.len()).as_bytes())
                .await
                .unwrap();
            stdout.write_all(message.as_bytes()).await.unwrap();
        }
    }
}

#[derive(Debug, Parser)]
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
