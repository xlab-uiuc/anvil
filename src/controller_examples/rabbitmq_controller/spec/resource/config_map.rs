// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
#![allow(unused_imports)]
use super::common::*;
use crate::external_api::spec::*;
use crate::kubernetes_api_objects::{
    container::*, label_selector::*, pod_template_spec::*, prelude::*, resource_requirements::*,
    volume::*,
};
use crate::kubernetes_cluster::spec::message::*;
use crate::rabbitmq_controller::common::*;
use crate::rabbitmq_controller::spec::resource::ServiceAccountBuilder;
use crate::rabbitmq_controller::spec::types::*;
use crate::reconciler::spec::{io::*, reconciler::*, resource_builder::*};
use crate::state_machine::{action::*, state_machine::*};
use crate::temporal_logic::defs::*;
use crate::vstd_ext::{map_lib::*, string_view::*};
use vstd::prelude::*;
use vstd::string::*;

verus! {

pub struct ServerConfigMapBuilder {}

impl ResourceBuilder<RabbitmqClusterView, RabbitmqReconcileState> for ServerConfigMapBuilder {
    open spec fn get_request(rabbitmq: RabbitmqClusterView) -> GetRequest {
        GetRequest { key: make_server_config_map_key(rabbitmq) }
    }

    open spec fn make(rabbitmq: RabbitmqClusterView, state: RabbitmqReconcileState) -> Result<DynamicObjectView, ()> {
        Ok(make_server_config_map(rabbitmq).marshal())
    }

    open spec fn update(rabbitmq: RabbitmqClusterView, state: RabbitmqReconcileState, obj: DynamicObjectView) -> Result<DynamicObjectView, ()> {
        let cm = ConfigMapView::unmarshal(obj);
        if cm.is_ok() {
            Ok(update_server_config_map(rabbitmq, cm.get_Ok_0()).marshal())
        } else {
            Err(())
        }
    }

    open spec fn state_after_create_or_update(obj: DynamicObjectView, state: RabbitmqReconcileState) -> (res: Result<RabbitmqReconcileState, ()>) {
        let cm = ConfigMapView::unmarshal(obj);
        if cm.is_ok() && cm.get_Ok_0().metadata.resource_version.is_Some() {
            Ok(RabbitmqReconcileState {
                latest_config_map_rv_opt: Some(int_to_string_view(cm.get_Ok_0().metadata.resource_version.get_Some_0())),
                ..state
            })
        } else {
            Err(())
        }
    }

    open spec fn resource_state_matches(rabbitmq: RabbitmqClusterView, resources: StoredState) -> bool {
        let key = make_server_config_map_key(rabbitmq);
        let obj = resources[key];
        &&& resources.contains_key(key)
        &&& ConfigMapView::unmarshal(obj).is_Ok()
        &&& ConfigMapView::unmarshal(obj).get_Ok_0().data == make_server_config_map(rabbitmq).data
        &&& obj.spec == ConfigMapView::marshal_spec((make_server_config_map(rabbitmq).data, ()))
        &&& obj.metadata.labels == make_server_config_map(rabbitmq).metadata.labels
        &&& obj.metadata.annotations == make_server_config_map(rabbitmq).metadata.annotations
    }

    open spec fn unchangeable(object: DynamicObjectView, rabbitmq: RabbitmqClusterView) -> bool {
        true
    }
}

pub open spec fn update_server_config_map(rabbitmq: RabbitmqClusterView, found_config_map: ConfigMapView) -> ConfigMapView {
    ConfigMapView {
        metadata: ObjectMetaView {
            owner_references: Some(make_owner_references(rabbitmq)),
            finalizers: None,
            labels: make_server_config_map(rabbitmq).metadata.labels,
            annotations: make_server_config_map(rabbitmq).metadata.annotations,
            ..found_config_map.metadata
        },
        data: make_server_config_map(rabbitmq).data,
        ..found_config_map
    }
}

pub open spec fn make_server_config_map_name(rabbitmq: RabbitmqClusterView) -> StringView
    recommends
        rabbitmq.metadata.name.is_Some(),
{
    rabbitmq.metadata.name.get_Some_0() + new_strlit("-server-conf")@
}

pub open spec fn make_server_config_map_key(rabbitmq: RabbitmqClusterView) -> ObjectRef
    recommends
        rabbitmq.metadata.name.is_Some(),
        rabbitmq.metadata.namespace.is_Some(),
{
    ObjectRef {
        kind: ConfigMapView::kind(),
        name: make_server_config_map_name(rabbitmq),
        namespace: rabbitmq.metadata.namespace.get_Some_0(),
    }
}

pub open spec fn make_server_config_map(rabbitmq: RabbitmqClusterView) -> ConfigMapView
    recommends
        rabbitmq.metadata.name.is_Some(),
        rabbitmq.metadata.namespace.is_Some(),
{
    ConfigMapView {
        metadata: ObjectMetaView {
            name: Some(make_server_config_map_name(rabbitmq)),
            namespace: rabbitmq.metadata.namespace,
            owner_references: Some(make_owner_references(rabbitmq)),
            labels: Some(make_labels(rabbitmq)),
            annotations: Some(rabbitmq.spec.annotations),
            ..ObjectMetaView::default()
        },
        data: Some({
            let data = Map::empty()
                        .insert(new_strlit("operatorDefaults.conf")@, default_rbmq_config(rabbitmq))
                        .insert(new_strlit("userDefinedConfiguration.conf")@,
                        {
                            if rabbitmq.spec.rabbitmq_config.is_Some()
                            && rabbitmq.spec.rabbitmq_config.get_Some_0().additional_config.is_Some()
                            {   // check if there are rabbitmq-related customized configurations
                                new_strlit("total_memory_available_override_value = 1717986919\n")@ + rabbitmq.spec.rabbitmq_config.get_Some_0().additional_config.get_Some_0()
                            } else {
                                new_strlit("total_memory_available_override_value = 1717986919\n")@
                            }
                        });
            if rabbitmq.spec.rabbitmq_config.is_Some() && rabbitmq.spec.rabbitmq_config.get_Some_0().advanced_config.is_Some()
            && rabbitmq.spec.rabbitmq_config.get_Some_0().advanced_config.get_Some_0() != new_strlit("")@
            && rabbitmq.spec.rabbitmq_config.get_Some_0().env_config.is_Some()
            && rabbitmq.spec.rabbitmq_config.get_Some_0().env_config.get_Some_0() != new_strlit("")@ {
                data.insert(new_strlit("advanced.config")@, rabbitmq.spec.rabbitmq_config.get_Some_0().advanced_config.get_Some_0())
                    .insert(new_strlit("rabbitmq-env.conf")@, rabbitmq.spec.rabbitmq_config.get_Some_0().env_config.get_Some_0())
            } else if rabbitmq.spec.rabbitmq_config.is_Some() && rabbitmq.spec.rabbitmq_config.get_Some_0().advanced_config.is_Some()
            && rabbitmq.spec.rabbitmq_config.get_Some_0().advanced_config.get_Some_0() != new_strlit("")@ {
                data.insert(new_strlit("advanced.config")@, rabbitmq.spec.rabbitmq_config.get_Some_0().advanced_config.get_Some_0())
            } else if rabbitmq.spec.rabbitmq_config.is_Some() && rabbitmq.spec.rabbitmq_config.get_Some_0().env_config.is_Some()
            && rabbitmq.spec.rabbitmq_config.get_Some_0().env_config.get_Some_0() != new_strlit("")@ {
                data.insert(new_strlit("rabbitmq-env.conf")@, rabbitmq.spec.rabbitmq_config.get_Some_0().env_config.get_Some_0())
            } else {
                data
            }
        }),
        ..ConfigMapView::default()
    }
}

pub open spec fn default_rbmq_config(rabbitmq: RabbitmqClusterView) -> StringView
    recommends
        rabbitmq.metadata.name.is_Some(),
        rabbitmq.metadata.namespace.is_Some(),
{
    let name = rabbitmq.metadata.name.get_Some_0();

    new_strlit(
        "queue_master_locator = min-masters\n\
        disk_free_limit.absolute = 2GB\n\
        cluster_partition_handling = pause_minority\n\
        cluster_formation.peer_discovery_backend = rabbit_peer_discovery_k8s\n\
        cluster_formation.k8s.host = kubernetes.default\n\
        cluster_formation.k8s.address_type = hostname\n"
    )@ + new_strlit("cluster_formation.target_cluster_size_hint = ")@ + int_to_string_view(rabbitmq.spec.replicas) + new_strlit("\n")@
    + new_strlit("cluster_name = ")@ + name + new_strlit("\n")@
}

}