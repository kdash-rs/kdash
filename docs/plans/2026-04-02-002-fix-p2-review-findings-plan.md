---
title: "fix: Address P2 review findings"
type: fix
status: active
date: 2026-04-02
origin: docs/REVIEW.md
---

# fix: Address P2 review findings

## Overview

Address all 7 P2 (Moderate) findings from the code review: fix bugs, harden numeric types, cap unbounded growth, replace blocking calls, normalize naming, and reduce resource boilerplate. Each P2 item lands as its own commit.

## Problem Frame

The code review identified 7 moderate-severity issues spanning correctness bugs, potential overflows, unbounded memory growth, UI-thread blocking, naming inconsistency, and maintainability friction from boilerplate duplication. None are critical, but together they degrade code quality and developer experience. (see origin: docs/REVIEW.md, P2 section #9-#15)

## Requirements Trace

- R1. Fix inverted condition bug in `get_lb_ext_ips` (Review #13)
- R2. Change `ScrollableTxt::offset` from `u16` to `usize` to prevent overflow (Review #12)
- R3. Cap `navigation_stack` to prevent unbounded growth (Review #10)
- R4. Replace `is_loading: bool` with a reference counter for correct loading state (Review #11)
- R5. Replace blocking `thread::sleep(100ms)` in clipboard copy (Review #14)
- R6. Normalize `ActiveBlock` variant names, `Data` field names, and module names (Review #15)
- R7. Reduce resource boilerplate via macro/registry pattern (Review #9)

## Scope Boundaries

- No changes to the P0/P1 fixes already landed on this branch
- No changes to the two-phase startup (separate concern)
- Boilerplate reduction targets handler dispatch and network dispatch only — the existing `draw_resource_tab!` macro stays as-is
- No new features, no new resource types added during this work

## Context & Research

### Relevant Code and Patterns

- `src/app/mod.rs` — `ActiveBlock` enum (lines 68-103), `Data` struct (lines 132-168), `navigation_stack` (line 194), `is_loading` (line 206), `dispatch()` (line 475), `push_navigation_route()` (line 530)
- `src/app/models.rs` — `ScrollableTxt` struct (lines 211-249), `Scrollable` trait
- `src/app/svcs.rs` — `get_lb_ext_ips` (lines 185-217)
- `src/handlers/mod.rs` — `copy_to_clipboard` (lines 741-757), block action dispatch (lines 241-651), scroll dispatch (lines 698-739)
- `src/network/mod.rs` — `IoEvent` enum, `handle_network_event` dispatch
- `src/ui/resource_tabs.rs` — tab render dispatch
- Existing macro pattern: `draw_resource_tab!` in `src/ui/utils.rs`

### Institutional Learnings

- The codebase uses `cargo-husky` pre-commit hooks running `cargo fmt` + `make lint` (clippy)
- Test suite: 76 tests, all passing on current branch
- IDE auto-bumps Cargo.toml deps on save — always restore with `git checkout HEAD -- Cargo.toml` before cargo commands

## Key Technical Decisions

- **Navigation cap strategy:** Use a simple `const MAX_NAV_STACK: usize = 128` with truncation from the bottom (oldest routes) when pushing beyond the cap. A ring buffer is overkill for a navigation stack that needs random-access (`get_nth_route_from_last`).
- **Loading counter type:** `u32` counter incremented on dispatch, decremented on completion. `is_loading` becomes a method returning `self.loading_counter > 0`. This preserves the existing API surface (callers still check `app.is_loading`) with minimal churn.
- **Clipboard sleep replacement:** Spawn the clipboard write on a background `std::thread` so the 100ms sleep doesn't block the UI loop. The sleep is necessary for clipboard correctness on some platforms, but it doesn't need to happen on the UI thread.
- **Naming normalization scope:** Rename `ActiveBlock` variants, `Data` field names, and abbreviations to consistent full forms. Module filenames (`svcs.rs`) stay unchanged to limit diff size — they can be renamed in a follow-up.
- **Boilerplate reduction approach:** A declarative `resource_dispatch!` macro that generates the match arms in `handle_network_event` and handler block action/scroll dispatch from a resource definition list. This covers the two highest-duplication sites.

## Open Questions

### Resolved During Planning

- **Should module files be renamed (e.g., `svcs.rs` → `services.rs`)?** No — limits diff size, deferred to follow-up.
- **Should `is_loading` change affect the network handler?** Yes — network handler calls `loading_counter -= 1` instead of `is_loading = false`.

### Deferred to Implementation

- **Exact macro syntax for `resource_dispatch!`:** The specific DSL will emerge during implementation based on what parameters each dispatch site needs.
- **Whether clippy raises warnings on the naming changes:** Will be caught by pre-commit hooks.

## Implementation Units

- [ ] **Unit 1: Fix `get_lb_ext_ips` inverted condition**

  **Goal:** Fix the bug where `external_ips` are silently discarded for LoadBalancer services.

  **Requirements:** R1

  **Dependencies:** None

  **Files:**
  - Modify: `src/app/svcs.rs`
  - Test: `src/app/svcs.rs` (inline tests module)

  **Approach:**
  - Change `external_ips.is_none()` to `external_ips.is_some()` on line 209
  - Simplify the logic: when `external_ips` is Some, extend `lb_ips` with it; when lb_ips is non-empty, return them; otherwise return `<pending>`

  **Patterns to follow:**
  - Existing test pattern in `test_services_from_api`

  **Test scenarios:**
  - Service with both LB ingress IPs and external IPs → both sets combined
  - Service with LB ingress IPs but no external IPs → LB IPs only
  - Service with external IPs but no LB ingress → external IPs returned
  - Service with neither → `<pending>`

  **Verification:**
  - `cargo test` passes, including new test cases for `get_lb_ext_ips`

---

- [ ] **Unit 2: Change `ScrollableTxt::offset` from `u16` to `usize`**

  **Goal:** Prevent silent overflow on documents exceeding 65535 lines.

  **Requirements:** R2

  **Dependencies:** None

  **Files:**
  - Modify: `src/app/models.rs`
  - Modify: `src/ui/utils.rs` (if `offset` is used in rendering with `as u16` casts)
  - Test: `src/app/models.rs` (inline tests module)

  **Approach:**
  - Change `offset: u16` to `offset: usize` in `ScrollableTxt`
  - Remove `as u16` casts in `scroll_down` and `scroll_up` — arithmetic becomes natural `usize` ops
  - Update any UI code that consumes `offset` (likely needs `as u16` at the rendering boundary only)

  **Patterns to follow:**
  - `StatefulTable` already uses `usize` for its index

  **Test scenarios:**
  - Scroll down on content exceeding 65535 lines → offset goes beyond u16 max without wrapping
  - Scroll up from high offset → saturating subtraction works correctly

  **Verification:**
  - `cargo test` passes
  - No `as u16` casts remain in `ScrollableTxt` methods

---

- [ ] **Unit 3: Cap `navigation_stack` growth**

  **Goal:** Bound navigation stack memory to prevent unbounded growth during long sessions.

  **Requirements:** R3

  **Dependencies:** None

  **Files:**
  - Modify: `src/app/mod.rs`
  - Test: `src/app/mod.rs` (inline tests module)

  **Approach:**
  - Add `const MAX_NAV_STACK: usize = 128`
  - In `push_navigation_route`, after pushing, if `len() > MAX_NAV_STACK`, drain oldest entries: `self.navigation_stack.drain(..self.navigation_stack.len() - MAX_NAV_STACK)`
  - This preserves `get_current_route`, `get_prev_route`, and `get_nth_route_from_last` behavior since they index from the end

  **Patterns to follow:**
  - `MAX_LOG_RECORDS` pattern in `models.rs` for bounding collections

  **Test scenarios:**
  - Push 130 routes → stack len is 128, most recent route is current
  - `get_prev_route` still returns second-to-last after truncation
  - Normal push/pop cycle within cap → no change in behavior

  **Verification:**
  - `cargo test` passes
  - Navigation works correctly in manual testing

---

- [ ] **Unit 4: Replace `is_loading: bool` with loading counter**

  **Goal:** Loading indicator stays visible until all dispatched requests complete, not just the first.

  **Requirements:** R4

  **Dependencies:** None

  **Files:**
  - Modify: `src/app/mod.rs` — replace `is_loading: bool` with `loading_counter: u32`, add `is_loading()` method
  - Modify: `src/network/mod.rs` — change `app.is_loading = false` to counter decrement
  - Modify: `src/network/stream.rs` — same
  - Modify: `src/cmd/mod.rs` — same
  - Modify: UI files that read `app.is_loading` — change to `app.is_loading()`
  - Test: `src/app/mod.rs` (inline tests module)

  **Approach:**
  - Replace `pub is_loading: bool` with `loading_counter: u32` (private)
  - Add `pub fn is_loading(&self) -> bool { self.loading_counter > 0 }`
  - In `dispatch()`: replace `self.is_loading = true` with `self.loading_counter += 1`; in error path set `self.loading_counter = self.loading_counter.saturating_sub(1)`
  - In network/stream/cmd handlers: replace `app.is_loading = false` with `app.loading_counter = app.loading_counter.saturating_sub(1)` (or expose a `decrement_loading()` method)
  - Grep for all `is_loading` usages and update

  **Patterns to follow:**
  - Saturating arithmetic pattern already used in scroll methods

  **Test scenarios:**
  - Dispatch 3 events → `is_loading()` true; complete 2 → still true; complete last → false
  - Dispatch with send error → counter stays correct (not incremented or properly decremented)
  - Counter never goes below 0 (saturating sub)
  - `reset()` sets counter to 0

  **Verification:**
  - `cargo test` passes
  - `cargo clippy` clean

---

- [ ] **Unit 5: Replace blocking clipboard sleep with background thread**

  **Goal:** Clipboard copy no longer freezes the TUI for 100ms.

  **Requirements:** R5

  **Dependencies:** None

  **Files:**
  - Modify: `src/handlers/mod.rs`
  - Test: `src/handlers/mod.rs` (existing handler tests should still pass)

  **Approach:**
  - Move the entire clipboard operation (`ClipboardContext::new()`, `set_contents`, `sleep`) into a `std::thread::spawn` closure
  - Error reporting: since the handler can no longer call `app.handle_error` from inside the spawned thread, use `log::error!` for clipboard failures instead (clipboard is best-effort)
  - The 100ms sleep remains inside the spawned thread for platform compatibility, but no longer blocks the UI

  **Patterns to follow:**
  - `std::thread::spawn` is already used in `src/main.rs` for network threads

  **Test scenarios:**
  - Clipboard copy function returns immediately (does not block caller)
  - Existing handler tests pass unchanged

  **Verification:**
  - `cargo test` passes
  - Manual test: copy action in TUI has no visible frame skip

---

- [ ] **Unit 6: Normalize naming conventions**

  **Goal:** Consistent naming across `ActiveBlock` variants, `Data` fields, and type aliases.

  **Requirements:** R6

  **Dependencies:** None (but must land before Unit 7's macro work since the macro will reference the new names)

  **Files:**
  - Modify: `src/app/mod.rs` — `ActiveBlock` enum, `Data` struct, `Default` impl
  - Modify: `src/handlers/mod.rs` — all match arms referencing renamed variants/fields
  - Modify: `src/ui/resource_tabs.rs` — tab dispatch referencing renamed variants
  - Modify: `src/network/mod.rs` — if it references `ActiveBlock`
  - Modify: Resource module files that reference `ActiveBlock` variants
  - Test: Existing tests updated to use new names

  **Approach:**
  Rename the following:

  | Current | New |
  |---|---|
  | `ActiveBlock::RplCtrl` | `ActiveBlock::ReplicationControllers` |
  | `ActiveBlock::ClusterRoleBinding` | `ActiveBlock::ClusterRoleBindings` |
  | `ActiveBlock::Pvc` | `ActiveBlock::PersistentVolumeClaims` |
  | `ActiveBlock::Pv` | `ActiveBlock::PersistentVolumes` |
  | `ActiveBlock::Ingress` | `ActiveBlock::Ingresses` |
  | `Data.rpl_ctrls` | `Data.replication_controllers` |
  | `Data.nw_policies` | `Data.network_policies` |
  | `Data.pvcs` | `Data.persistent_volume_claims` |
  | `Data.pvs` | `Data.persistent_volumes` |

  Use find-and-replace across the codebase. Each rename is mechanical.

  **Patterns to follow:**
  - Existing full-name variants: `Deployments`, `StatefulSets`, `ReplicaSets`, `NetworkPolicies`, `ServiceAccounts`, `StorageClasses`

  **Test scenarios:**
  - All existing tests compile and pass with new names (purely mechanical rename)
  - No stale references remain (grep for old names)

  **Verification:**
  - `cargo test` passes
  - `grep -r "RplCtrl\|\.rpl_ctrls\|\.nw_policies\|\.pvcs\|\.pvs\|ClusterRoleBinding[^s]" src/` returns no matches

---

- [ ] **Unit 7: Reduce handler and network dispatch boilerplate**

  **Goal:** Replace 20+ near-identical match arms with a declarative macro, reducing per-resource boilerplate.

  **Requirements:** R7

  **Dependencies:** Unit 6 (naming normalization should land first so the macro uses clean names)

  **Files:**
  - Create: `src/handlers/resource_dispatch.rs` (or inline macro in `src/handlers/mod.rs`)
  - Modify: `src/handlers/mod.rs` — replace block action and scroll dispatch match arms with macro invocations
  - Modify: `src/network/mod.rs` — replace `handle_network_event` match arms with macro invocation
  - Test: Existing handler and network tests must continue passing

  **Approach:**
  - Define a `resource_dispatch!` macro that takes a list of `(ActiveBlock variant, data field, kubectl kind, IoEvent variant)` tuples
  - Generate the repetitive match arms for: (1) block action handling, (2) scroll handling, (3) network event dispatch
  - Keep non-standard dispatch arms (Pods with containers, Logs, DynamicResource) as explicit match arms outside the macro
  - Start with the handler dispatch (highest duplication), then extend to network if the pattern fits cleanly

  **Patterns to follow:**
  - `draw_resource_tab!` macro in `src/ui/utils.rs` — existing precedent for resource dispatch macros in this codebase

  **Test scenarios:**
  - All 76+ existing tests pass without modification (the macro generates identical code)
  - Adding a hypothetical new resource would require ~1 macro entry instead of 7 file changes

  **Verification:**
  - `cargo test` passes
  - `cargo clippy` clean
  - Handler match arms reduced from 20+ explicit arms to macro invocation + a few special cases

## System-Wide Impact

- **Interaction graph:** The `is_loading` counter change touches the dispatch path (`App::dispatch`), all three network/stream/cmd handler completions, and all UI rendering that reads the loading state. All paths must be updated atomically in Unit 4.
- **Error propagation:** Clipboard errors shift from `app.handle_error()` (user-visible toast) to `log::error!` (log only). This is acceptable since clipboard is best-effort.
- **State lifecycle risks:** The loading counter must be `saturating_sub` to avoid underflow. The `reset()` method must zero the counter.
- **API surface parity:** `is_loading` changes from a public field to a public method — all call sites must switch from field access to method call.
- **Integration coverage:** Naming changes (Unit 6) are purely mechanical but touch many files — rely on compiler to catch all stale references.

## Risks & Dependencies

- **Naming rename breadth:** Unit 6 touches many files and could conflict with other in-flight changes. Mitigated by doing it as one atomic commit.
- **Macro complexity:** Unit 7's macro must handle several variations (namespaced vs cluster-scoped, with/without describe). If complexity grows beyond what a declarative macro can handle, fall back to keeping explicit match arms for outliers.
- **IDE interference:** The IDE on this machine auto-bumps Cargo.toml deps. Use `git checkout HEAD -- Cargo.toml` before any cargo command.

## Sources & References

- **Origin document:** [docs/REVIEW.md](../REVIEW.md) — P2 findings #9 through #15
- **P0/P1 plan:** [docs/plans/2026-04-02-001-fix-p0-p1-review-findings-plan.md](2026-04-02-001-fix-p0-p1-review-findings-plan.md)
- Related code: `src/ui/utils.rs` `draw_resource_tab!` macro (boilerplate reduction precedent)
