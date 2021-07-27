use std::fmt;

use anyhow::anyhow;
use k8s_openapi::api::core::v1::{Namespace, Node, Pod};
use kube::{
  api::{ListMeta, ListParams, ObjectList},
  config::Kubeconfig,
  Api, Resource as ApiResource,
};
use kubectl_view_allocations::{
  extract_allocatable_from_nodes, extract_allocatable_from_pods,
  extract_utilizations_from_pod_metrics, make_qualifiers, metrics::PodMetrics, Resource,
};
use serde::de::DeserializeOwned;

use super::Network;
use crate::app::{
  configmaps::KubeConfigMap,
  contexts,
  cronjobs::KubeCronJob,
  daemonsets::KubeDaemonSet,
  deployments::KubeDeployment,
  jobs::KubeJob,
  metrics::{self, KubeNodeMetrics},
  models::KubeResource,
  nodes::KubeNode,
  ns::KubeNs,
  pods::KubePod,
  replicasets::KubeReplicaSet,
  replication_controllers::KubeReplicationController,
  secrets::KubeSecret,
  statefulsets::KubeStatefulSet,
  svcs::KubeSvc,
};

impl<'a> Network<'a> {
  pub async fn get_kube_config(&self) {
    match Kubeconfig::read() {
      Ok(config) => {
        let mut app = self.app.lock().await;
        let selected_ctx = app.data.selected.context.to_owned();
        app.set_contexts(contexts::get_contexts(&config, selected_ctx));
        app.data.kubeconfig = Some(config);
      }
      Err(e) => {
        self
          .handle_error(anyhow!("Failed to load Kubernetes config. {:?}", e))
          .await;
      }
    }
  }

  pub async fn get_node_metrics(&self) {
    let api_node_metrics: Api<metrics::NodeMetrics> = Api::all(self.client.clone());

    match api_node_metrics.list(&ListParams::default()).await {
      Ok(node_metrics) => {
        let mut app = self.app.lock().await;

        let items = node_metrics
          .iter()
          .map(|metric| KubeNodeMetrics::from_api(metric, &app))
          .collect();

        app.data.node_metrics = items;
      }
      Err(_) => {
        let mut app = self.app.lock().await;
        app.data.node_metrics = vec![];
        // lets not show error as it will always be showing up and be annoying
        // TODO may be show once and then disable polling
      }
    };
  }

  pub async fn get_utilizations(&self) {
    let mut resources: Vec<Resource> = vec![];

    let api: Api<Node> = Api::all(self.client.clone());
    match api.list(&ListParams::default()).await {
      Ok(node_list) => {
        if let Err(e) = extract_allocatable_from_nodes(node_list, &mut resources).await {
          self
            .handle_error(anyhow!(
              "Failed to extract node allocation metrics. {:?}",
              e
            ))
            .await;
        }
      }
      Err(e) => {
        self
          .handle_error(anyhow!(
            "Failed to extract node allocation metrics. {:?}",
            e
          ))
          .await
      }
    }

    let api: Api<Pod> = self.get_namespaced_api().await;
    match api.list(&ListParams::default()).await {
      Ok(pod_list) => {
        if let Err(e) = extract_allocatable_from_pods(pod_list, &mut resources).await {
          self
            .handle_error(anyhow!("Failed to extract pod allocation metrics. {:?}", e))
            .await;
        }
      }
      Err(e) => {
        self
          .handle_error(anyhow!("Failed to extract pod allocation metrics. {:?}", e))
          .await
      }
    }

    let api_pod_metrics: Api<PodMetrics> = Api::all(self.client.clone());

    match api_pod_metrics
    .list(&ListParams::default())
    .await
    {
      Ok(pod_metrics) => {
        if let Err(e) = extract_utilizations_from_pod_metrics(pod_metrics, &mut resources).await {
          self.handle_error(anyhow!("Failed to extract pod utilization metrics. {:?}", e)).await;
        }
      }
      Err(_e) => self.handle_error(anyhow!("Failed to extract pod utilization metrics. Make sure you have a metrics-server deployed on your cluster.")).await,
    };

    let mut app = self.app.lock().await;

    let data = make_qualifiers(&resources, &app.utilization_group_by, &[]);

    app.data.metrics.set_items(data);
  }

  pub async fn get_nodes(&self) {
    let lp = ListParams::default();
    let api_pods: Api<Pod> = Api::all(self.client.clone());
    let api_nodes: Api<Node> = Api::all(self.client.clone());

    match api_nodes.list(&lp).await {
      Ok(node_list) => {
        self.get_node_metrics().await;

        let pods_list = match api_pods.list(&lp).await {
          Ok(list) => list,
          Err(_) => ObjectList {
            metadata: ListMeta::default(),
            items: vec![],
          },
        };

        let mut app = self.app.lock().await;

        let items = node_list
          .iter()
          .map(|node| KubeNode::from_api_with_pods(node, &pods_list, &mut app))
          .collect::<Vec<_>>();

        app.data.nodes.set_items(items);
      }
      Err(e) => {
        self
          .handle_error(anyhow!("Failed to get nodes. {:?}", e))
          .await;
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
        self
          .handle_error(anyhow!("Failed to get namespaces. {:?}", e))
          .await;
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

  pub async fn get_jobs(&self) {
    let items: Vec<KubeJob> = self
      .get_namespaced_resources(|it| KubeJob::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    app.data.jobs.set_items(items);
  }

  pub async fn get_cron_jobs(&self) {
    let items: Vec<KubeCronJob> = self
      .get_namespaced_resources(|it| KubeCronJob::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    app.data.cronjobs.set_items(items);
  }

  pub async fn get_secrets(&self) {
    let items: Vec<KubeSecret> = self
      .get_namespaced_resources(|it| KubeSecret::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    app.data.secrets.set_items(items);
  }

  pub async fn get_replication_controllers(&self) {
    let items: Vec<KubeReplicationController> = self
      .get_namespaced_resources(|it| KubeReplicationController::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    app.data.rpl_ctrls.set_items(items);
  }

  pub async fn get_deployments(&self) {
    let items: Vec<KubeDeployment> = self
      .get_namespaced_resources(|it| KubeDeployment::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    app.data.deployments.set_items(items);
  }

  pub async fn get_daemon_sets_jobs(&self) {
    let items: Vec<KubeDaemonSet> = self
      .get_namespaced_resources(|it| KubeDaemonSet::from_api(it))
      .await;

    let mut app = self.app.lock().await;
    app.data.daemon_sets.set_items(items);
  }

  /// calls the kubernetes API to list the given resource for either selected namespace or all namespaces
  async fn get_namespaced_resources<K: ApiResource, T, F>(&self, map_fn: F) -> Vec<T>
  where
    <K as ApiResource>::DynamicType: Default,
    K: Clone + DeserializeOwned + fmt::Debug,
    F: FnMut(&K) -> T,
  {
    let api: Api<K> = self.get_namespaced_api().await;
    let lp = ListParams::default();
    match api.list(&lp).await {
      Ok(list) => list.iter().map(map_fn).collect::<Vec<_>>(),
      Err(e) => {
        self
          .handle_error(anyhow!(
            "Failed to get resource {}. {:?}",
            std::any::type_name::<T>(),
            e
          ))
          .await;
        vec![]
      }
    }
  }

  async fn get_namespaced_api<K: ApiResource>(&self) -> Api<K>
  where
    <K as ApiResource>::DynamicType: Default,
  {
    let app = self.app.lock().await;
    match &app.data.selected.ns {
      Some(ns) => Api::namespaced(self.client.clone(), &ns),
      None => Api::all(self.client.clone()),
    }
  }
}
