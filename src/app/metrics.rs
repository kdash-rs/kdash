// adapted from https://github.com/davidB/kubectl-view-allocations
// TODO move to use this directly from dependency if https://github.com/davidB/kubectl-view-allocations/issues/128 is resolved

use anyhow::{anyhow, Context, Error, Result};
use itertools::Itertools;
use k8s_openapi::{
  api::core::v1::{Node, Pod},
  apimachinery::pkg::api::resource,
};
use kube::api::ObjectList;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BTreeMap};
use std::{collections::HashMap, str::FromStr};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Usage {
  cpu: String,
  memory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Container {
  name: String,
  usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodMetrics {
  metadata: kube::api::ObjectMeta,
  containers: Vec<Container>,
  timestamp: String,
  window: String,
}

#[derive(Debug, Clone, Default)]
struct Location {
  node_name: Option<String>,
  namespace: Option<String>,
  pod_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Resource {
  kind: String,
  quantity: Qty,
  location: Location,
  qualifier: ResourceQualifier,
}

#[derive(Debug, Clone)]
enum ResourceQualifier {
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
      .find(|scale| scale.label == s)
      .cloned()
      .ok_or_else(|| anyhow!("scale not found in {}", s))
  }
}

impl From<&Scale> for f64 {
  fn from(scale: &Scale) -> f64 {
    if scale.pow == 0 || scale.base == 0 {
      1.0
    } else {
      f64::from(scale.base).powf(f64::from(scale.pow))
    }
  }
}

impl PartialOrd for Scale {
  //TODO optimize accuracy with big number
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    let v_self = f64::from(self);
    let v_other = f64::from(other);
    if v_self > v_other {
      Some(Ordering::Greater)
    } else if v_self < v_other {
      Some(Ordering::Less)
    } else if (v_self - v_other).abs() < std::f64::EPSILON {
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
  fn from(qty: &Qty) -> f64 {
    (qty.value as f64) * 0.001
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
  fn from_str(val: &str) -> Result<Self, Self::Err> {
    let (num_str, scale_str): (&str, &str) = match val
      .find(|c: char| !c.is_digit(10) && c != 'E' && c != 'e' && c != '+' && c != '-' && c != '.')
    {
      Some(pos) => (&val[..pos], &val[pos..]),
      None => (val, ""),
    };
    let scale = Scale::from_str(scale_str.trim())
      .with_context(|| format!("Failed to read Qty (scale) from {}", val))?;
    let num =
      f64::from_str(num_str).with_context(|| format!("Failed to read Qty (num) from {}", val))?;
    let value = (num * f64::from(&scale) * 1000f64) as i64;
    Ok(Qty { value, scale })
  }
}

impl std::fmt::Display for Qty {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      formatter,
      "{:.1}{}",
      (self.value as f64 / (f64::from(&self.scale) * 1000f64)),
      self.scale.label
    )
  }
}

impl PartialOrd for Qty {
  //TODO optimize accuracy with big number
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    let v_self = self.value; // f64::from(self);
    let v_other = other.value; // f64::from(other);
    v_self.partial_cmp(&v_other)
  }
}

impl Ord for Qty {
  //TODO optimize accuracy with big number
  fn cmp(&self, other: &Self) -> Ordering {
    let v_self = self.value; // f64::from(self);
    let v_other = other.value; // f64::from(other);
    v_self.partial_cmp(&v_other).unwrap() // i64 should always be comparable (no NaNs or anything crazy like that)
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
  fn extract_kind(r: &Resource) -> Option<String> {
    Some(r.kind.clone())
  }

  fn extract_node_name(r: &Resource) -> Option<String> {
    r.location.node_name.clone()
  }

  fn extract_pod_name(r: &Resource) -> Option<String> {
    // We do not need to display "pods" resource types when grouping by pods
    if r.kind == "pods" {
      return None;
    }
    r.location.pod_name.clone()
  }

  fn extract_namespace(r: &Resource) -> Option<String> {
    r.location.namespace.clone()
  }
}

impl Resource {
  pub async fn compute_utilizations_metrics(
    pod_metrics: ObjectList<PodMetrics>,
    resources: &mut Vec<Resource>,
  ) -> Result<()> {
    let cpu_kind = "cpu";
    let memory_kind = "memory";
    let locations = extract_locations(resources);
    for pod_metric in pod_metrics.items {
      let metadata = &pod_metric.metadata;
      let key = (
        metadata.namespace.clone().unwrap_or_default(),
        metadata.name.clone().unwrap_or_default(),
      );
      let location = locations.get(&key).cloned().unwrap_or_else(|| Location {
        // node_name: node_name.clone(),
        namespace: metadata.namespace.clone(),
        pod_name: metadata.name.clone(),
        ..Location::default()
      });
      let mut cpu_utilization = Qty::default();
      let mut memory_utilization = Qty::default();
      for container in pod_metric.containers.into_iter() {
        cpu_utilization += &Qty::from_str(&container.usage.cpu)?.max(Qty::lowest_positive());
        memory_utilization += &Qty::from_str(&container.usage.memory)?.max(Qty::lowest_positive());
      }

      resources.push(Resource {
        kind: cpu_kind.to_owned(),
        qualifier: ResourceQualifier::Utilization,
        quantity: cpu_utilization,
        location: location.clone(),
      });
      resources.push(Resource {
        kind: memory_kind.to_owned(),
        qualifier: ResourceQualifier::Utilization,
        quantity: memory_utilization,
        location: location.clone(),
      });
    }
    Ok(())
  }

  pub async fn compute_pod_utilizations(
    pod_list: ObjectList<Pod>,
    resources: &mut Vec<Resource>,
  ) -> Result<()> {
    for pod in pod_list.items.into_iter().filter(is_scheduled) {
      let spec = pod.spec.as_ref();
      let node_name = spec.and_then(|s| s.node_name.clone());
      let metadata = &pod.metadata;
      let location = Location {
        node_name: node_name.clone(),
        namespace: metadata.namespace.clone(),
        pod_name: metadata.name.clone(),
      };
      // compute the effective resource qualifier
      // see https://kubernetes.io/docs/concepts/workloads/pods/init-containers/#resources
      let mut resource_requests: BTreeMap<String, Qty> = BTreeMap::new();
      let mut resource_limits: BTreeMap<String, Qty> = BTreeMap::new();
      // handle regular containers
      let containers = spec.map(|s| s.containers.clone()).unwrap_or_default();
      for container in containers.into_iter() {
        if let Some(requirements) = container.resources {
          if let Some(r) = requirements.requests {
            process_resources(&mut resource_requests, &r, std::ops::Add::add)?;
          }
          if let Some(l) = requirements.limits {
            process_resources(&mut resource_limits, &l, std::ops::Add::add)?;
          }
        }
      }
      // handle initContainers
      let init_containers = spec
        .and_then(|s| s.init_containers.clone())
        .unwrap_or_default();
      for container in init_containers.into_iter() {
        if let Some(requirements) = container.resources {
          if let Some(r) = requirements.requests {
            process_resources(&mut resource_requests, &r, std::cmp::max)?;
          }
          if let Some(l) = requirements.limits {
            process_resources(&mut resource_limits, &l, std::cmp::max)?;
          }
        }
      }
      // handler overhead (add to both requests and limits)
      if let Some(overhead) = spec.and_then(|s| s.overhead.as_ref()) {
        process_resources(&mut resource_requests, &overhead, std::ops::Add::add)?;
        process_resources(&mut resource_limits, &overhead, std::ops::Add::add)?;
      }
      // push these onto resources
      push_resources(
        resources,
        &location,
        ResourceQualifier::Requested,
        &resource_requests,
      )?;
      push_resources(
        resources,
        &location,
        ResourceQualifier::Limit,
        &resource_limits,
      )?;
    }
    Ok(())
  }

  pub async fn compute_node_utilizations(
    node_list: ObjectList<Node>,
    resources: &mut Vec<Resource>,
  ) -> Result<()> {
    for node in node_list.items {
      let location = Location {
        node_name: node.metadata.name,
        ..Location::default()
      };
      if let Some(als) = node.status.and_then(|status| status.allocatable) {
        // add_resource(resources, &location, ResourceUsage::Allocatable, &als)?
        for (kind, value) in als.iter() {
          let quantity = Qty::from_str(&(value).0)?;
          resources.push(Resource {
            kind: kind.clone(),
            qualifier: ResourceQualifier::Allocatable,
            quantity,
            location: location.clone(),
          });
        }
      }
    }
    Ok(())
  }

  pub fn make_qualifiers(
    rsrcs: &[Resource],
    group_by: &[GroupBy],
  ) -> Vec<(Vec<String>, Option<QtyByQualifier>)> {
    let group_by_fct = group_by.iter().map(GroupBy::to_fct).collect::<Vec<_>>();
    let mut out = make_group_x_qualifier(&rsrcs.iter().collect::<Vec<_>>(), &[], &group_by_fct, 0);
    out.sort_by_key(|i| i.0.clone());
    out
  }
}

fn make_group_x_qualifier(
  rsrcs: &[&Resource],
  prefix: &[String],
  group_by_fct: &[fn(&Resource) -> Option<String>],
  group_by_depth: usize,
) -> Vec<(Vec<String>, Option<QtyByQualifier>)> {
  // Note: The `&` is significant here, `GroupBy` is iterable
  // only by reference. You can also call `.into_iter()` explicitly.
  let mut out = vec![];
  if let Some(group_by) = group_by_fct.get(group_by_depth) {
    for (key, group) in rsrcs
      .iter()
      .filter_map(|r| group_by(r).map(|s| (s, *r)))
      .into_group_map()
    {
      let mut key_full = prefix.to_vec();
      key_full.push(key);
      let children = make_group_x_qualifier(&group, &key_full, group_by_fct, group_by_depth + 1);
      out.push((key_full, sum_by_qualifier(&group)));
      out.extend(children);
    }
  }
  // let kg = &rsrcs.into_iter().group_by(|v| v.kind);
  // kg.into_iter().map(|(key, group)|  ).collect()
  out
}

fn add(lhs: Option<Qty>, rhs: &Qty) -> Option<Qty> {
  lhs.map(|l| &l + rhs).or_else(|| Some(rhs.clone()))
}

fn sum_by_qualifier(rsrcs: &[&Resource]) -> Option<QtyByQualifier> {
  if !rsrcs.is_empty() {
    let kind = rsrcs
      .get(0)
      .expect("group contains at least 1 element")
      .kind
      .clone();

    if rsrcs.iter().all(|r| r.kind == kind) {
      let sum = rsrcs.iter().fold(QtyByQualifier::default(), |mut acc, r| {
        match &r.qualifier {
          ResourceQualifier::Limit => acc.limit = add(acc.limit, &r.quantity),
          ResourceQualifier::Requested => acc.requested = add(acc.requested, &r.quantity),
          ResourceQualifier::Allocatable => acc.allocatable = add(acc.allocatable, &r.quantity),
          ResourceQualifier::Utilization => acc.utilization = add(acc.utilization, &r.quantity),
        };
        acc
      });
      Some(sum)
    } else {
      None
    }
  } else {
    None
  }
}

fn is_scheduled(pod: &Pod) -> bool {
  pod
    .status
    .as_ref()
    .and_then(|ps| {
      ps.phase.as_ref().and_then(|phase| {
        match &phase[..] {
          "Succeeded" | "Failed" => Some(false),
          "Running" => Some(true),
          "Unknown" => None, // this is the case when a node is down (kubelet is not responding)
          "Pending" => ps.conditions.as_ref().map(|s| {
            s.iter()
              .any(|c| c.type_ == "PodScheduled" && c.status == "True")
          }),
          &_ => None, // should not happen
        }
      })
    })
    .unwrap_or(false)
}

fn process_resources<F>(
  effective_resources: &mut BTreeMap<String, Qty>,
  resource_list: &BTreeMap<String, resource::Quantity>,
  op: F,
) -> Result<()>
where
  F: Fn(Qty, Qty) -> Qty,
{
  for (key, value) in resource_list.iter() {
    let quantity = Qty::from_str(&(value).0)?;
    if let Some(current_quantity) = effective_resources.get_mut(key) {
      *current_quantity = op(current_quantity.clone(), quantity).clone();
    } else {
      effective_resources.insert(key.clone(), quantity.clone());
    }
  }
  Ok(())
}

fn push_resources(
  resources: &mut Vec<Resource>,
  location: &Location,
  qualifier: ResourceQualifier,
  resource_list: &BTreeMap<String, Qty>,
) -> Result<()> {
  for (key, quantity) in resource_list.iter() {
    resources.push(Resource {
      kind: key.clone(),
      qualifier: qualifier.clone(),
      quantity: quantity.clone(),
      location: location.clone(),
    });
  }
  // add a "pods" resource as well
  resources.push(Resource {
    kind: "pods".to_owned(),
    qualifier,
    quantity: Qty::from_str("1")?,
    location: location.clone(),
  });
  Ok(())
}

fn extract_locations(resources: &[Resource]) -> HashMap<(String, String), Location> {
  resources
    .iter()
    .filter_map(|resource| {
      let loc = &resource.location;
      loc.pod_name.as_ref().map(|n| {
        (
          (loc.namespace.clone().unwrap_or_default(), n.to_owned()),
          loc.clone(),
        )
      })
    })
    .collect()
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
