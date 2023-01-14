use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use self::config::LspConfig;

mod config;

#[derive(Debug, Clone, Parser)]
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
    println!("{:#?}", lsp_config);
    Ok(())
}
