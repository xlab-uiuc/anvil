// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
#![allow(unused_imports)]
use crate::external_api::spec::{EmptyAPI, EmptyTypeView};
use crate::kubernetes_api_objects::spec::{
    api_method::*, common::*, config_map::*, dynamic::*, owner_reference::*, prelude::*, resource::*,
    stateful_set::*,
};
use crate::kubernetes_cluster::spec::{
    api_server::state_machine::*,
    cluster::*,
    cluster_state_machine::Step,
    controller::types::{ControllerActionInput, ControllerStep},
    message::*,
};
use crate::reconciler::spec::reconciler::*;
use crate::temporal_logic::{defs::*, rules::*};
use crate::vstd_ext::{multiset_lib, seq_lib, string_view::*};
use crate::v_replica_set_controller::{
    model::reconciler::*,
    proof::{predicate::*},
    trusted::{spec_types::*, step::*, liveness_theorem::*},
};
use vstd::{multiset::*, prelude::*, string::*};

verus! {

pub open spec fn cluster_resources_is_finite() -> StatePred<VRSCluster> {
    |s: VRSCluster| s.resources().dom().finite()
} 

// The proof will probabily involve more changes elsewhere.
pub open spec fn vrs_replicas_bounded_above(
    vrs: VReplicaSetView
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        vrs.spec.replicas.unwrap_or(0) <= i32::MAX // As allowed by Kubernetes.
    }
}

pub open spec fn vrs_selector_matches_template_labels(
    vrs: VReplicaSetView
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let match_value = 
            if vrs.spec.template.is_none()
            || vrs.spec.template.unwrap().metadata.is_none()
            || vrs.spec.template.unwrap().metadata.unwrap().labels.is_none() {
                Map::empty()
            } else {
                vrs.spec.template.unwrap().metadata.unwrap().labels.unwrap()
            };
        vrs.spec.selector.matches(match_value)
    }
}

pub open spec fn every_create_request_is_well_formed() -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        forall |msg: VRSMessage| #![trigger msg.dst.is_ApiServer(), msg.content.is_APIRequest()] {
            let content = msg.content;
            let obj = content.get_create_request().obj;
            &&& s.in_flight().contains(msg)
            &&& msg.dst.is_ApiServer()
            &&& msg.content.is_APIRequest()
            &&& content.is_create_request()
        } ==> {
            let content = msg.content;
            let req = content.get_create_request();
            let obj = req.obj;
            let created_obj = DynamicObjectView {
                kind: req.obj.kind,
                metadata: ObjectMetaView {
                    // Set name for new object if name is not provided, here we generate
                    // a unique name. The uniqueness is guaranteed by generated_name_is_unique.
                    name: if req.obj.metadata.name.is_Some() {
                        req.obj.metadata.name
                    } else {
                        Some(generate_name(s.kubernetes_api_state))
                    },
                    namespace: Some(req.namespace), // Set namespace for new object
                    resource_version: Some(s.kubernetes_api_state.resource_version_counter), // Set rv for new object
                    uid: Some(s.kubernetes_api_state.uid_counter), // Set uid for new object
                    deletion_timestamp: None, // Unset deletion timestamp for new object
                    ..req.obj.metadata
                },
                spec: req.obj.spec,
                status: marshalled_default_status::<VReplicaSetView>(req.obj.kind), // Overwrite the status with the default one
            };
            &&& obj.metadata.deletion_timestamp.is_None()
            &&& content.get_create_request().namespace == obj.metadata.namespace.unwrap()
            &&& unmarshallable_object::<VReplicaSetView>(obj)
            &&& created_object_validity_check::<VReplicaSetView>(created_obj).is_none()
        }
    }
}

pub open spec fn no_pending_update_or_update_status_request_on_pods() -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        forall |msg: VRSMessage| {
            &&& s.in_flight().contains(msg)
            &&& #[trigger] msg.dst.is_ApiServer()
            &&& #[trigger] msg.content.is_APIRequest()
        } ==> {
            &&& msg.content.is_update_request() ==> msg.content.get_update_request().key().kind != PodView::kind()
            &&& msg.content.is_update_status_request() ==> msg.content.get_update_status_request().key().kind != PodView::kind()
        }
    }
}


pub open spec fn every_create_matching_pod_request_implies_at_after_create_pod_step(
    vrs: VReplicaSetView
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        forall |msg: VRSMessage| #![trigger msg.dst.is_ApiServer(), msg.content.is_APIRequest()] {
            let content = msg.content;
            let obj = content.get_create_request().obj;
            &&& s.in_flight().contains(msg)
            &&& msg.dst.is_ApiServer()
            &&& msg.content.is_APIRequest()
            &&& content.is_create_request()
            &&& owned_selector_match_is(vrs, obj)
        } ==> {
            &&& exists |diff: usize| #[trigger] at_vrs_step_with_vrs(vrs, VReplicaSetReconcileStep::AfterCreatePod(diff))(s)
            &&& VRSCluster::pending_req_msg_is(s, vrs.object_ref(), msg)
        }
    }
}

pub open spec fn every_delete_matching_pod_request_implies_at_after_delete_pod_step(
    vrs: VReplicaSetView
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        forall |msg: VRSMessage| #![trigger msg.dst.is_ApiServer(), msg.content.is_APIRequest()] {
            let content = msg.content;
            let key = content.get_delete_request().key;
            let obj = s.resources()[key];
            &&& s.in_flight().contains(msg)
            &&& msg.dst.is_ApiServer()
            &&& msg.content.is_APIRequest()
            &&& content.is_delete_request()
            &&& s.resources().contains_key(key)
            &&& owned_selector_match_is(vrs, obj)
        } ==> {
            &&& exists |diff: usize| #[trigger] at_vrs_step_with_vrs(vrs, VReplicaSetReconcileStep::AfterDeletePod(diff))(s)
            &&& VRSCluster::pending_req_msg_is(s, vrs.object_ref(), msg)
        }
    }
}

}
