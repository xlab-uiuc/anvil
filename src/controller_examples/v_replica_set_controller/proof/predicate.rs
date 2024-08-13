// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
#![allow(unused_imports)]
use crate::kubernetes_api_objects::spec::{
    api_method::*, common::*, prelude::*, resource::*, stateful_set::*,
};
use crate::kubernetes_cluster::proof::controller_runtime::*;
use crate::kubernetes_cluster::spec::{
    cluster::*,
    cluster_state_machine::Step,
    controller::types::{ControllerActionInput, ControllerStep},
    message::*,
};
use crate::temporal_logic::defs::*;
use crate::v_replica_set_controller::model::{reconciler::*};
use crate::v_replica_set_controller::trusted::{
    liveness_theorem::*, spec_types::*, step::*,
};
use vstd::math::abs;
use vstd::prelude::*;

verus! {

// Predicates for reasoning about model states
pub open spec fn at_step_closure(step: VReplicaSetReconcileStep) -> spec_fn(VReplicaSetReconcileState) -> bool {
    |s: VReplicaSetReconcileState| s.reconcile_step == step
}

pub open spec fn at_vrs_step_with_vrs(vrs: VReplicaSetView, step: VReplicaSetReconcileStep) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        &&& s.ongoing_reconciles().contains_key(vrs.object_ref())
        &&& s.ongoing_reconciles()[vrs.object_ref()].triggering_cr.object_ref() == vrs.object_ref()
        &&& s.ongoing_reconciles()[vrs.object_ref()].triggering_cr.spec() == vrs.spec()
        &&& s.ongoing_reconciles()[vrs.object_ref()].triggering_cr.metadata().uid == vrs.metadata().uid
        &&& s.ongoing_reconciles()[vrs.object_ref()].local_state.reconcile_step == step
    }
}

// Predicates for reasoning about pods
pub open spec fn matching_pods(vrs: VReplicaSetView, resources: StoredState) -> Set<ObjectRef> {
    Set::new(|k: ObjectRef| owned_selector_match_is(vrs, resources, k))
}

pub open spec fn num_diff_pods_is(vrs: VReplicaSetView, diff: int) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let pods = matching_pods(vrs, s.resources());
        &&& pods.finite() 
        &&& pods.len() - vrs.spec.replicas.unwrap_or(0) == diff
    }
}

// Predicates to specify leads-to boundary lemmas.
pub open spec fn pending_req_in_flight_at_after_list_pods_step(
    vrs: VReplicaSetView
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let step = VReplicaSetReconcileStep::AfterListPods;
        let msg = s.ongoing_reconciles()[vrs.object_ref()].pending_req_msg.get_Some_0();
        let request = msg.content.get_APIRequest_0();
        &&& at_vrs_step_with_vrs(vrs, step)(s)
        &&& VRSCluster::has_pending_k8s_api_req_msg(s, vrs.object_ref())
        &&& s.in_flight().contains(msg)
        &&& msg.src == HostId::CustomController
        &&& msg.dst == HostId::ApiServer
        &&& msg.content.is_APIRequest()
        &&& request.is_ListRequest()
        &&& request.get_ListRequest_0() == ListRequest {
            kind: PodView::kind(),
            namespace: vrs.metadata.namespace.unwrap(),
        }
    }
}

pub open spec fn exists_resp_in_flight_at_after_list_pods_step(
    vrs: VReplicaSetView,
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let step = VReplicaSetReconcileStep::AfterListPods;
        let msg = s.ongoing_reconciles()[vrs.object_ref()].pending_req_msg.get_Some_0();
        let request = msg.content.get_APIRequest_0();
        &&& at_vrs_step_with_vrs(vrs, step)(s)
        &&& VRSCluster::has_pending_k8s_api_req_msg(s, vrs.object_ref())
        &&& s.in_flight().contains(msg)
        &&& msg.src == HostId::CustomController
        &&& msg.dst == HostId::ApiServer
        &&& msg.content.is_APIRequest()
        &&& request.is_ListRequest()
        &&& request.get_ListRequest_0() == ListRequest {
            kind: PodView::kind(),
            namespace: vrs.metadata.namespace.unwrap(),
        }
        &&& exists |resp_msg| {
            &&& #[trigger] s.in_flight().contains(resp_msg)
            &&& Message::resp_msg_matches_req_msg(resp_msg, msg)
            &&& resp_msg.content.get_list_response().res.is_Ok()
            &&& {
                let resp_objs = resp_msg.content.get_list_response().res.unwrap();
                // The response must give back all the pods in the replicaset's namespace.
                resp_objs.to_set() == s.resources().values().filter(
                    |o: DynamicObjectView| {
                        &&& o.kind == PodView::kind()
                        &&& o.metadata.namespace.is_Some()
                        &&& o.metadata.namespace.unwrap() == vrs.metadata.namespace.unwrap()
                    }
                )
            }
        }
    }
}

pub open spec fn resp_msg_is_the_in_flight_list_resp_at_after_list_pods_step(
    vrs: VReplicaSetView, resp_msg: VRSMessage
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let step = VReplicaSetReconcileStep::AfterListPods;
        let msg = s.ongoing_reconciles()[vrs.object_ref()].pending_req_msg.get_Some_0();
        let request = msg.content.get_APIRequest_0();
        &&& at_vrs_step_with_vrs(vrs, step)(s)
        &&& VRSCluster::has_pending_k8s_api_req_msg(s, vrs.object_ref())
        &&& msg.src == HostId::CustomController
        &&& msg.dst == HostId::ApiServer
        &&& msg.content.is_APIRequest()
        &&& request.is_ListRequest()
        &&& request.get_ListRequest_0() == ListRequest {
            kind: PodView::kind(),
            namespace: vrs.metadata.namespace.unwrap(),
        }
        &&& s.in_flight().contains(resp_msg)
        &&& Message::resp_msg_matches_req_msg(resp_msg, msg)
        &&& resp_msg.content.get_list_response().res.is_Ok()
        &&& {
            let resp_objs = resp_msg.content.get_list_response().res.unwrap();
            // The response must give back all the pods in the replicaset's namespace.
            resp_objs.to_set() == s.resources().values().filter(
                |o: DynamicObjectView| {
                    &&& o.kind == PodView::kind()
                    &&& o.metadata.namespace.is_Some()
                    &&& o.metadata.namespace.unwrap() == vrs.metadata.namespace.unwrap()
                }
            )
        }
    }
}

// Pod creation predicates
pub open spec fn pending_req_in_flight_at_after_create_pod_step(
    vrs: VReplicaSetView, diff: nat
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let step = VReplicaSetReconcileStep::AfterCreatePod(diff as usize);
        let msg = s.ongoing_reconciles()[vrs.object_ref()].pending_req_msg.get_Some_0();
        let request = msg.content.get_APIRequest_0();
        &&& at_vrs_step_with_vrs(vrs, step)(s)
        &&& VRSCluster::has_pending_k8s_api_req_msg(s, vrs.object_ref())
        &&& s.in_flight().contains(msg)
        &&& msg.src == HostId::CustomController
        &&& msg.dst == HostId::ApiServer
        &&& msg.content.is_APIRequest()
        &&& request.is_CreateRequest()
        &&& request.get_CreateRequest_0() == CreateRequest {
            namespace: vrs.metadata.namespace.unwrap(),
            obj: make_pod(vrs).marshal(),
        }
    }
}

pub open spec fn req_msg_is_the_in_flight_create_request_at_after_create_pod_step(
    vrs: VReplicaSetView, req_msg: VRSMessage, diff: nat
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let step = VReplicaSetReconcileStep::AfterCreatePod(diff as usize);
        let request = req_msg.content.get_APIRequest_0();
        &&& at_vrs_step_with_vrs(vrs, step)(s)
        &&& VRSCluster::has_pending_k8s_api_req_msg(s, vrs.object_ref())
        &&& s.in_flight().contains(req_msg)
        &&& req_msg.src == HostId::CustomController
        &&& req_msg.dst == HostId::ApiServer
        &&& req_msg.content.is_APIRequest()
        &&& request.is_CreateRequest()
        &&& request.get_CreateRequest_0() == CreateRequest {
            namespace: vrs.metadata.namespace.unwrap(),
            obj: make_pod(vrs).marshal(),
        }
    }
}

pub open spec fn exists_ok_resp_in_flight_at_after_create_pod_step(
    vrs: VReplicaSetView, diff: nat
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let step = VReplicaSetReconcileStep::AfterCreatePod(diff as usize);
        let msg = s.ongoing_reconciles()[vrs.object_ref()].pending_req_msg.get_Some_0();
        let request = msg.content.get_APIRequest_0();
        &&& at_vrs_step_with_vrs(vrs, step)(s)
        &&& VRSCluster::has_pending_k8s_api_req_msg(s, vrs.object_ref())
        &&& msg.src == HostId::CustomController
        &&& msg.dst == HostId::ApiServer
        &&& msg.content.is_APIRequest()
        &&& request.is_CreateRequest()
        &&& request.get_CreateRequest_0() == CreateRequest {
            namespace: vrs.metadata.namespace.unwrap(),
            obj: make_pod(vrs).marshal(),
        }
        &&& exists |resp_msg| {
            &&& #[trigger] s.in_flight().contains(resp_msg)
            &&& Message::resp_msg_matches_req_msg(resp_msg, msg)
            &&& resp_msg.content.get_create_response().res.is_Ok()
        }
    }
}

pub open spec fn resp_msg_is_the_in_flight_ok_resp_at_after_create_pod_step(
    vrs: VReplicaSetView, resp_msg: VRSMessage, diff: nat
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let step = VReplicaSetReconcileStep::AfterCreatePod(diff as usize);
        let msg = s.ongoing_reconciles()[vrs.object_ref()].pending_req_msg.get_Some_0();
        let request = msg.content.get_APIRequest_0();
        &&& at_vrs_step_with_vrs(vrs, step)(s)
        &&& VRSCluster::has_pending_k8s_api_req_msg(s, vrs.object_ref())
        &&& msg.src == HostId::CustomController
        &&& msg.dst == HostId::ApiServer
        &&& msg.content.is_APIRequest()
        &&& request.is_CreateRequest()
        &&& request.get_CreateRequest_0() == CreateRequest {
            namespace: vrs.metadata.namespace.unwrap(),
            obj: make_pod(vrs).marshal(),
        }
        &&& s.in_flight().contains(resp_msg)
        &&& Message::resp_msg_matches_req_msg(resp_msg, msg)
        &&& resp_msg.content.get_create_response().res.is_Ok()
    }
}

// Pod deletion predicates

// Placeholder predicate constraining the delete request
// We'll probably need something here ensuring we only delete the
// appropriate keys: this will facilitate modifications by a search-and-replace.
pub open spec fn delete_constraint(
    vrs: VReplicaSetView, req: DeleteRequest
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        true // placeholder
    }
}


pub open spec fn pending_req_in_flight_at_after_delete_pod_step(
    vrs: VReplicaSetView, diff: nat
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let step = VReplicaSetReconcileStep::AfterDeletePod(diff as usize);
        let msg = s.ongoing_reconciles()[vrs.object_ref()].pending_req_msg.get_Some_0();
        let request = msg.content.get_APIRequest_0();
        &&& at_vrs_step_with_vrs(vrs, step)(s)
        &&& VRSCluster::has_pending_k8s_api_req_msg(s, vrs.object_ref())
        &&& s.in_flight().contains(msg)
        &&& msg.src == HostId::CustomController
        &&& msg.dst == HostId::ApiServer
        &&& msg.content.is_APIRequest()
        &&& request.is_DeleteRequest()
        &&& delete_constraint(vrs, request.get_DeleteRequest_0())(s)
    }
}

pub open spec fn req_msg_is_the_in_flight_delete_request_at_after_delete_pod_step(
    vrs: VReplicaSetView, req_msg: VRSMessage, diff: nat
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let step = VReplicaSetReconcileStep::AfterDeletePod(diff as usize);
        let request = req_msg.content.get_APIRequest_0();
        &&& at_vrs_step_with_vrs(vrs, step)(s)
        &&& VRSCluster::has_pending_k8s_api_req_msg(s, vrs.object_ref())
        &&& s.in_flight().contains(req_msg)
        &&& req_msg.src == HostId::CustomController
        &&& req_msg.dst == HostId::ApiServer
        &&& req_msg.content.is_APIRequest()
        &&& request.is_DeleteRequest()
        &&& delete_constraint(vrs, request.get_DeleteRequest_0())(s)
    }
}

pub open spec fn exists_ok_resp_in_flight_at_after_delete_pod_step(
    vrs: VReplicaSetView, diff: nat
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let step = VReplicaSetReconcileStep::AfterDeletePod(diff as usize);
        let msg = s.ongoing_reconciles()[vrs.object_ref()].pending_req_msg.get_Some_0();
        let request = msg.content.get_APIRequest_0();
        &&& at_vrs_step_with_vrs(vrs, step)(s)
        &&& VRSCluster::has_pending_k8s_api_req_msg(s, vrs.object_ref())
        &&& msg.src == HostId::CustomController
        &&& msg.dst == HostId::ApiServer
        &&& msg.content.is_APIRequest()
        &&& request.is_DeleteRequest()
        &&& delete_constraint(vrs, request.get_DeleteRequest_0())(s)
        &&& exists |resp_msg| {
            &&& #[trigger] s.in_flight().contains(resp_msg)
            &&& Message::resp_msg_matches_req_msg(resp_msg, msg)
            &&& resp_msg.content.get_delete_response().res.is_Ok()
        }
    }
}

pub open spec fn resp_msg_is_the_in_flight_ok_resp_at_after_delete_pod_step(
    vrs: VReplicaSetView, resp_msg: VRSMessage, diff: nat
) -> StatePred<VRSCluster> {
    |s: VRSCluster| {
        let step = VReplicaSetReconcileStep::AfterDeletePod(diff as usize);
        let msg = s.ongoing_reconciles()[vrs.object_ref()].pending_req_msg.get_Some_0();
        let request = msg.content.get_APIRequest_0();
        &&& at_vrs_step_with_vrs(vrs, step)(s)
        &&& VRSCluster::has_pending_k8s_api_req_msg(s, vrs.object_ref())
        &&& msg.src == HostId::CustomController
        &&& msg.dst == HostId::ApiServer
        &&& msg.content.is_APIRequest()
        &&& request.is_DeleteRequest()
        &&& delete_constraint(vrs, request.get_DeleteRequest_0())(s)
        &&& s.in_flight().contains(resp_msg)
        &&& Message::resp_msg_matches_req_msg(resp_msg, msg)
        &&& resp_msg.content.get_delete_response().res.is_Ok()
    }
}

}
