use super::super::app::{
  contexts,
  metrics::{self, GroupBy, Location, Qty, QtyByQualifier, Resource, ResourceQualifier},
  nodes::{KubeNode, NodeMetrics},
  ns::KubeNs,
  pods::KubePods,
  svcs::KubeSvs,
};
use super::Network;

use anyhow::{anyhow, Result};
use itertools::Itertools;
use k8s_openapi::{
  api::core::v1::{Namespace, Node, Pod, Service},
  apimachinery::pkg::api::resource,
};
use kube::{
  api::{DynamicObject, GroupVersionKind, ListParams, ObjectList, Request},
  config::Kubeconfig,
  Api, Client, Resource as KubeResource,
};
use std::{
  collections::{BTreeMap, HashMap},
  str::FromStr,
};

impl<'a> Network<'a> {
  pub async fn get_kube_config(&self) {
    match Kubeconfig::read() {
      Ok(config) => {
        let mut app = self.app.lock().await;
        app.set_contexts(contexts::get_contexts(&config));
        app.data.kubeconfig = Some(config);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  pub async fn get_node_metrics(&self) {
    let gvk = GroupVersionKind::gvk("metrics.k8s.io", "v1beta1", "nodemetrics").unwrap();
    let node_metrics: Api<DynamicObject> = Api::all_with(self.client.clone(), &gvk);
    match node_metrics.list(&ListParams::default()).await {
      Ok(metrics) => {
        let mut app = self.app.lock().await;

        let rows = metrics
          .items
          .iter()
          .map(|metric| NodeMetrics::from_api(metric, &app))
          .collect();

        app.data.node_metrics = rows;
      }
      Err(_) => {
        let mut app = self.app.lock().await;
        app.data.node_metrics = vec![];
      }
    };
  }

  pub async fn get_utilizations(&self) {
    let mut resources: Vec<Resource> = vec![];
    if let Err(e) = get_node_utilizations(self.client.clone(), &mut resources).await {
      self.handle_error(anyhow!(e)).await;
    }

    let pods: Api<Pod> = self.get_namespaced_api().await;
    if let Err(e) = get_pod_utilizations(pods, &mut resources).await {
      self.handle_error(anyhow!(e)).await;
    }
    if let Err(_e) = get_utilizations_metrics(self.client.clone(), &mut resources).await {
      // don't do anything to avoid showing constant error when metric-server is not found,
      // since its not a mandatory component in a cluster
      // TODO may be show a non intrusive warning
    }

    let mut app = self.app.lock().await;

    let data = make_qualifiers(&resources, &app.utilization_group_by);

    app.data.metrics.set_items(data);
  }

  pub async fn get_nodes(&self) {
    let lp = ListParams::default();
    let pods: Api<Pod> = Api::all(self.client.clone());
    let nodes: Api<Node> = Api::all(self.client.clone());

    match nodes.list(&lp).await {
      Ok(node_list) => {
        self.get_node_metrics().await;
        let pods_list = pods.list(&lp).await;

        let mut app = self.app.lock().await;

        let render_nodes = node_list
          .iter()
          .map(|node| KubeNode::from_api(node, &pods_list, &mut app))
          .collect::<Vec<_>>();

        app.data.nodes.set_items(render_nodes);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  pub async fn get_namespaces(&self) {
    let ns: Api<Namespace> = Api::all(self.client.clone());

    let lp = ListParams::default();
    match ns.list(&lp).await {
      Ok(ns_list) => {
        let nss = ns_list
          .iter()
          .map(|ns| KubeNs::from_api(ns))
          .collect::<Vec<_>>();
        let mut app = self.app.lock().await;
        app.data.namespaces.set_items(nss);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  pub async fn get_pods(&self) {
    let pods: Api<Pod> = self.get_namespaced_api().await;

    let lp = ListParams::default();
    match pods.list(&lp).await {
      Ok(pod_list) => {
        let render_pods = pod_list
          .iter()
          .map(|pod| KubePods::from_api(pod))
          .collect::<Vec<_>>();
        let mut app = self.app.lock().await;
        app.data.pods.set_items(render_pods);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  pub async fn get_services(&self) {
    let svc: Api<Service> = self.get_namespaced_api().await;

    let lp = ListParams::default();
    match svc.list(&lp).await {
      Ok(svc_list) => {
        let render_services = svc_list
          .iter()
          .map(|service| KubeSvs::from_api(service))
          .collect::<Vec<_>>();
        let mut app = self.app.lock().await;
        app.data.services.set_items(render_services);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_namespaced_api<K: KubeResource>(&self) -> Api<K>
  where
    <K as KubeResource>::DynamicType: Default,
  {
    let app = self.app.lock().await;
    match &app.data.selected_ns {
      Some(ns) => Api::namespaced(self.client.clone(), &ns),
      None => Api::all(self.client.clone()),
    }
  }
}

// adapted from https://github.com/davidB/kubectl-view-allocations
// TODO move to use this directly from dependency if https://github.com/davidB/kubectl-view-allocations/issues/128 is resolved

async fn get_utilizations_metrics(client: Client, resources: &mut Vec<Resource>) -> Result<()> {
  let request = Request::new("/apis/metrics.k8s.io/v1beta1/pods");
  let pod_metrics: ObjectList<metrics::PodMetrics> = client
    .request(request.list(&ListParams::default())?)
    .await?;

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
      kind: cpu_kind.to_string(),
      qualifier: ResourceQualifier::Utilization,
      quantity: cpu_utilization,
      location: location.clone(),
    });
    resources.push(Resource {
      kind: memory_kind.to_string(),
      qualifier: ResourceQualifier::Utilization,
      quantity: memory_utilization,
      location: location.clone(),
    });
  }
  Ok(())
}

async fn get_pod_utilizations(pods: Api<Pod>, resources: &mut Vec<Resource>) -> Result<()> {
  let pod_list = pods.list(&ListParams::default()).await?;
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

async fn get_node_utilizations(client: Client, resources: &mut Vec<Resource>) -> Result<()> {
  let lp = ListParams::default();
  let nodes: Api<Node> = Api::all(client);

  let node_list = nodes.list(&lp).await?;
  for node in node_list.items {
    let location = Location {
      node_name: node.metadata.name,
      ..Location::default()
    };
    if let Some(als) = node.status.and_then(|v| v.allocatable) {
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
    kind: "pods".to_string(),
    qualifier: qualifier.clone(),
    quantity: Qty::from_str("1")?,
    location: location.clone(),
  });
  Ok(())
}

fn extract_locations(resources: &Vec<Resource>) -> HashMap<(String, String), Location> {
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

fn make_qualifiers(
  rsrcs: &[Resource],
  group_by: &[GroupBy],
) -> Vec<(Vec<String>, Option<QtyByQualifier>)> {
  let group_by_fct = group_by.iter().map(GroupBy::to_fct).collect::<Vec<_>>();
  let mut out = make_group_x_qualifier(&rsrcs.iter().collect::<Vec<_>>(), &[], &group_by_fct, 0);
  out.sort_by_key(|i| i.0.clone());
  out
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
      .filter_map(|e| group_by(e).map(|k| (k, *e)))
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

    if rsrcs.iter().all(|i| i.kind == kind) {
      let sum = rsrcs.iter().fold(QtyByQualifier::default(), |mut acc, v| {
        match &v.qualifier {
          ResourceQualifier::Limit => acc.limit = add(acc.limit, &v.quantity),
          ResourceQualifier::Requested => acc.requested = add(acc.requested, &v.quantity),
          ResourceQualifier::Allocatable => acc.allocatable = add(acc.allocatable, &v.quantity),
          ResourceQualifier::Utilization => acc.utilization = add(acc.utilization, &v.quantity),
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
