[![Build](https://github.com/anttiharju/slack9/actions/workflows/build.yml/badge.svg)](https://github.com/anttiharju/slack9/actions/workflows/build.yml)

# Introduction

<!--🐒-->

`slack9` is a terminal user interface (TUI) for tracking Slack messages based on search queries.

## Features

<!--🐵-->

- **Message search** across channels using configurable search queries
- **Emoji reaction-based categorisation** – organise messages by reactions (e.g., ✅ completed, 👀 WIP)
- **Configurable polling** with an animated progress indicator
- **Channel filtering** to narrow results
- **Deep linking** – press Enter to open a message directly in Slack
- **Vim-style keybindings** for navigation

![Screenshot](./Scherm­afbeelding%202026-03-12%20om%2021.30.32.png)

## Installation

<!--🙈-->

```sh
brew install anttiharju/tap/slack9
```

Or download a binary from GitHub releases.

Also available via Nix at https://github.com/anttiharju/nur-packages

## Usage

<!--🙉-->

Use with a direnv `.envrc`:

```sh
#!/usr/bin/env bash

export SLACK9_WORKSPACE=https://foo.slack.com
# consider 1Password CLI for safe storage if you have it
export SLACK9_XOXC=xoxc-bar
export SLACK9_XOXD=xoxd-baz
```

and run:

```sh
slack9
```

### Environment variables

Three environment variables must be set:

| Variable           | Description                                             |
| ------------------ | ------------------------------------------------------- |
| `SLACK9_WORKSPACE` | Workspace URL (e.g., `https://yourworkspace.slack.com`) |
| `SLACK9_XOXD`      | Slack session token (used in Cookie header)             |
| `SLACK9_XOXC`      | Slack OAuth token                                       |

#### `SLACK9_XOXC`

Dev Tools -> Console

<!-- prettier-ignore-start -->
```js
copy(JSON.parse(localStorage.getItem("localConfig_v2")).teams[Object.keys(JSON.parse(localStorage.getItem("localConfig_v2")).teams)[0]].token)
```
<!-- prettier-ignore-end -->

The token is copied to the clipboard.

#### `SLACK9_XOXD`

Dev Tools -> Application / Storage -> Cookies -> d

Copy the value manually.

#### Revoke tokens

Log out from the browser session where the tokens were extracted.

## Configuration

<!--🙊-->

`config.toml` is loaded from `~/.config/slack9` (override with `$SLACK9_CONFIG_DIR`).

```toml
[header]
past = "7d"      # Lookback window
poll = "10s"     # Poll interval

[categories]
blocked = "hourglass"
completed = "white_check_mark"
wip = ["eyes", "writing_hand"]

[state]
user_pings = true
search = ["help", "me"]
active_categories = []
show_uncategorised = true
```

Duration values support `s`, `m`, `h`, `d`, `w`, `M` (e.g., `7d`, `2h`).

## Keybindings

### Navigation

| Key       | Action                |
| --------- | --------------------- |
| `j` / `↓` | Move down             |
| `k` / `↑` | Move up               |
| `gg`      | Jump to first         |
| `GG`      | Jump to last          |
| `Enter`   | Open message in Slack |

### Filtering

| Key     | Action                        |
| ------- | ----------------------------- |
| `/`     | Open channel filter           |
| `1`–`9` | Toggle category visibility    |
| `0`     | Toggle uncategorised messages |
| `o1`    | Show only category 1          |
| `Esc`   | Clear filter                  |

### Commands

| Command            | Action                 |
| ------------------ | ---------------------- |
| `:time <duration>` | Change lookback window |
| `:poll <duration>` | Change poll interval   |
| `:quit` `:q` `:q!` | Exit TUI               |

## Development

A Nix flake is provided for a reproducible dev environment:

```sh
nix develop
```

Alternatively, use [nix-direnv](https://github.com/nix-community/nix-direnv) to retain your shell customisations via

```sh
direnv allow
```

### Nix cache setup

To avoid building from scratch, add the following extra cache substituters:

```nix
nixConfig.extra-substituters = [
  "https://nix-community.cachix.org"
  "https://anttiharju.cachix.org"
];
```

with the following `nix.conf`:

```conf
trusted-substituters = https://cache.nixos.org/ https://nix-community.cachix.org https://anttiharju.cachix.org
trusted-public-keys = cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY= nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs= anttiharju.cachix.org-1:1/w1vNdGsG6EynEoWhjJLJ2rxqde2BP+wkKM+YdOxMQ=
substituters = https://cache.nixos.org/
experimental-features = nix-command flakes
accept-flake-config = true
```

Details at https://blog.ielliott.io/per-project-nix-substituters.

To get started with Nix:

- https://github.com/NixOS/nix-installer
- https://zero-to-nix.com

## Troubleshooting

### API logs

Launch with `--debug` to store debug logs under `$SLACK9_CONFIG_DIR`.

### Build results in ProcessFdQuotaExceeded

- Related to https://github.com/ziglang/zig/issues/23273

Run:

```sh
ulimit -n 512
```

This unfortunately cannot be packaged into the direnv setup.

## Acknowledgements

Heavily inspired by [K9s](https://github.com/derailed/k9s).

## License

[MIT](LICENSE)
