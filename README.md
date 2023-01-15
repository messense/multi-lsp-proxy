# multi-lsp-proxy

[![GitHub Actions](https://github.com/messense/multi-lsp-proxy/workflows/CI/badge.svg)](https://github.com/messense/multi-lsp-proxy/actions?query=workflow%3ACI)
[![PyPI](https://img.shields.io/pypi/v/multi-lsp-proxy.svg)](https://pypi.org/project/multi-lsp-proxy)

A **barely working** LSP Proxy to multiple language servers, to use multiple LSP with one programming language in
editors that doesn't support multiple LSP natively like Helix (version 22.12).

## Installation

```bash
pip install multi-lsp-proxy
```

## Usage

```bash
Usage: multi-lsp-proxy [OPTIONS] --config <CONFIG>

Options:
  -c, --config <CONFIG>      Configuration file path
  -l, --language <LANGUAGE>  Select language servers by programming language name
  -h, --help                 Print help
  -V, --version              Print version
```

To use with Helix, set the `language-server` option in `languages.toml`,
below is an example for Python that enables both `pyright-langserver` and `ruff-lsp`:

```toml
# Helix languages.toml file
[[language]]
 name = "python"
 scope = "source.python"
 injection-regex = "python"
 file-types = ["py", "pyi"]
 shebangs = ["python"]
 roots = ["pyproject.toml", "setup.py", "Poetry.lock"]
 comment-token = "#"
 language-server = { command = "multi-lsp-proxy", args = ["--config", "/path/to/multi-lsp-config.toml"] }
 auto-format = false
 indent = { tab-width = 4, unit = "    " }
 config = {}
```

and configure multi-lsp-proxy in `multi-lsp-proxy.toml`

```toml
log-file = "/tmp/multi-lsp-proxy.log"

[[language]]
name = "python"
command = "pyright-langserver"
args = ["--stdio"]

[[language]]
name = "python"
command = "ruff-lsp"
```

## License

This work is released under the MIT license. A copy of the license is provided in the [LICENSE](../LICENSE) file.
