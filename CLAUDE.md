# CLAUDE.md/AGENTS.md

This file provides guidance to Claude Code (claude.ai/code) and other AI Agents when working with code in this repository.

## Project Overview

KDash is a terminal UI dashboard for Kubernetes, built with Rust using ratatui (TUI framework) and the kube-rs client library. It provides real-time monitoring of Kubernetes resources with keyboard-driven navigation.

## Build & Development Commands

```bash
# Build (runs lint + tests first)
make build

# Run (formats, lints, then runs)
make run

# Run directly without checks
cargo run

# Run with CLI args (e.g., tick rate, debug mode)
cargo run -- -t 100 -d=debug

# Lint (clippy with strict warnings-as-errors)
cargo clippy --all --all-features --all-targets --workspace -- -D warnings
# or
make lint

# Format
cargo fmt

# Run all tests (lint + cargo test)
make test

# Run a single test
cargo test <test_name>

# Run tests in a specific module
cargo test <module>::tests

# Test coverage (requires cargo-tarpaulin)
make test-cov
```

## Architecture

The app follows an async event-driven architecture with three main communication channels (tokio mpsc):

### Core Loop (main.rs)
- **UI thread** (main runtime): terminal rendering loop using crossterm + ratatui. Polls for input/tick/kubeconfig-change events.
- **Network thread** (separate OS thread + tokio runtime): runs three concurrent tasks:
  - `Network` — handles one-shot K8s API calls (list pods, get nodes, etc.) via `IoEvent`
  - `NetworkStream` — handles streaming operations (log tailing, exec) via `IoStreamEvent`
  - `CmdRunner` — runs kubectl shell commands (describe, top) via `IoCmdEvent`

### Module Responsibilities

- **`app/`** — Application state (`App` struct) and Kubernetes resource models. Each resource type (pods, deployments, nodes, etc.) has its own file defining its `Kube*` data struct and an `*Resource` trait impl. `models.rs` contains shared UI state types (`StatefulTable`, `ScrollableTxt`, `TabsState`, etc.). `key_binding.rs` defines all keybinding actions.
- **`network/`** — K8s API interaction layer. `mod.rs` has `Network` struct handling `IoEvent` variants. `stream.rs` handles streaming events. Uses `kube` crate client.
- **`ui/`** — Rendering logic. `draw()` in `mod.rs` is the entry point. `overview.rs` renders the main dashboard. `resource_tabs.rs` renders resource-specific views. `theme.rs` handles color theming.
- **`handlers/`** — Input event handling. Maps key presses and mouse events to app state changes and network dispatches. Uses `handle_workload_action!` macro for common resource interactions.
- **`event/`** — Terminal event abstraction (keyboard, mouse, tick, kubeconfig file watch).
- **`cmd/`** — Shell command execution (kubectl describe, top, etc.).
- **`config.rs`** — User config from `$KDASH_CONFIG` or `~/.config/kdash/config.yaml` (YAML, supports keybinding and theme overrides).

### Key Patterns

- App state is `Arc<Mutex<App>>` shared between UI and network threads.
- Network calls are dispatched by sending `IoEvent`/`IoStreamEvent`/`IoCmdEvent` through channels; results are written back to `App` state under the mutex.
- Navigation uses a stack (`nav_stack`) of `Route` objects with `ActiveBlock` enum variants.
- Each K8s resource type follows a consistent pattern: data struct in `app/<resource>.rs`, network fetch in `network/mod.rs`, UI rendering in `ui/resource_tabs.rs`, key handling in `handlers/mod.rs`.

### Resource Actions (write path)

- **Registry:** `app/actions.rs` is the single source of truth for per-block actions. `ResourceAction` carries a label and an optional hotkey; `actions_for(block)` lists what the `m` action menu shows. Menu selection replays an action's hotkey for hotkey-backed actions, or dispatches directly for menu-only ones (`execute_resource_action` in `handlers/mod.rs`).
- **Generic writes:** `app/dynamic.rs::api_resource_for_block` maps an `ActiveBlock` to `(ApiResource, Scope)`; `network/mod.rs` uses a dynamic `Api<DynamicObject>` for `delete_resource` and `patch_resource` (driven by the `ResourcePatch` enum) so one helper covers most kinds. Typed `Api<K>` is used only where a subresource/create needs a concrete type (e.g. `trigger_cronjob`).
- **Safety:** impactful actions route through a confirmation overlay (`actions::Modal`, stored as `App.modal`, drawn by `ui::draw_modal`). The modal consumes keys first in `handlers::handle_key_events`; there is no separate read-only mode. Every successful write re-dispatches the view's `Get*` via `dispatch_by_active_block`.
- **Value input:** actions that need a value (scale) open a single-line input overlay (`actions::InputModal` + `InputAction`, stored as `App.input_modal`, drawn by `ui::draw_input_modal`). It captures keys after the confirm modal but before the action menu. `InputModal::validate` parses the buffer and, on success, returns the confirmation `Modal` to chain into (so impactful changes still confirm); on failure it sets an inline error and stays open. Scale reuses the generic `ResourcePatch::SetReplicas` via `patch_resource`.
- Adding a keybinding bumps the help-page fixtures in `ui/help.rs` (the `[N]` count and `[1/N]`); update them when `generate_keybindings!` grows.

## Pre-commit Hooks

cargo-husky runs pre-commit (format + test + lint) and pre-push (lint + test) hooks. Run `cargo test` once after clone to set them up.

## CI

Tests run on Linux, macOS, and Windows with both stable and nightly Rust.
