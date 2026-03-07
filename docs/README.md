# slack9s

`slack9s` is a template for my Rust projects to make it easier to start new ones.

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

export SLACK9S_WORKSPACE=https://foo.slack.com

# consider 1Password CLI for safe storage if you have it
export SLACK9S_XOXC=xoxc-bar
export SLACK9S_XOXD=xoxd-baz
```

2. `direnv allow`
3. `slack9s`
