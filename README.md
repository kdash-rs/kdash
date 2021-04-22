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
  - [x] Containers
  - [ ] Deployments
  - [ ] ConfigMaps
  - [ ] StatefulSets
  - [ ] ReplicaSets
- Describe resources
  - [x] Pods
  - [x] Nodes
  - [ ] Services (simulated)
  - [ ] Deployments (simulated)
  - [ ] ConfigMaps (simulated)
  - [ ] StatefulSets (simulated)
  - [ ] ReplicaSets (simulated)
  - [ ] as YAML
- Stream logs/events
  - [x] Containers
  - [ ] Services
  - [ ] Deployments
  - [ ] StatefulSets
- Context
  - [x] Context info
  - [x] Node metrics
  - [x] Context watch
  - [x] Change namespace?
  - [ ] Context switch
- [ ] Resources utilizations
- [x] Dark/Light themes
- [ ] Custom keymap
- [ ] Custom theme
- [ ] Tests, a lot of them :)

## Installation

Beta release binaries for macOS, Linux and Windows are available on the [releases](https://github.com/kdash-rs/kdash/releases) page

If you have Cargo installed then you install KDash from crates.io

```
cargo install kdash
```

You can also clone the repo and run `cargo run` to build and run the app

If you face issues with openssl then please run `cargo run --features vendored`

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
