# Contribution are welcome 🙏

You need to have the Rust tool belt for developing KDash

Install Rust tool belt following [this](https://www.rust-lang.org/tools/install). This will install `rustup`, `rustc` and `cargo`

## Other requirements

- kubectl for local testing

## Setup workspace

1. Clone this repo
1. Run `cargo test` to setup hooks
1. Make changes
1. Run the application using `make run` or `cargo run`
1. Commit changes. This will trigger pre-commit hooks that will run format, test and lint. If there are errors or warnings from Clippy, fix them
1. Push to your clone. This will trigger pre-push hooks that will run lint and test
1. Create a PR

- There are other commands that are configured on the Makefile. If you have make installed then you can use those directly
- For `make test` you need to install tarpaulin with `cargo install cargo-tarpaulin`
- For `make analyse` you need to install geiger with `cargo install cargo-geiger`
