use super::super::app::{
  configmaps::KubeConfigMap,
  contexts,
  deployments::KubeDeployments,
  metrics::{self, Resource},
  nodes::{KubeNode, NodeMetrics},
  ns::KubeNs,
  pods::KubePod,
  replicasets::KubeReplicaSet,
  statefulsets::KubeStatefulSet,
  svcs::KubeSvc,
};
use super::Network;

use anyhow::anyhow;
use k8s_openapi::api::core::v1::{Namespace, Node, Pod};
use kube::{
  api::{DynamicObject, GroupVersionKind, ListParams, ObjectList, Request},
  config::Kubeconfig,
  Api, Resource as KubeResource,
};
use serde::de::DeserializeOwned;
use std::fmt;

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
    // custom request since metrics API doesnt exist on kube-rs
    let gvk = GroupVersionKind::gvk("metrics.k8s.io", "v1beta1", "nodemetrics").unwrap();
    let api: Api<DynamicObject> = Api::all_with(self.client.clone(), &gvk);
    match api.list(&ListParams::default()).await {
      Ok(metrics) => {
        let mut app = self.app.lock().await;

        let items = metrics
          .items
          .iter()
          .map(|metric| NodeMetrics::from_api(metric, &app))
          .collect();

        app.data.node_metrics = items;
      }
      Err(_) => {
        let mut app = self.app.lock().await;
        app.data.node_metrics = vec![];
      }
    };
  }

  pub async fn get_utilizations(&self) {
    let mut resources: Vec<Resource> = vec![];

    let api: Api<Node> = Api::all(self.client.clone());
    match api.list(&ListParams::default()).await {
      Ok(node_list) => {
        if let Err(e) = Resource::compute_node_utilizations(node_list, &mut resources).await {
          self.handle_error(anyhow!(e)).await;
        }
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    }

    let api: Api<Pod> = self.get_namespaced_api().await;
    match api.list(&ListParams::default()).await {
      Ok(pod_list) => {
        if let Err(e) = Resource::compute_pod_utilizations(pod_list, &mut resources).await {
          self.handle_error(anyhow!(e)).await;
        }
      }
      Err(e) => self.handle_error(anyhow!(e)).await,
    }

    // custom request since metrics API doesnt exist on kube-rs
    let request = Request::new("/apis/metrics.k8s.io/v1beta1/pods");
    if let Ok(pod_metrics) = self
      .client
      .clone()
      .request::<ObjectList<metrics::PodMetrics>>(request.list(&ListParams::default()).unwrap())
      .await
    {
      if let Err(_e) = Resource::compute_utilizations_metrics(pod_metrics, &mut resources).await {
        // don't do anything to avoid showing constant error when metric-server is not found,
        // since its not a mandatory component in a cluster
        // TODO may be show a non intrusive warning
      }
    };

    let mut app = self.app.lock().await;

    let data = Resource::make_qualifiers(&resources, &app.utilization_group_by);

    app.data.metrics.set_items(data);
  }

  pub async fn get_nodes(&self) {
    let lp = ListParams::default();
    let api_pods: Api<Pod> = Api::all(self.client.clone());
    let api_nodes: Api<Node> = Api::all(self.client.clone());

    match api_nodes.list(&lp).await {
      Ok(node_list) => {
        self.get_node_metrics().await;
        let pods_list = api_pods.list(&lp).await;

        let mut app = self.app.lock().await;

        let items = node_list
          .iter()
          .map(|node| KubeNode::from_api(node, &pods_list, &mut app))
          .collect::<Vec<_>>();

        app.data.nodes.set_items(items);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  pub async fn get_namespaces(&self) {
    let api: Api<Namespace> = Api::all(self.client.clone());

    let lp = ListParams::default();
    match api.list(&lp).await {
      Ok(ns_list) => {
        let items = ns_list
          .iter()
          .map(|ns| KubeNs::from_api(ns))
          .collect::<Vec<_>>();
        let mut app = self.app.lock().await;
        app.data.namespaces.set_items(items);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  pub async fn get_pods(&self) {
    let items: Vec<KubePod> = self
      .get_namespaced_resources(|it| KubePod::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    if app.data.selected.pod.is_some() {
      let containers = &items.iter().find_map(|pod| {
        if pod.name == app.data.selected.pod.clone().unwrap() {
          Some(&pod.containers)
        } else {
          None
        }
      });
      if containers.is_some() {
        app.data.containers.set_items(containers.unwrap().clone());
      }
    }
    app.data.pods.set_items(items);
  }

  pub async fn get_services(&self) {
    let items: Vec<KubeSvc> = self
      .get_namespaced_resources(|it| KubeSvc::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    app.data.services.set_items(items);
  }

  pub async fn get_config_maps(&self) {
    let items: Vec<KubeConfigMap> = self
      .get_namespaced_resources(|it| KubeConfigMap::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    app.data.config_maps.set_items(items);
  }

  pub async fn get_stateful_sets(&self) {
    let items: Vec<KubeStatefulSet> = self
      .get_namespaced_resources(|it| KubeStatefulSet::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    app.data.stateful_sets.set_items(items);
  }

  pub async fn get_replica_sets(&self) {
    let items: Vec<KubeReplicaSet> = self
      .get_namespaced_resources(|it| KubeReplicaSet::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    app.data.replica_sets.set_items(items);
  }

  pub async fn get_deployments(&self) {
    let items: Vec<KubeDeployments> = self
      .get_namespaced_resources(|it| KubeDeployments::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    app.data.deployments.set_items(items);
  }

  /// calls the kubernetes API to list the given resource for either selected namespace or all namespaces
  async fn get_namespaced_resources<K: KubeResource, T, F>(&self, map_fn: F) -> Vec<T>
  where
    <K as KubeResource>::DynamicType: Default,
    K: Clone + DeserializeOwned + fmt::Debug,
    F: FnMut(&K) -> T,
  {
    let api: Api<K> = self.get_namespaced_api().await;
    let lp = ListParams::default();
    match api.list(&lp).await {
      Ok(list) => list.iter().map(map_fn).collect::<Vec<_>>(),
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
        vec![]
      }
    }
  }

  async fn get_namespaced_api<K: KubeResource>(&self) -> Api<K>
  where
    <K as KubeResource>::DynamicType: Default,
  {
    let app = self.app.lock().await;
    match &app.data.selected.ns {
      Some(ns) => Api::namespaced(self.client.clone(), &ns),
      None => Api::all(self.client.clone()),
    }
  }
}
