use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LspConfig {
    #[serde(rename = "language")]
    pub languages: Vec<Language>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Language {
    pub name: String,
    pub command: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let config = r#"
[[language]]
name = "python"
command = "pylsp"

[[language]]
name = "python"
command = "ruff-lsp"
        "#;

        let conf: LspConfig = toml_edit::easy::from_str(config).unwrap();
        assert_eq!(conf.languages.len(), 2);
        assert_eq!(conf.languages[0].name, "python");
        assert_eq!(conf.languages[0].command, PathBuf::from("pylsp"));
        assert_eq!(conf.languages[0].args.len(), 0);
        assert_eq!(conf.languages[1].name, "python");
        assert_eq!(conf.languages[1].command, PathBuf::from("ruff-lsp"));
        assert_eq!(conf.languages[1].args.len(), 0);
    }
}
