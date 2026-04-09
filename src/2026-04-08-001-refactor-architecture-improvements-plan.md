---
title: "refactor: Architecture improvements — registry macros, watch support, tab safety"
type: refactor
status: active
date: 2026-04-08
---

# Architecture Improvements Plan

## Overview

KDash is a ~20K LOC Rust TUI Kubernetes dashboard. An architecture review identified structural improvements to reduce boilerplate, improve real-time responsiveness, and eliminate latent bugs. This plan covers three improvement areas ordered by dependency and risk.

## Problem Frame

Adding a new Kubernetes resource to kdash requires touching 6+ files with tightly coupled changes that must stay in sync: `IoEvent` enum, `handle_network_event`, `dispatch_by_active_block`, handler macros, scroll macros, and `Data` struct. This coupling slows development and risks subtle bugs from mismatched dispatch chains. Additionally, the app uses polling instead of Kubernetes watch APIs, causing delayed visibility of changes.

## Requirements Trace

- R1. Eliminate magic tab index numbers that silently break if tab order changes
- R2. Reduce the number of sites that must be kept in sync when adding a standard resource
- R3. Replace polling with watch/reflector for real-time updates (future phase)

## Scope Boundaries

- Out of scope: Full god-struct decomposition of `App`, UI component refactoring, clipboard blocking fix
- Registry covers "standard" resources only — Pods, Nodes, Metrics, Dynamic resources retain manual handlers due to custom logic

## Key Technical Decisions

- **`set_active_block()` over named constants**: A method that looks up the tab index by `ActiveBlock` variant is impossible to get out of sync, unlike constants that could drift from the tab list
- **Registry as macro pair, not dynamic dispatch**: Two macros (`dispatch_standard_resource!` and `handle_standard_network_event!`) keep Rust's type safety while centralizing the mapping
- **Incremental approach**: Each improvement is independently shippable

## Implementation Units

### Phase 1: Named Tab Indices

- [x] **Unit 1: Replace magic tab indices with `set_active_block()` method**

  Added `TabsState::set_active_block(ActiveBlock)` in `src/app/models.rs` that looks up the index by matching the `ActiveBlock` variant in the tab items list. Replaced all 11 `set_index(N)` calls in `src/handlers/mod.rs`. Added unit test.

  **Files modified:** `src/app/models.rs`, `src/handlers/mod.rs`

### Phase 2: Resource Registry

- [x] **Unit 2: Create resource registry macros**

  Created `src/app/resource_registry.rs` with two macros:
  - `dispatch_standard_resource!` — generates `ActiveBlock → IoEvent` dispatch in `app/mod.rs`
  - `handle_standard_network_event!` — generates `IoEvent → Resource::get_resource` dispatch in `network/mod.rs`

  **Files created:** `src/app/resource_registry.rs`
  **Files modified:** `src/app/mod.rs`, `src/network/mod.rs`

- [x] **Unit 3: Consolidate IoEvent dispatch with registry**

  Replaced the flat `dispatch_by_active_block` match (20+ arms) with `dispatch_standard_resource!` macro invocation + manual overrides for Pods/Dynamic/Logs. Replaced the flat `handle_network_event` match with `handle_standard_network_event!` macro invocation + manual overrides for special events.

  **Files modified:** `src/app/mod.rs`, `src/network/mod.rs`

- [ ] **Unit 4: Migrate remaining resources to registry**

  Future: Add new resources by adding one line to each macro invocation instead of 6+ manual match arms.

### Phase 3: Watch/Informer Support (Future)

- [ ] **Unit 5: Enable kube-rs runtime features and add reflector infrastructure**
- [ ] **Unit 6: Pilot reflector-backed fetching for namespaces**
- [ ] **Unit 7: Extend reflector support to core resources**
- [ ] **Unit 8: Full migration and remove polling fallback**

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| Macro complexity hinders debugging | Macros are thin wrappers (match generation only), not complex code generators |
| kube-rs reflector API stability | Research exact API during Unit 5; evaluate upgrading kube-rs if needed |
| RBAC restrictions may prevent watch | Graceful per-resource fallback to polling |

## Verification

- All 257 tests pass after Units 1-3
- `cargo clippy --all --all-features --all-targets -- -D warnings` passes clean
- No behavioral changes — purely structural refactoring
