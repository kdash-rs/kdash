# KDash - A fast and simple dashboard for Kubernetes

**Note**: This is a beta version and is work in progress

```
 _  __ ___             _
| |/ /|   \  __ _  ___| |_
| ' < | |) |/ _` |(_-<| ' \
|_|\_\|___/ \__,_|/__/|_||_|
```

A simple terminal dashboard for Kubernetes built with Rust

![UI](./ui.gif)

## Features & Todos

- CLI Info
- Resource Watch (configurable polling interval with `-p` flag)
  - [x] Pods
  - [x] Nodes
  - [x] Namespace
  - [x] Services
  - [ ] Deployments
  - [ ] ConfigMaps
  - [ ] StatefulSets
  - [ ] ReplicaSets
- [ ] describe resources
  - [ ] Pods
  - [ ] Nodes
  - [ ] Namespace
  - [ ] Services
  - [ ] Deployments
  - [ ] ConfigMaps
  - [ ] StatefulSets
  - [ ] ReplicaSets
- [ ] Stream logs/events
  - [ ] Pods
  - [ ] Services
  - [ ] Deployments
  - [ ] ConfigMaps
  - [ ] StatefulSets
  - [ ] ReplicaSets
- Context
  - [x] Context info
  - [x] Node metrics
  - [x] Context watch
  - [x] Change namespace?
  - [ ] Context switch
- [x] Dark/Light themes
- [ ] Custom keymap
- [ ] Custom theme

- [ ] Tests, need a lot of them
- [ ] CI/CD

## Installation

Min cargo version: 1.48.0

For now you can use this by cloning the repo and running `cargo run`

If you face issues with openssl then please run `cargo run --features vendored`

Will publish binaries and crates once out of beta

## USAGE:

Press `?` while running the app to see keybindings

## FLAGS:

- `-h, --help`: Prints help information
- `-V, --version`: Prints version information
- `-t, --tick-rate <tick-rate>`: Set the tick rate (milliseconds): the lower the number the higher the FPS.
- `-p, --poll-rate <poll-rate>`: Set the network call polling rate (milliseconds, should be multiples of tick-rate): the lower the number the higher the network calls.

## Libraries used

- [tui-rs](https://github.com/fdehau/tui-rs)
- [clap](https://github.com/clap-rs/clap)
- [tokio](https://github.com/tokio-rs/tokio)
- [duct.rs](https://github.com/oconnor663/duct.rs)
- [kube-rs](https://github.com/clux/kube-rs)
- [serde](https://github.com/serde-rs/serde)

## Licence

MIT

## Authors

- [Deepu K Sasidharan](https://deepu.tech/)
