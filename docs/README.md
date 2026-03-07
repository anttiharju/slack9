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
