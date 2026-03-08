# slack9

Tracking various requests across Slack is a massive pain. This TUI helps you find and track them better.

## How to acquire xoxd and xoxc tokens

Open Slack in your browser.

### xoxc

Dev Tools -> Console

<!-- prettier-ignore-start -->
```js
copy(JSON.parse(localStorage.getItem("localConfig_v2")).teams[Object.keys(JSON.parse(localStorage.getItem("localConfig_v2")).teams)[0]].token)
```
<!-- prettier-ignore-end -->

and you have it copied in your clipboard.

### xoxd

Dev Tools -> Storage -> Cookies -> d.

This one you need to do manually.

## Usage

Assumes you use direnv.

1. Create `.envrc` that supplies the following:

```sh
#!/usr/bin/env bash

export SLACK9_WORKSPACE=https://foo.slack.com

# consider 1Password CLI for safe storage if you have it
export SLACK9_XOXC=xoxc-bar
export SLACK9_XOXD=xoxd-baz
```

2. `direnv allow`
3. `slack9`

### Revoke tokens

Simply log out from the browser session from where you extracted the tokens.

## Troubleshooting

### API logs

Launch with `--debug` to store debug logs under `~/.config/slack9/`

### Build results in ProcessFdQuotaExceeded

- Related to https://github.com/ziglang/zig/issues/23273

Simply run

```sh
ulimit -n 512
```

This unfortunately cannot be packaged into the direnv setup.
