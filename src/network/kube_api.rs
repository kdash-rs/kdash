use super::super::app::{
  contexts,
  nodes::{KubeNode, NodeMetrics},
  ns::KubeNs,
  pods::KubePods,
  svcs::KubeSvs,
};
use super::Network;

use anyhow::anyhow;
use k8s_openapi::api::core::v1::{Namespace, Node, Pod, Service};
use kube::{
  api::{DynamicObject, GroupVersionKind, ListParams},
  config::Kubeconfig,
  Api, Resource,
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

  pub async fn get_top_node(&self) {
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

  pub async fn get_nodes(&self) {
    let lp = ListParams::default();
    let pods: Api<Pod> = Api::all(self.client.clone());
    let nodes: Api<Node> = Api::all(self.client.clone());

    match nodes.list(&lp).await {
      Ok(node_list) => {
        self.get_top_node().await;
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

  async fn get_namespaced_api<K: Resource>(&self) -> Api<K>
  where
    <K as Resource>::DynamicType: Default,
  {
    let app = self.app.lock().await;
    match &app.data.selected_ns {
      Some(ns) => Api::namespaced(self.client.clone(), &ns),
      None => Api::all(self.client.clone()),
    }
  }
}
