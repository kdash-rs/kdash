use k8s_openapi::{
  apimachinery::pkg::apis::meta::v1::Time,
  chrono::{DateTime, Duration, Utc},
};
use kube::{Resource, ResourceExt};

pub fn sanitize_obj<K: Resource>(mut obj: K) -> K {
  obj.managed_fields_mut().clear();
  obj
}

pub static UNKNOWN: &str = "Unknown";

pub fn to_age(timestamp: Option<&Time>, against: DateTime<Utc>) -> String {
  match timestamp {
    Some(time) => {
      let time = time.0;
      let duration = against.signed_duration_since(time);

      duration_to_age(duration, false)
    }
    None => String::default(),
  }
}

pub fn to_age_secs(timestamp: Option<&Time>, against: DateTime<Utc>) -> String {
  match timestamp {
    Some(time) => {
      let time = time.0;
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

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
  use k8s_openapi::{
    apimachinery::pkg::apis::meta::v1::Time,
    chrono::{DateTime, TimeZone, Utc},
  };

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

  fn get_time(s: &str) -> Time {
    Time(to_utc(s))
  }

  fn to_utc(s: &str) -> DateTime<Utc> {
    Utc.datetime_from_str(s, "%d-%m-%Y %H:%M:%S").unwrap()
  }

  #[test]
  fn test_to_age_secs() {
    use std::time::SystemTime;

    use super::to_age_secs;

    assert_eq!(
      to_age_secs(Some(&Time(Utc::now())), Utc::now()),
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
        Some(&Time(DateTime::from(SystemTime::UNIX_EPOCH))),
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
      to_age(Some(&Time(Utc::now())), Utc::now()),
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
        Some(&Time(DateTime::from(SystemTime::UNIX_EPOCH))),
        to_utc("15-4-2021 14:10:0")
      ),
      String::from("2676w14h")
    );
  }
}
