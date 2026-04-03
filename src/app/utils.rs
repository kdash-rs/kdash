use chrono::{DateTime, Duration, Utc};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::{Resource, ResourceExt};

pub fn sanitize_obj<K: Resource>(mut obj: K) -> K {
  obj.managed_fields_mut().clear();
  obj
}

pub static UNKNOWN: &str = "Unknown";

/// Convert a k8s-openapi `Time` (jiff::Timestamp) to a chrono `DateTime<Utc>`.
fn time_to_chrono(time: &Time) -> DateTime<Utc> {
  DateTime::from_timestamp(time.0.as_second(), time.0.subsec_nanosecond() as u32)
    .unwrap_or_default()
}

pub fn to_age(timestamp: Option<&Time>, against: DateTime<Utc>) -> String {
  match timestamp {
    Some(time) => {
      let time = time_to_chrono(time);
      let duration = against.signed_duration_since(time);

      duration_to_age(duration, false)
    }
    None => String::default(),
  }
}

pub fn to_age_secs(timestamp: Option<&Time>, against: DateTime<Utc>) -> String {
  match timestamp {
    Some(time) => {
      let time = time_to_chrono(time);
      let duration = against.signed_duration_since(time);

      duration_to_age(duration, true)
    }
    None => String::default(),
  }
}

pub fn duration_to_age(duration: Duration, with_secs: bool) -> String {
  let mut out = String::new();
  if duration.num_weeks() != 0 {
    out.push_str(format!("{}w", duration.num_weeks()).as_str());
  }
  let days = duration.num_days() - (duration.num_weeks() * 7);
  if days != 0 {
    out.push_str(format!("{}d", days).as_str());
  }
  let hrs = duration.num_hours() - (duration.num_days() * 24);
  if hrs != 0 {
    out.push_str(format!("{}h", hrs).as_str());
  }
  let mins = duration.num_minutes() - (duration.num_hours() * 60);
  if mins != 0 && days == 0 && duration.num_weeks() == 0 {
    out.push_str(format!("{}m", mins).as_str());
  }
  if with_secs {
    let secs = duration.num_seconds() - (duration.num_minutes() * 60);
    if secs != 0 && hrs == 0 && days == 0 && duration.num_weeks() == 0 {
      out.push_str(format!("{}s", secs).as_str());
    }
  }
  if out.is_empty() && with_secs {
    "0s".into()
  } else if out.is_empty() {
    "0m".into()
  } else {
    out
  }
}

pub fn mem_to_mi(mem: String) -> String {
  if mem.ends_with("Ki") {
    let mem = mem.trim_end_matches("Ki").parse::<i64>().unwrap_or(0);
    format!("{}Mi", mem / 1024)
  } else if mem.ends_with("Gi") {
    let mem = mem.trim_end_matches("Gi").parse::<i64>().unwrap_or(0);
    format!("{}Mi", mem * 1024)
  } else {
    mem
  }
}

pub fn cpu_to_milli(cpu: String) -> String {
  if cpu.ends_with('m') {
    cpu
  } else if cpu.ends_with('n') {
    format!(
      "{}m",
      (convert_to_f64(cpu.trim_end_matches('n')) / 1000000f64).floor()
    )
  } else {
    format!("{}m", (convert_to_f64(&cpu) * 1000f64).floor())
  }
}

pub fn to_cpu_percent(used: String, total: String) -> f64 {
  // convert from nano cpu to milli cpu
  let used = convert_to_f64(used.trim_end_matches('m'));
  let total = convert_to_f64(total.trim_end_matches('m'));

  to_percent(used, total)
}

pub fn to_mem_percent(used: String, total: String) -> f64 {
  let used = convert_to_f64(used.trim_end_matches("Mi"));
  let total = convert_to_f64(total.trim_end_matches("Mi"));

  to_percent(used, total)
}

pub fn to_percent(used: f64, total: f64) -> f64 {
  ((used / total) * 100f64).floor()
}

pub fn convert_to_f64(s: &str) -> f64 {
  s.parse().unwrap_or(0f64)
}

/// Extract a human-friendly type name from a full Rust type path.
/// e.g. `kdash::app::pods::KubePod` → `Pod`, `kdash::app::ns::KubeNs` → `Namespace`
pub fn friendly_type_name<T>() -> String {
  let full = std::any::type_name::<T>();
  // Take the last segment after ::
  let short = full.rsplit("::").next().unwrap_or(full);
  // Strip "Kube" prefix
  let name = short.strip_prefix("Kube").unwrap_or(short);
  // Expand common abbreviations
  match name {
    "Ns" => "Namespace".to_string(),
    "Svc" | "Svcs" => "Service".to_string(),
    "Pvc" | "Pvcs" => "PersistentVolumeClaim".to_string(),
    "Pv" | "Pvs" => "PersistentVolume".to_string(),
    _ => name.to_string(),
  }
}

/// Clean up an error message for UI display by extracting the root cause
/// and stripping verbose Rust type wrapper noise.
pub fn sanitize_error_message(e: &anyhow::Error) -> String {
  // Get the top-level message (from the anyhow! context)
  let top = e.to_string();

  // Walk the error chain to find the root cause message
  let root = e.root_cause().to_string();

  // If the top message already contains a clean description and differs
  // from root, combine them: "Top level: root cause"
  // If they're the same, just clean up the single message
  let msg = if top == root {
    clean_error_string(&top)
  } else {
    let clean_top = clean_error_string(&top);
    let clean_root = clean_error_string(&root);
    // Avoid duplication if the cleaned top already ends with root cause
    if clean_top.contains(&clean_root) {
      clean_top
    } else {
      format!("{}: {}", clean_top, clean_root)
    }
  };

  msg
}

/// Strip common Rust error wrapper noise from a single error string.
fn clean_error_string(s: &str) -> String {
  let mut result = s.to_string();

  // Strip full Rust module paths (e.g. "kdash::app::pods::KubePod" → "KubePod")
  // Pattern: word::word::...::Word
  let path_re = regex::Regex::new(r"\b(\w+::)+(\w+)\b").unwrap();
  result = path_re
    .replace_all(&result, |caps: &regex::Captures<'_>| {
      caps.get(2).map_or("", |m| m.as_str()).to_string()
    })
    .to_string();

  // Strip "Kube" prefix from type names that appear after stripping paths
  let kube_prefix_re = regex::Regex::new(r"\bKube(\w+)").unwrap();
  result = kube_prefix_re
    .replace_all(&result, |caps: &regex::Captures<'_>| {
      caps.get(1).map_or("", |m| m.as_str()).to_string()
    })
    .to_string();

  // Clean up nested Error(Connect, ConnectError(...)) patterns
  // Extract the innermost quoted message if present
  if let Some(inner) = extract_inner_message(&result) {
    return inner;
  }

  result.trim().to_string()
}

/// Try to extract a meaningful inner message from nested error wrappers.
/// Looks for the deepest quoted string or known error patterns.
fn extract_inner_message(s: &str) -> Option<String> {
  // If the string has nested type wrappers like Error(Kind, Wrapper("msg")),
  // try to find the core message
  let has_nested_wrappers = s.contains("Error(") || s.contains("Custom {");
  if !has_nested_wrappers {
    return None;
  }

  // Extract all quoted strings — the last one is typically the root cause
  let quote_re = regex::Regex::new(r#""([^"]+)""#).unwrap();
  let quotes: Vec<&str> = quote_re
    .captures_iter(s)
    .filter_map(|c| c.get(1).map(|m| m.as_str()))
    .collect();

  if let Some(&last_quote) = quotes.last() {
    // Build a cleaner message: prefix from before the wrapper + root quoted cause
    // Find everything before the first wrapper type
    let prefix_end = s
      .find("Error(")
      .or_else(|| s.find("Custom {"))
      .unwrap_or(s.len());
    let prefix = s[..prefix_end].trim().trim_end_matches('.');
    if prefix.is_empty() {
      Some(last_quote.to_string())
    } else {
      Some(format!("{}: {}", prefix, last_quote))
    }
  } else {
    None
  }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
  use chrono::{DateTime, Utc};
  use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;

  #[test]
  fn test_mem_to_mi() {
    use super::mem_to_mi;
    assert_eq!(mem_to_mi(String::from("2820Mi")), String::from("2820Mi"));
    assert_eq!(mem_to_mi(String::from("2888180Ki")), String::from("2820Mi"));
    assert_eq!(mem_to_mi(String::from("5Gi")), String::from("5120Mi"));
    assert_eq!(mem_to_mi(String::from("5")), String::from("5"));
  }
  #[test]
  fn test_to_cpu_percent() {
    use super::to_cpu_percent;
    assert_eq!(
      to_cpu_percent(String::from("126m"), String::from("940m")),
      13f64
    );
  }

  #[test]
  fn test_to_mem_percent() {
    use super::to_mem_percent;
    assert_eq!(
      to_mem_percent(String::from("645784Mi"), String::from("2888184Mi")),
      22f64
    );
  }
  #[test]
  fn test_cpu_to_milli() {
    use super::cpu_to_milli;
    assert_eq!(cpu_to_milli(String::from("645m")), String::from("645m"));
    assert_eq!(
      cpu_to_milli(String::from("126632173n")),
      String::from("126m")
    );
    assert_eq!(cpu_to_milli(String::from("8")), String::from("8000m"));
    assert_eq!(cpu_to_milli(String::from("0")), String::from("0m"));
  }

  fn chrono_to_jiff(dt: DateTime<Utc>) -> k8s_openapi::jiff::Timestamp {
    k8s_openapi::jiff::Timestamp::from_second(dt.timestamp()).unwrap()
  }

  fn get_time(s: &str) -> Time {
    Time(chrono_to_jiff(to_utc(s)))
  }

  fn to_utc(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_str(&format!("{} +0000", s), "%d-%m-%Y %H:%M:%S %z")
      .unwrap()
      .into()
  }

  #[test]
  fn test_to_age_secs() {
    use std::time::SystemTime;

    use super::to_age_secs;

    assert_eq!(
      to_age_secs(Some(&Time(chrono_to_jiff(Utc::now()))), Utc::now()),
      String::from("0s")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("15-4-2021 14:09:10")),
        to_utc("15-4-2021 14:10:00")
      ),
      String::from("50s")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("15-4-2021 14:08:10")),
        to_utc("15-4-2021 14:10:00")
      ),
      String::from("1m50s")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("15-4-2021 14:09:00")),
        to_utc("15-4-2021 14:10:00")
      ),
      String::from("1m")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("15-4-2021 13:50:00")),
        to_utc("15-4-2021 14:10:00")
      ),
      String::from("20m")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("15-4-2021 13:50:10")),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("19m50s")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("15-4-2021 10:50:10")),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("3h19m")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("14-4-2021 15:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("23h")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("14-4-2021 14:11:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("23h59m")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("14-4-2021 14:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("1d")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("12-4-2021 14:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("3d")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("12-4-2021 13:50:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("3d")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("12-4-2021 11:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("3d3h")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("12-4-2021 10:50:10")),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("3d3h")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("08-4-2021 14:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("1w")
    );
    assert_eq!(
      to_age_secs(
        Some(&get_time("05-4-2021 12:30:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("1w3d1h")
    );
    assert_eq!(
      to_age_secs(
        Some(&Time(chrono_to_jiff(DateTime::from(
          SystemTime::UNIX_EPOCH
        )))),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("2676w14h")
    );
  }
  #[test]
  fn test_to_age() {
    use std::time::SystemTime;

    use super::to_age;

    assert_eq!(
      to_age(Some(&Time(chrono_to_jiff(Utc::now()))), Utc::now()),
      String::from("0m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("15-4-2021 14:09:00")),
        to_utc("15-4-2021 14:10:00")
      ),
      String::from("1m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("15-4-2021 13:50:00")),
        to_utc("15-4-2021 14:10:00")
      ),
      String::from("20m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("15-4-2021 13:50:10")),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("19m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("15-4-2021 10:50:10")),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("3h19m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("14-4-2021 15:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("23h")
    );
    assert_eq!(
      to_age(
        Some(&get_time("14-4-2021 14:11:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("23h59m")
    );
    assert_eq!(
      to_age(
        Some(&get_time("14-4-2021 14:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("1d")
    );
    assert_eq!(
      to_age(
        Some(&get_time("12-4-2021 14:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("3d")
    );
    assert_eq!(
      to_age(
        Some(&get_time("12-4-2021 13:50:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("3d")
    );
    assert_eq!(
      to_age(
        Some(&get_time("12-4-2021 11:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("3d3h")
    );
    assert_eq!(
      to_age(
        Some(&get_time("12-4-2021 10:50:10")),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("3d3h")
    );
    assert_eq!(
      to_age(
        Some(&get_time("08-4-2021 14:10:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("1w")
    );
    assert_eq!(
      to_age(
        Some(&get_time("05-4-2021 12:30:10")),
        to_utc("15-4-2021 14:10:10")
      ),
      String::from("1w3d1h")
    );
    assert_eq!(
      to_age(
        Some(&Time(chrono_to_jiff(DateTime::from(
          SystemTime::UNIX_EPOCH
        )))),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("2676w14h")
    );
  }

  #[test]
  fn test_friendly_type_name_strips_module_path() {
    assert_eq!(
      super::friendly_type_name::<super::super::pods::KubePod>(),
      "Pod"
    );
  }

  #[test]
  fn test_friendly_type_name_expands_abbreviations() {
    assert_eq!(
      super::friendly_type_name::<super::super::ns::KubeNs>(),
      "Namespace"
    );
  }

  #[test]
  fn test_clean_error_string_strips_module_paths() {
    let input = "Failed to get namespaced resource kdash::app::pods::KubePod. some error";
    let result = super::clean_error_string(input);
    assert_eq!(result, "Failed to get namespaced resource Pod. some error");
  }

  #[test]
  fn test_clean_error_string_strips_kube_prefix() {
    let input = "Failed for KubeNode";
    let result = super::clean_error_string(input);
    assert_eq!(result, "Failed for Node");
  }

  #[test]
  fn test_extract_inner_message_from_nested_errors() {
    let input = r#"Failed to get namespaced resource Pod. Service(Error(Connect, ConnectError("dns error", Custom { kind: Other, error: "something" })))"#;
    let result = super::clean_error_string(input);
    assert!(result.contains("dns error") || result.contains("something"));
    assert!(!result.contains("ConnectError"));
  }

  #[test]
  fn test_sanitize_error_message_simple() {
    let e = anyhow::anyhow!("connection refused");
    let result = super::sanitize_error_message(&e);
    assert_eq!(result, "connection refused");
  }

  #[test]
  fn test_sanitize_error_message_with_context() {
    let inner = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "connection refused");
    let e = anyhow::anyhow!(inner).context("Failed to connect to cluster");
    let result = super::sanitize_error_message(&e);
    assert!(result.contains("Failed to connect to cluster"));
    assert!(result.contains("connection refused"));
  }

  #[test]
  fn test_sanitize_error_message_strips_module_paths() {
    let e = anyhow::anyhow!("Failed to get namespaced resource kdash::app::pods::KubePod. timeout");
    let result = super::sanitize_error_message(&e);
    assert!(result.contains("Pod"));
    assert!(!result.contains("kdash::app::pods"));
    assert!(!result.contains("KubePod"));
  }

  #[test]
  fn test_extract_inner_message_returns_none_for_simple() {
    assert!(super::extract_inner_message("simple error message").is_none());
  }

  #[test]
  fn test_extract_inner_message_extracts_from_nested() {
    let input = r#"Service Error(Connect, "connection refused")"#;
    let result = super::extract_inner_message(input);
    assert!(result.is_some());
    assert!(result.unwrap().contains("connection refused"));
  }
}
