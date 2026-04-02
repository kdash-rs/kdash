# KDash Codebase Review

**Scope:** Full codebase (~12K lines Rust, 41 files)
**Intent:** Architecture improvements, bug fixes, and quality assessment

---

## P0 - Critical

### 1. Multiple `#[tokio::main]` creates separate tokio runtimes
**File:** `src/main.rs:71,139,157,175`

`main()` uses `#[tokio::main]`, and then `start_network()`, `start_stream_network()`, and `start_cmd_runner()` each also use `#[tokio::main]`. Each `#[tokio::main]` creates an independent tokio runtime. These functions are called inside `std::thread::spawn`, meaning you have **4 separate tokio runtimes** instead of 1.

This is a known anti-pattern that:
- Wastes threads (each runtime creates its own thread pool)
- Prevents task cooperation across runtimes
- Can cause subtle bugs with tokio primitives that assume a shared runtime

**Fix:** Use `tokio::spawn` for the network tasks instead of `std::thread::spawn` + `#[tokio::main]`, or use `tokio::runtime::Handle::current()` to spawn onto the existing runtime.

### 2. UI loop holds mutex for entire frame cycle
**File:** `src/main.rs:202-242`

The main UI loop does `let mut app = app.lock().await` at the top of each iteration and holds the lock through drawing AND event handling. All three network threads are blocked from updating `App` state until the UI finishes its entire render + input cycle. At 250ms tick rate, network threads may stall significantly.

**Fix:** Minimize lock scope - lock, clone needed data, unlock, then render. Or switch to a message-passing architecture where the UI owns the state.

### 3. `unwrap()` on event::poll/read can crash the app
**File:** `src/event/events.rs:65`

```rust
if event::poll(timeout).unwrap() {
    match event::read().unwrap() {
```

These run in a background thread. If the terminal goes away or crossterm errors, the app panics in a non-main thread, which may not trigger the panic hook cleanly.

**Fix:** Handle errors gracefully - send an error event or break the loop.

---

## P1 - High Impact

### 4. Unbounded log buffer (memory leak)
**File:** `src/app/models.rs:252` (TODO comment acknowledges this)

`LogsState` uses a `VecDeque` with initial capacity 512 but **no upper bound**. A pod producing rapid logs will grow memory indefinitely. The existing TODO confirms this is known.

**Fix:** Add a max capacity (e.g., 10K records) and evict oldest entries via `pop_front()`.

### 5. `unwrap()` on secret decoding can panic
**File:** `src/app/secrets.rs:48`

```rust
Ok(decoded_bytes) => String::from_utf8(decoded_bytes).unwrap(),
```

If a Kubernetes secret value contains non-UTF8 binary data (e.g., TLS certificates, binary keys), this will panic.

**Fix:** Use `String::from_utf8_lossy()` or handle the error.

### 6. Sensitive data logged at INFO level
**File:** `src/network/mod.rs:98,113`

```rust
info!("env KUBECONFIG: {:?}", std::env::var_os("KUBECONFIG"));
// ...
info!("Kubernetes client config: {:?}", client_config);
```

The kubeconfig path and full client config (which may include tokens, certs) are logged at INFO level. Debug logs are written to `kdash-debug-*.log` in the current directory.

**Fix:** Log at `debug!` level, and redact sensitive fields from the client config.

### 7. Command injection risk in kubectl describe
**File:** `src/cmd/mod.rs:138-146`

```rust
async fn get_describe(&self, kind: String, value: String, ns: Option<String>) {
    let mut args = vec!["describe", kind.as_str(), value.as_str()];
    // ...
    let out = duct::cmd("kubectl", &args).stderr_null().read();
```

While `duct::cmd` passes args as a list (not shell-interpolated), resource names come from the Kubernetes API. A maliciously named resource (e.g., containing newlines or special characters) could cause unexpected kubectl behavior. Low risk since it's an argument array, but worth noting.

### 8. `cache_all_resource_data()` fetches everything on startup 
**File:** `src/app/mod.rs:595-622`

On first render AND every refresh, the app fires **24 sequential API calls** for all resource types, even though only the active tab's resources are visible. On a large cluster, this hammers the API server and delays initial render.

**Fix:** Lazy-load resources only when their tab becomes active. Pre-fetch only namespaces, nodes, and the default tab (pods).

**Status** Not fixed.
---

## P2 - Moderate

### 9. Massive boilerplate per resource type
**Files:** All `src/app/*.rs` resource files, `src/handlers/mod.rs`, `src/network/mod.rs`

Adding a new Kubernetes resource type requires touching **at minimum 7 files** and writing ~100 lines of nearly identical code:
1. New `src/app/<resource>.rs` with struct, From impl, KubeResource impl, AppResource impl
2. `src/app/mod.rs` - add module, add field to Data struct, add to Default impl
3. `src/network/mod.rs` - add IoEvent variant, add match arm
4. `src/handlers/mod.rs` - add ActiveBlock match arms (action + scroll)
5. `src/ui/resource_tabs.rs` - add render dispatch

The handler file alone (`src/handlers/mod.rs`) has **20+ near-identical blocks** like:
```rust
ActiveBlock::Secrets => {
    if let Some(res) = handle_block_action(key, &app.data.secrets) {
        let _ok = handle_describe_decode_or_yaml_action(key, app, &res,
            IoCmdEvent::GetDescribe { kind: "secret".to_owned(), ... }).await;
    }
}
```

**Fix:** Consider a registry pattern or macro that generates the boilerplate from a resource definition. The `draw_resource_tab!` macro is a good start but doesn't cover handlers/network.

### 10. Navigation stack can grow without bound
**File:** `src/app/mod.rs:526-542`

Every navigation action pushes to `navigation_stack`. Normal usage won't cause issues, but rapid tab switching (especially with polling resetting `is_routing`) accumulates stack entries that are never cleaned up.

### 11. `is_loading` flag race condition
**File:** `src/app/mod.rs:475-484`

`is_loading` is set to `true` before dispatching, and set to `false` in the network handler after completion. But since multiple dispatches happen in sequence (24 on startup), `is_loading` gets set back to `false` as soon as the *first* request completes, even though 23 others are still in-flight.

### 12. `ScrollableTxt` offset is `u16` - overflow on large content
**File:** `src/app/models.rs:213`

`ScrollableTxt.offset` is `u16`, meaning it wraps at 65535 lines. Large YAML descriptions or describe output could exceed this. `kubectl describe` on a complex resource can easily produce thousands of lines.

### 13. Bug in `get_lb_ext_ips` logic
**File:** `src/app/svcs.rs:211-212`

```rust
if external_ips.is_none() && !lb_ips.is_empty() {
    lb_ips.extend(external_ips.unwrap_or_default());
```

When `external_ips.is_none()` is true, the code enters the branch and calls `external_ips.unwrap_or_default()` which will always be an empty vec. The `extend` is a no-op. The condition should likely be `external_ips.is_some()`.

### 14. Blocking `thread::sleep` in clipboard copy
**File:** `src/handlers/mod.rs:750`

```rust
Ok(_) => thread::sleep(std::time::Duration::from_millis(100)),
```

This blocks the **main UI thread** for 100ms, causing a visible frame skip. The comment says "without this sleep the clipboard is not set in some OSes."

### 15. Inconsistent naming conventions
Throughout the codebase:
- `RplCtrl` vs `ReplicationControllers` vs `replication_controllers`
- `svcs` vs `services`, `pvcs`, `pvs`
- `nw_policies` vs `network_policies`
- `ClusterRoleBinding` (singular) vs `ClusterRoleBindings` (plural) in ActiveBlock

---

## P3 - Minor

### 16. Commented-out code
**File:** `src/app/mod.rs:424-427`

```rust
//   pub table_cols: u16,
//   pub dialog: Option<String>,
//   pub confirm: bool,
```

Dead code that should be removed.

### 17. `rand` dependency for loading indicator
**File:** `src/ui/mod.rs:1`

```rust
use rand::Rng;
```

Importing `rand` (a non-trivial dependency) likely just for the loading spinner animation. Could use a simple counter-based approach instead.

### 18. Test data contains real-looking tokens
**File:** `src/app/secrets.rs:163-165` (test data)

Test data includes what appear to be real JWT tokens and certificates. While they're likely from a test cluster, it's better practice to use obviously fake values.

---

## Coverage Notes

- **Test coverage:** Good unit tests for models, handlers, and resource parsing. No integration tests.
- **Error handling:** Generally uses `anyhow` well, but several `unwrap()` calls in non-test code (events, secrets, log file creation).
- **Dependency health:** `serde_yaml` 0.9 is deprecated in favor of alternatives. `k8s_openapi::chrono` re-export couples the chrono version to k8s-openapi.

## Verdict

**Functional but architecturally strained.** The app works well for its current scope, but the resource-type boilerplate pattern makes it expensive to maintain and extend. The P0 issues (multiple runtimes, mutex contention, unwrap panics) should be addressed as they affect correctness and stability. The P1 issues (memory leak, startup API load) affect user experience on real clusters.
