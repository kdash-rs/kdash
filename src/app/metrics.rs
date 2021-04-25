// adapted from https://github.com/davidB/kubectl-view-allocations
// TODO move to use this directly from dependency if https://github.com/davidB/kubectl-view-allocations/issues/128 is resolved

use anyhow::{anyhow, Context, Error};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
  pub cpu: String,
  pub memory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
  pub name: String,
  pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodMetrics {
  pub metadata: kube::api::ObjectMeta,
  pub containers: Vec<Container>,
  pub timestamp: String,
  pub window: String,
}

#[derive(Debug, Clone, Default)]
pub struct Location {
  pub node_name: Option<String>,
  pub namespace: Option<String>,
  pub pod_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Resource {
  pub kind: String,
  pub quantity: Qty,
  pub location: Location,
  pub qualifier: ResourceQualifier,
}

#[derive(Debug, Clone)]
pub enum ResourceQualifier {
  Limit,
  Requested,
  Allocatable,
  Utilization,
}

#[derive(Debug, Clone, Default)]
pub struct QtyByQualifier {
  pub limit: Option<Qty>,
  pub requested: Option<Qty>,
  pub allocatable: Option<Qty>,
  pub utilization: Option<Qty>,
}

impl QtyByQualifier {
  pub fn calc_free(&self) -> Option<Qty> {
    let total_used = std::cmp::max(self.limit.as_ref(), self.requested.as_ref());
    self
      .allocatable
      .as_ref()
      .zip(total_used)
      .map(|(allocatable, total_used)| {
        if allocatable > total_used {
          allocatable - total_used
        } else {
          Qty::default()
        }
      })
  }
}

// see [Definitions of the SI units: The binary prefixes](https://physics.nist.gov/cuu/Units/binary.html)
// see [Managing Compute Resources for Containers - Kubernetes](https://kubernetes.io/docs/concepts/configuration/manage-compute-resources-container/)
//TODO rewrite to support exponent, ... see [apimachinery/quantity.go at master · kubernetes/apimachinery](https://github.com/kubernetes/apimachinery/blob/master/pkg/api/resource/quantity.go)

#[derive(Debug, Clone, Eq, PartialEq, Default)]
struct Scale {
  label: &'static str,
  base: u32,
  pow: i32,
}

// should be sorted in DESC
#[rustfmt::skip]
static SCALES: [Scale;13] = [
    Scale{ label:"Pi", base: 2, pow: 50},
    Scale{ label:"Ti", base: 2, pow: 40},
    Scale{ label:"Gi", base: 2, pow: 30},
    Scale{ label:"Mi", base: 2, pow: 20},
    Scale{ label:"Ki", base: 2, pow: 10},
    Scale{ label:"P", base: 10, pow: 15},
    Scale{ label:"T", base: 10, pow: 12},
    Scale{ label:"G", base: 10, pow: 9},
    Scale{ label:"M", base: 10, pow: 6},
    Scale{ label:"k", base: 10, pow: 3},
    Scale{ label:"", base: 10, pow: 0},
    Scale{ label:"m", base: 10, pow: -3},
    Scale{ label:"n", base: 10, pow: -9},
];

impl FromStr for Scale {
  type Err = Error;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    SCALES
      .iter()
      .find(|v| v.label == s)
      .cloned()
      .ok_or_else(|| anyhow!("scale not found in {}", s))
  }
}

impl From<&Scale> for f64 {
  fn from(v: &Scale) -> f64 {
    if v.pow == 0 || v.base == 0 {
      1.0
    } else {
      f64::from(v.base).powf(f64::from(v.pow))
    }
  }
}

impl PartialOrd for Scale {
  //TODO optimize accuracy with big number
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    let v1 = f64::from(self);
    let v2 = f64::from(other);
    if v1 > v2 {
      Some(Ordering::Greater)
    } else if v1 < v2 {
      Some(Ordering::Less)
    } else if (v1 - v2).abs() < std::f64::EPSILON {
      Some(Ordering::Equal)
    } else {
      None
    }
  }
}

impl Scale {
  fn min(&self, other: &Scale) -> Scale {
    if self < other {
      self.clone()
    } else {
      other.clone()
    }
  }
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct Qty {
  value: i64,
  scale: Scale,
}

impl From<&Qty> for f64 {
  fn from(v: &Qty) -> f64 {
    (v.value as f64) * 0.001
  }
}

impl Qty {
  pub fn lowest_positive() -> Qty {
    Qty {
      value: 1,
      scale: Scale::from_str("m").unwrap(),
    }
  }

  pub fn is_zero(&self) -> bool {
    self.value == 0
  }

  pub fn calc_percentage(&self, base100: &Self) -> f64 {
    if base100.value != 0 {
      f64::from(self) * 100f64 / f64::from(base100)
    } else {
      core::f64::NAN
    }
  }

  pub fn adjust_scale(&self) -> Qty {
    let valuef64 = f64::from(self);
    let scale = SCALES
      .iter()
      .filter(|s| s.base == self.scale.base || self.scale.base == 0)
      .find(|s| f64::from(*s) <= valuef64);
    match scale {
      Some(scale) => Qty {
        value: self.value,
        scale: scale.clone(),
      },
      None => self.clone(),
    }
  }
}

impl FromStr for Qty {
  type Err = Error;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let (num_str, scale_str): (&str, &str) = match s
      .find(|c: char| !c.is_digit(10) && c != 'E' && c != 'e' && c != '+' && c != '-' && c != '.')
    {
      Some(pos) => (&s[..pos], &s[pos..]),
      None => (s, ""),
    };
    let scale = Scale::from_str(scale_str.trim())
      .with_context(|| format!("Failed to read Qty (scale) from {}", s))?;
    let num =
      f64::from_str(num_str).with_context(|| format!("Failed to read Qty (num) from {}", s))?;
    let value = (num * f64::from(&scale) * 1000f64) as i64;
    Ok(Qty { value, scale })
  }
}

impl std::fmt::Display for Qty {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{:.1}{}",
      (self.value as f64 / (f64::from(&self.scale) * 1000f64)),
      self.scale.label
    )
  }
}

impl PartialOrd for Qty {
  //TODO optimize accuracy with big number
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    let v1 = self.value; // f64::from(self);
    let v2 = other.value; // f64::from(other);
    v1.partial_cmp(&v2)
  }
}

impl Ord for Qty {
  //TODO optimize accuracy with big number
  fn cmp(&self, other: &Self) -> Ordering {
    let v1 = self.value; // f64::from(self);
    let v2 = other.value; // f64::from(other);
    v1.partial_cmp(&v2).unwrap() // i64 should always be comparable (no NaNs or anything crazy like that)
  }
}

fn select_scale_for_add(v1: &Qty, v2: &Qty) -> Scale {
  if v2.value == 0 {
    v1.scale.clone()
  } else if v1.value == 0 {
    v2.scale.clone()
  } else {
    v1.scale.min(&v2.scale)
  }
}

impl std::ops::Add for Qty {
  type Output = Qty;
  fn add(self, other: Self) -> Qty {
    &self + &other
  }
}

impl std::ops::Add for &Qty {
  type Output = Qty;
  fn add(self, other: Self) -> Qty {
    Qty {
      value: self.value + other.value,
      scale: select_scale_for_add(self, other),
    }
  }
}

impl<'b> std::ops::AddAssign<&'b Qty> for Qty {
  fn add_assign(&mut self, other: &'b Self) {
    *self = Qty {
      value: self.value + other.value,
      scale: select_scale_for_add(self, other),
    }
  }
}

impl std::ops::Sub for Qty {
  type Output = Qty;
  fn sub(self, other: Self) -> Qty {
    &self - &other
  }
}

impl std::ops::Sub for &Qty {
  type Output = Qty;
  fn sub(self, other: Self) -> Qty {
    Qty {
      value: self.value - other.value,
      scale: select_scale_for_add(self, other),
    }
  }
}

impl<'b> std::ops::SubAssign<&'b Qty> for Qty {
  fn sub_assign(&mut self, other: &'b Self) {
    *self = Qty {
      value: self.value - other.value,
      scale: select_scale_for_add(self, other),
    };
  }
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupBy {
  Resource,
  Node,
  Pod,
  Namespace,
}

impl GroupBy {
  pub fn to_fct(&self) -> fn(&Resource) -> Option<String> {
    match self {
      Self::Resource => Self::extract_kind,
      Self::Node => Self::extract_node_name,
      Self::Pod => Self::extract_pod_name,
      Self::Namespace => Self::extract_namespace,
    }
  }

  #[allow(clippy::unnecessary_wraps)]
  fn extract_kind(e: &Resource) -> Option<String> {
    Some(e.kind.clone())
  }

  fn extract_node_name(e: &Resource) -> Option<String> {
    e.location.node_name.clone()
  }

  fn extract_pod_name(e: &Resource) -> Option<String> {
    // We do not need to display "pods" resource types when grouping by pods
    if e.kind == "pods" {
      return None;
    }
    e.location.pod_name.clone()
  }

  fn extract_namespace(e: &Resource) -> Option<String> {
    e.location.namespace.clone()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use spectral::prelude::*;

  #[test]
  fn test_to_base() -> Result<(), Box<dyn std::error::Error>> {
    assert_that!(f64::from(&Qty::from_str("1k")?))
      .is_close_to(f64::from(&Qty::from_str("1000000m")?), 0.01);
    assert_that!(Qty::from_str("1Ki")?).is_equal_to(Qty {
      value: 1024000,
      scale: Scale {
        label: "Ki",
        base: 2,
        pow: 10,
      },
    });
    Ok(())
  }

  #[test]
  fn expectation_ok_for_adjust_scale() -> Result<(), Box<dyn std::error::Error>> {
    let cases = vec![
      ("1k", "1.0k"),
      ("10k", "10.0k"),
      ("100k", "100.0k"),
      ("999k", "999.0k"),
      ("1000k", "1.0M"),
      ("1999k", "2.0M"), //TODO 1.9M should be better ?
      ("1Ki", "1.0Ki"),
      ("10Ki", "10.0Ki"),
      ("100Ki", "100.0Ki"),
      ("1000Ki", "1000.0Ki"),
      ("1024Ki", "1.0Mi"),
      ("25641877504", "25.6G"),
      ("1770653738944", "1.8T"),
      ("1000m", "1.0"),
      ("100m", "100.0m"),
      ("1m", "1.0m"),
    ];
    for (input, expected) in cases {
      assert_that!(format!("{}", &Qty::from_str(input)?.adjust_scale()))
        .is_equal_to(expected.to_string());
    }
    Ok(())
  }

  #[test]
  fn test_display() -> Result<(), Box<dyn std::error::Error>> {
    let cases = vec![
      ("1k", "1.0k"),
      ("10k", "10.0k"),
      ("100k", "100.0k"),
      ("999k", "999.0k"),
      ("1000k", "1000.0k"),
      ("1999k", "1999.0k"),
      ("1Ki", "1.0Ki"),
      ("10Ki", "10.0Ki"),
      ("100Ki", "100.0Ki"),
      ("1000Ki", "1000.0Ki"),
      ("1024Ki", "1024.0Ki"),
      ("25641877504", "25641877504.0"),
      ("1000m", "1000.0m"),
      ("100m", "100.0m"),
      ("1m", "1.0m"),
      ("1000000n", "1000000.0n"),
      // lowest precision is m, under 1m value is trunked
      ("1n", "0.0n"),
      ("999999n", "0.0n"),
    ];
    for input in cases {
      assert_that!(format!("{}", &Qty::from_str(input.0)?)).is_equal_to(input.1.to_string());
      assert_that!(format!("{}", &Qty::from_str(input.1)?)).is_equal_to(input.1.to_string());
    }
    Ok(())
  }

  #[test]
  fn test_f64_from_scale() -> Result<(), Box<dyn std::error::Error>> {
    assert_that!(f64::from(&Scale::from_str("m")?)).is_close_to(0.001, 0.00001);
    Ok(())
  }

  #[test]
  fn test_f64_from_qty() -> Result<(), Box<dyn std::error::Error>> {
    assert_that!(f64::from(&Qty::from_str("20m")?)).is_close_to(0.020, 0.00001);
    assert_that!(f64::from(&Qty::from_str("300m")?)).is_close_to(0.300, 0.00001);
    assert_that!(f64::from(&Qty::from_str("1000m")?)).is_close_to(1.000, 0.00001);
    assert_that!(f64::from(&Qty::from_str("+1000m")?)).is_close_to(1.000, 0.00001);
    assert_that!(f64::from(&Qty::from_str("-1000m")?)).is_close_to(-1.000, 0.00001);
    assert_that!(f64::from(&Qty::from_str("3145728e3")?)).is_close_to(3145728000.000, 0.00001);
    Ok(())
  }

  #[test]
  fn test_add() -> Result<(), Box<dyn std::error::Error>> {
    assert_that!(
      &(Qty::from_str("1")?
        + Qty::from_str("300m")?
        + Qty::from_str("300m")?
        + Qty::from_str("300m")?
        + Qty::from_str("300m")?)
    )
    .is_equal_to(&Qty::from_str("2200m")?);
    assert_that!(&(Qty::default() + Qty::from_str("300m")?)).is_equal_to(Qty::from_str("300m")?);
    assert_that!(&(Qty::default() + Qty::from_str("16Gi")?)).is_equal_to(Qty::from_str("16Gi")?);
    assert_that!(&(Qty::from_str("20m")? + Qty::from_str("300m")?))
      .is_equal_to(Qty::from_str("320m")?);
    assert_that!(&(Qty::from_str("1k")? + Qty::from_str("300m")?))
      .is_equal_to(&Qty::from_str("1000300m")?);
    assert_that!(&(Qty::from_str("1Ki")? + Qty::from_str("1Ki")?))
      .is_equal_to(&Qty::from_str("2Ki")?);
    assert_that!(&(Qty::from_str("1Ki")? + Qty::from_str("1k")?)).is_equal_to(&Qty {
      value: 2024000,
      scale: Scale {
        label: "k",
        base: 10,
        pow: 3,
      },
    });
    Ok(())
  }
}
