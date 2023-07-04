use anyhow::anyhow;
use kube::{
  core::DynamicObject,
  discovery::{verbs, Scope},
  Api, Discovery,
};

use crate::app::{
  dynamic::{KubeDynamicGroup, KubeDynamicResource},
  models::StatefulList,
  ActiveBlock,
};

use super::Network;

impl<'a> Network<'a> {
  /// Discover and cache custom resources on the cluster
  pub async fn discover_dynamic_resources(&self) {
    let discovery = match Discovery::new(self.client.clone()).run().await {
      Ok(d) => d,
      Err(e) => {
        self
          .handle_error(anyhow!("Failed to get dynamic resources. {:?}", e))
          .await;
        return;
      }
    };

    let mut dynamic_resources = vec![];
    let mut dynamic_menu = vec![];

    let excluded = vec![
      "Namespace",
      "Pod",
      "Service",
      "Node",
      "ConfigMap",
      "StatefulSet",
      "ReplicaSet",
      "Deployment",
      "Job",
      "DaemonSet",
      "CronJob",
      "Secret",
      "ReplicationController",
      "PersistentVolumeClaim",
      "PersistentVolume",
      "StorageClass",
      "Role",
      "RoleBinding",
      "ClusterRole",
      "ClusterRoleBinding",
      "ServiceAccount",
      "Ingress",
    ];

    for group in discovery.groups() {
      for (ar, caps) in group.recommended_resources() {
        if !caps.supports_operation(verbs::LIST) || excluded.contains(&ar.kind.as_str()) {
          continue;
        }

        dynamic_menu.push((ar.kind.to_string(), ActiveBlock::DynamicResource));
        dynamic_resources.push(KubeDynamicGroup::new(ar, caps.scope));
      }
    }
    let mut app = self.app.lock().await;
    app.dynamic_resources_menu = StatefulList::with_items(dynamic_menu);
    app.data.dynamic_resources = dynamic_resources.clone();
  }

  /// fetch entries for a custom resource from the cluster
  pub async fn get_dynamic_resources(&self) {
    let mut app = self.app.lock().await;

    if let Some(drs) = &app.data.dynamic_resource_selected {
      let api: Api<DynamicObject> = if drs.scope == Scope::Cluster {
        Api::all_with(self.client.clone(), &drs.api_resource)
      } else {
        match &app.data.selected.ns {
          Some(ns) => Api::namespaced_with(self.client.clone(), ns, &drs.api_resource),
          None => Api::all_with(self.client.clone(), &drs.api_resource),
        }
      };

      let items = match api.list(&Default::default()).await {
        Ok(list) => list
          .items
          .iter()
          .map(|item| KubeDynamicResource::from(item.clone()))
          .collect::<Vec<KubeDynamicResource>>(),
        Err(e) => {
          self
            .handle_error(anyhow!("Failed to get dynamic resources. {:?}", e))
            .await;
          return;
        }
      };
      app.data.dynamic_resource_items.set_items(items);
    }
  }
}
