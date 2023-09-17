use async_trait::async_trait;
use k8s_openapi::{
  api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding},
  chrono::Utc,
};
use tui::{
  backend::Backend,
  layout::{Constraint, Rect},
  widgets::{Cell, Row},
  Frame,
};

use super::{
  models::{AppResource, KubeResource},
  utils::{self},
  ActiveBlock, App,
};
use crate::{
  draw_resource_tab,
  network::Network,
  ui::utils::{
    draw_describe_block, draw_resource_block, get_describe_active, get_resource_title,
    style_primary, title_with_dual_style, ResourceTableProps, COPY_HINT,
    DESCRIBE_YAML_AND_ESC_HINT,
  },
};

#[derive(Clone, Debug, PartialEq)]
pub struct KubeRole {
  pub namespace: String,
  pub name: String,
  pub age: String,
  k8s_obj: Role,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KubeRoleBinding {
  pub namespace: String,
  pub name: String,
  pub role: String,
  pub age: String,
  k8s_obj: RoleBinding,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KubeClusterRole {
  pub name: String,
  pub age: String,
  k8s_obj: ClusterRole,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KubeClusterRoleBinding {
  pub name: String,
  pub role: String,
  pub age: String,
  k8s_obj: ClusterRoleBinding,
}

impl From<Role> for KubeRole {
  fn from(role: Role) -> Self {
    KubeRole {
      namespace: role.metadata.namespace.clone().unwrap_or_default(),
      name: role.metadata.name.clone().unwrap_or_default(),
      age: utils::to_age(role.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: utils::sanitize_obj(role),
    }
  }
}

impl KubeResource<Role> for KubeRole {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &Role {
    &self.k8s_obj
  }
}

static ROLES_TITLE: &str = "Roles";

pub struct RoleResource {}

#[async_trait]
impl AppResource for RoleResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      ROLES_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_roles_block,
      app.data.roles
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeRole> = nw.get_namespaced_resources(Role::into).await;

    let mut app = nw.app.lock().await;
    app.data.roles.set_items(items);
  }
}

fn draw_roles_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(app, ROLES_TITLE, "", app.data.roles.items.len());

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.roles,
      table_headers: vec!["Namespace", "Name", "Age"],
      column_widths: vec![
        Constraint::Percentage(40),
        Constraint::Percentage(40),
        Constraint::Percentage(20),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary(app.light_theme))
    },
    app.light_theme,
    app.is_loading,
    app.data.selected.filter.to_owned(),
  );
}

impl From<ClusterRole> for KubeClusterRole {
  fn from(cluster_role: ClusterRole) -> Self {
    KubeClusterRole {
      name: cluster_role.metadata.name.clone().unwrap_or_default(),
      age: utils::to_age(
        cluster_role.metadata.creation_timestamp.as_ref(),
        Utc::now(),
      ),
      k8s_obj: utils::sanitize_obj(cluster_role),
    }
  }
}

impl KubeResource<ClusterRole> for KubeClusterRole {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &ClusterRole {
    &self.k8s_obj
  }
}

static CLUSTER_ROLES_TITLE: &str = "ClusterRoles";

pub struct ClusterRoleResource {}

#[async_trait]
impl AppResource for ClusterRoleResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      CLUSTER_ROLES_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_cluster_roles_block,
      app.data.cluster_roles
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeClusterRole> = nw.get_resources(ClusterRole::into).await;

    let mut app = nw.app.lock().await;
    app.data.cluster_roles.set_items(items);
  }
}

fn draw_cluster_roles_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(
    app,
    CLUSTER_ROLES_TITLE,
    "",
    app.data.cluster_roles.items.len(),
  );

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.cluster_roles,
      table_headers: vec!["Name", "Age"],
      column_widths: vec![Constraint::Percentage(50), Constraint::Percentage(50)],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.name.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary(app.light_theme))
    },
    app.light_theme,
    app.is_loading,
    app.data.selected.filter.to_owned(),
  );
}

impl From<RoleBinding> for KubeRoleBinding {
  fn from(role_binding: RoleBinding) -> Self {
    KubeRoleBinding {
      namespace: role_binding.metadata.namespace.clone().unwrap_or_default(),
      name: role_binding.metadata.name.clone().unwrap_or_default(),
      role: role_binding.role_ref.name.clone(),
      age: utils::to_age(
        role_binding.metadata.creation_timestamp.as_ref(),
        Utc::now(),
      ),
      k8s_obj: utils::sanitize_obj(role_binding),
    }
  }
}
impl KubeResource<RoleBinding> for KubeRoleBinding {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &RoleBinding {
    &self.k8s_obj
  }
}

static ROLE_BINDINGS_TITLE: &str = "RoleBindings";

pub struct RoleBindingResource {}

#[async_trait]
impl AppResource for RoleBindingResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      ROLE_BINDINGS_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_role_bindings_block,
      app.data.role_bindings
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeRoleBinding> = nw.get_namespaced_resources(RoleBinding::into).await;

    let mut app = nw.app.lock().await;
    app.data.role_bindings.set_items(items);
  }
}

fn draw_role_bindings_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(
    app,
    ROLE_BINDINGS_TITLE,
    "",
    app.data.role_bindings.items.len(),
  );

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.role_bindings,
      table_headers: vec!["Namespace", "Name", "Role", "Age"],
      column_widths: vec![
        Constraint::Percentage(20),
        Constraint::Percentage(30),
        Constraint::Percentage(30),
        Constraint::Percentage(20),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.namespace.to_owned()),
        Cell::from(c.name.to_owned()),
        Cell::from(c.role.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary(app.light_theme))
    },
    app.light_theme,
    app.is_loading,
    app.data.selected.filter.to_owned(),
  );
}

impl From<ClusterRoleBinding> for KubeClusterRoleBinding {
  fn from(crb: ClusterRoleBinding) -> Self {
    KubeClusterRoleBinding {
      name: crb.metadata.name.clone().unwrap_or_default(),
      role: format!("{}/{}", crb.role_ref.kind, crb.role_ref.name),
      age: utils::to_age(crb.metadata.creation_timestamp.as_ref(), Utc::now()),
      k8s_obj: utils::sanitize_obj(crb),
    }
  }
}

impl KubeResource<ClusterRoleBinding> for KubeClusterRoleBinding {
  fn get_name(&self) -> &String {
    &self.name
  }
  fn get_k8s_obj(&self) -> &ClusterRoleBinding {
    &self.k8s_obj
  }
}

static CLUSTER_ROLES_BINDING_TITLE: &str = "ClusterRoleBinding";

pub struct ClusterRoleBindingResource {}

#[async_trait]
impl AppResource for ClusterRoleBindingResource {
  fn render<B: Backend>(block: ActiveBlock, f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
    draw_resource_tab!(
      CLUSTER_ROLES_BINDING_TITLE,
      block,
      f,
      app,
      area,
      Self::render,
      draw_cluster_role_binding_block,
      app.data.cluster_role_bindings
    );
  }

  async fn get_resource(nw: &Network<'_>) {
    let items: Vec<KubeClusterRoleBinding> = nw.get_resources(ClusterRoleBinding::into).await;

    let mut app = nw.app.lock().await;
    app.data.cluster_role_bindings.set_items(items);
  }
}

fn draw_cluster_role_binding_block<B: Backend>(f: &mut Frame<'_, B>, app: &mut App, area: Rect) {
  let title = get_resource_title(
    app,
    CLUSTER_ROLES_BINDING_TITLE,
    "",
    app.data.cluster_role_bindings.items.len(),
  );

  draw_resource_block(
    f,
    area,
    ResourceTableProps {
      title,
      inline_help: DESCRIBE_YAML_AND_ESC_HINT.into(),
      resource: &mut app.data.cluster_role_bindings,
      table_headers: vec!["Name", "Role", "Age"],
      column_widths: vec![
        Constraint::Percentage(40),
        Constraint::Percentage(40),
        Constraint::Percentage(20),
      ],
    },
    |c| {
      Row::new(vec![
        Cell::from(c.name.to_owned()),
        Cell::from(c.role.to_owned()),
        Cell::from(c.age.to_owned()),
      ])
      .style(style_primary(app.light_theme))
    },
    app.light_theme,
    app.is_loading,
    app.data.selected.filter.to_owned(),
  );
}

#[cfg(test)]
mod tests {
  use k8s_openapi::chrono::Utc;

  use crate::app::{
    roles::{KubeClusterRole, KubeClusterRoleBinding, KubeRole, KubeRoleBinding},
    test_utils::{convert_resource_from_file, get_time},
    utils,
  };

  #[test]
  fn test_roles_binding_from_rbac_api() {
    let (roles, roles_list): (Vec<KubeRole>, Vec<_>) = convert_resource_from_file("roles");

    assert_eq!(roles.len(), 1);
    assert_eq!(
      roles[0],
      KubeRole {
        namespace: "default".to_string(),
        name: "kiali-viewer".into(),
        age: utils::to_age(Some(&get_time("2022-06-27T16:33:06Z")), Utc::now()),
        k8s_obj: roles_list[0].clone(),
      }
    )
  }

  #[test]
  fn test_cluster_roles_from_rbac_api() {
    let (cluster_roles, cluster_roles_list): (Vec<KubeClusterRole>, Vec<_>) =
      convert_resource_from_file("clusterroles");

    assert_eq!(cluster_roles.len(), 1);
    assert_eq!(
      cluster_roles[0],
      KubeClusterRole {
        name: "admin".into(),
        age: utils::to_age(Some(&get_time("2021-12-14T11:04:22Z")), Utc::now()),
        k8s_obj: cluster_roles_list[0].clone(),
      }
    )
  }

  #[test]
  fn test_role_binding_from_rbac_api() {
    let (role_bindings, rolebindings_list): (Vec<KubeRoleBinding>, Vec<_>) =
      convert_resource_from_file("role_bindings");

    assert_eq!(role_bindings.len(), 1);
    assert_eq!(
      role_bindings[0],
      KubeRoleBinding {
        namespace: "default".to_string(),
        name: "kiali".into(),
        role: "kiali-viewer".into(),
        age: utils::to_age(Some(&get_time("2022-06-27T16:33:07Z")), Utc::now()),
        k8s_obj: rolebindings_list[0].clone(),
      }
    )
  }

  #[test]
  fn test_cluster_role_bindings_from_rbac_api() {
    let (cluster_role_binding, cluster_role_bindings_list): (Vec<KubeClusterRoleBinding>, Vec<_>) =
      convert_resource_from_file("clusterrole_binding");

    assert_eq!(cluster_role_binding.len(), 2);
    assert_eq!(
      cluster_role_binding[0],
      KubeClusterRoleBinding {
        name: "admin-user".into(),
        role: "ClusterRole/cluster-admin".into(),
        age: utils::to_age(Some(&get_time("2022-03-02T16:50:53Z")), Utc::now()),
        k8s_obj: cluster_role_bindings_list[0].clone(),
      }
    )
  }
}
