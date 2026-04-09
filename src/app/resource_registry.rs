// Central registry macros for standard Kubernetes resources.
//
// Resources with special dispatch logic (Pods, Nodes, Logs, Metrics, Dynamic, etc.)
// are NOT included here — they use manual match arms.
//
// To add a new standard resource:
// 1. Create the resource file (e.g., src/app/myresource.rs)
// 2. Add its entry to both macro invocations (dispatch_standard_resource
//    in app/mod.rs and handle_standard_network_event in network/mod.rs)
// 3. Add its ActiveBlock variant, Data field, handler/scroll entries

/// Dispatches a standard resource fetch by matching ActiveBlock to IoEvent.
#[macro_export]
macro_rules! dispatch_standard_resource {
  ($self:expr, $active_block:expr, $(($block:path, $event:path)),* $(,)?) => {
    match $active_block {
      $( $block => { $self.dispatch($event).await; } )*
      _ => {}
    }
  };
}

/// Handles standard resource network events.
/// Returns `true` if the event was handled, `false` otherwise.
#[macro_export]
macro_rules! handle_standard_network_event {
  ($nw:expr, $io_event:expr, $(($event:pat, $resource:ty)),* $(,)?) => {
    match &$io_event {
      $( $event => {
        <$resource as $crate::app::models::AppResource>::get_resource($nw).await;
        true
      } )*
      _ => false,
    }
  };
}
