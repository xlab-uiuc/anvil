// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
#![allow(unused_imports)]
use crate::external_api::exec::*;
use crate::kubernetes_api_objects::exec::resource::ResourceWrapper;
use crate::kubernetes_api_objects::exec::{
    container::*, label_selector::*, pod_template_spec::*, prelude::*, resource_requirements::*,
    volume::*,
};
use crate::reconciler::exec::{io::*, reconciler::*, resource_builder::*};
// use crate::v_stateful_set_controller::model::reconciler as model_reconciler;
// use crate::v_stateful_set_controller::model::resource as model_resource;
use crate::v_stateful_set_controller::trusted::exec_types::*;
use crate::v_stateful_set_controller::trusted::spec_types;
use crate::v_stateful_set_controller::trusted::step::*;
use crate::vstd_ext::{string_map::StringMap, string_view::*};
use std::convert::TryInto;
use vstd::prelude::*;
use vstd::seq_lib::*;
use vstd::string::*;

verus! {

pub struct VStatefulSetReconciler {}

impl Reconciler for VStatefulSetReconciler {
    type R = VStatefulSet;
    type T = VStatefulSetReconcileState;
    type ExternalAPIType = EmptyAPIShimLayer;

    open spec fn well_formed(v_stateful_set: &VStatefulSet) -> bool { v_stateful_set@.well_formed() }

    fn reconcile_init_state() -> VStatefulSetReconcileState {
        reconcile_init_state()
    }

    fn reconcile_core(v_stateful_set: &VStatefulSet, resp_o: Option<Response<EmptyType>>, state: VStatefulSetReconcileState) -> (VStatefulSetReconcileState, Option<Request<EmptyType>>) {
        reconcile_core(v_stateful_set, resp_o, state)
    }

    fn reconcile_done(state: &VStatefulSetReconcileState) -> bool {
        reconcile_done(state)
    }

    fn reconcile_error(state: &VStatefulSetReconcileState) -> bool {
        reconcile_error(state)
    }
}

impl Default for VStatefulSetReconciler {
    fn default() -> VStatefulSetReconciler { VStatefulSetReconciler{} }
}

pub fn reconcile_init_state() -> (state: VStatefulSetReconcileState)
    // ensures state@ == model_reconciler::reconcile_init_state(),
{
    VStatefulSetReconcileState {
        reconcile_step: VStatefulSetReconcileStep::Init,
        filtered_pods: None,
        replica_count: None,
        replicas: None,
        condemned: None,
    }
}

pub fn reconcile_done(state: &VStatefulSetReconcileState) -> (res: bool)
    // ensures res == model_reconciler::reconcile_done(state@),
{
    match state.reconcile_step {
        VStatefulSetReconcileStep::Done => true,
        _ => false,
    }
}

pub fn reconcile_error(state: &VStatefulSetReconcileState) -> (res: bool)
    // ensures res == model_reconciler::reconcile_error(state@),
{
    match state.reconcile_step {
        VStatefulSetReconcileStep::Error => true,
        _ => false,
    }
}

pub fn reconcile_core(v_stateful_set: &VStatefulSet, resp_o: Option<Response<EmptyType>>, state: VStatefulSetReconcileState) -> (res: (VStatefulSetReconcileState, Option<Request<EmptyType>>))
    requires v_stateful_set@.well_formed(),
    // ensures (res.0@, opt_request_to_view(&res.1)) == model_reconciler::reconcile_core(v_stateful_set@, opt_response_to_view(&resp_o), state@),
{
    let namespace = v_stateful_set.metadata().namespace().unwrap();
    match &state.reconcile_step {
        VStatefulSetReconcileStep::Init => {
            let req = KubeAPIRequest::ListRequest(KubeListRequest {
                api_resource: Pod::api_resource(),
                namespace: namespace,
            });
            let state_prime = VStatefulSetReconcileState {
                reconcile_step: VStatefulSetReconcileStep::AfterListPods,
                ..state
            };
            return (state_prime, Some(Request::KRequest(req)));
        },
        VStatefulSetReconcileStep::AfterListPods => {
            if !(resp_o.is_some() && resp_o.as_ref().unwrap().is_k_response()
            && resp_o.as_ref().unwrap().as_k_response_ref().is_list_response()
            && resp_o.as_ref().unwrap().as_k_response_ref().as_list_response_ref().res.is_ok()) {
                return (error_state(state), None);
            }
            let objs = resp_o.unwrap().into_k_response().into_list_response().res.unwrap();
            let pods_or_none = objects_to_pods(objs);
            if pods_or_none.is_none() {
                return (error_state(state), None);
            }
            let pods = pods_or_none.unwrap();

            // Partition pods associated with this set
            let filtered_pods = filter_pods(pods, v_stateful_set);
            let replica_count = v_stateful_set.spec().replicas().unwrap_or(1);
            let (mut replicas, condemned) = partition_pods(&filtered_pods, replica_count, v_stateful_set);

            // Find first non-existent replica 
            let mut idx = get_start_ordinal(v_stateful_set);
            let mut unalloc_idx = 0;
            let mut unalloc_found = false;
            while idx <= get_end_ordinal(v_stateful_set) {
                if replicas[idx as usize].is_none() {
                    unalloc_idx = idx;
                    unalloc_found = true;
                    break;
                }
                idx = idx + 1;
            }

            // Either create a replica or continue reconciling the StatefulSet.
            if unalloc_found {
                let pod = make_pod(idx, v_stateful_set);
                replicas.set(idx as usize, Some(pod.clone()));
                let req = KubeAPIRequest::CreateRequest(KubeCreateRequest {
                    api_resource: Pod::api_resource(),
                    namespace: namespace,
                    obj: pod.marshal(),
                });
                let state_prime = VStatefulSetReconcileState {
                    reconcile_step: VStatefulSetReconcileStep::AfterCreatePod(idx),
                    filtered_pods: Some(filtered_pods),
                    replica_count: Some(replica_count),
                    replicas: Some(replicas),
                    condemned: Some(condemned),
                    ..state
                }; 
                return (state_prime, Some(Request::KRequest(req)));
            } else {
                let state_prime = VStatefulSetReconcileState {
                    reconcile_step: VStatefulSetReconcileStep::Done,
                    filtered_pods: Some(filtered_pods),
                    replica_count: Some(replica_count),
                    replicas: Some(replicas),
                    condemned: Some(condemned),
                    ..state
                };
                return (state_prime, None);
            }
        },
        VStatefulSetReconcileStep::AfterCreatePod(idx) => {
            let mut idx = *idx;
            if !(resp_o.is_some() && resp_o.as_ref().unwrap().is_k_response()
            && resp_o.as_ref().unwrap().as_k_response_ref().is_create_response()
            && resp_o.as_ref().unwrap().as_k_response_ref().as_create_response_ref().res.is_ok()) {
                return (error_state(state), None);
            }

            // Find next non-existent replica
            let mut replicas = state.replicas.unwrap();
            let mut unalloc_idx = 0;
            let mut unalloc_found = false;
            while idx <= get_end_ordinal(v_stateful_set) {
                if replicas[idx as usize].is_none() {
                    unalloc_idx = idx;
                    unalloc_found = true;
                    break;
                }
                idx = idx + 1;
            }

            // Either create a replica or continue reconciling the StatefulSet.
            if unalloc_found {
                let pod = make_pod(idx, v_stateful_set);
                replicas.set(idx as usize, Some(pod.clone()));
                let req = KubeAPIRequest::CreateRequest(KubeCreateRequest {
                    api_resource: Pod::api_resource(),
                    namespace: namespace,
                    obj: pod.marshal(),
                });
                let state_prime = VStatefulSetReconcileState {
                    reconcile_step: VStatefulSetReconcileStep::AfterCreatePod(idx),
                    replicas: Some(replicas),
                    ..state
                }; 
                return (state_prime, Some(Request::KRequest(req)));
            } else {
                let state_prime = VStatefulSetReconcileState {
                    reconcile_step: VStatefulSetReconcileStep::Done,
                    replicas: Some(replicas),
                    ..state
                };
                return (state_prime, None);
            }
        },
        _ => {
            return (state, None);
        }
    }
}

// TODO: This function can be replaced by a map.
// Revisit it if Verus supports Vec.map.
fn filter_pods(pods: Vec<Pod>, v_stateful_set: &VStatefulSet) -> (filtered_pods: Vec<Pod>)
{
    let mut filtered_pods = Vec::new();
    let mut idx = 0;
    while idx < pods.len() {
        let pod = &pods[idx];
        // TODO: check other conditions such as pod status and deletion timestamp
        if pod.metadata().owner_references_contains(v_stateful_set.controller_owner_ref())
        && v_stateful_set.spec().selector().matches(pod.metadata().labels().unwrap_or(StringMap::new())) {
            filtered_pods.push(pod.clone());
        }
        idx = idx + 1;
    }
    filtered_pods
}

fn partition_pods(pods: &Vec<Pod>, replica_count: i32, v_stateful_set: &VStatefulSet) 
    -> (partitions: (Vec<Option<Pod>>, Vec<Pod>))
{
    let mut replicas: Vec<Option<Pod>> = Vec::new();
    let mut condemned = Vec::with_capacity(pods.len());
    let mut idx: usize = 0;
    let mut temp: Option<Pod> = None;
    while idx < replica_count as usize {
        replicas.push(None);
        idx = idx + 1;
    }
    idx = 0;
    while idx < pods.len() {
        let pod = &pods[idx];
        if pod_in_ordinal_range(pod, v_stateful_set) {
            let idx: usize = (get_ordinal(pod).unwrap() - get_start_ordinal(v_stateful_set)) as usize;
            replicas.set(idx, Some(pod.clone()));
        } else if get_ordinal(pod).unwrap() > 0 {
            condemned.push(pod.clone());
        }
        idx = idx + 1;
    }
    (replicas, condemned)
}

// TODO: re-implement in a way that allows verification.
#[verifier(external_body)]
fn get_parent_name_and_ordinal(pod: &Pod) -> (res: Option<(String, i32)>) 
{
    let pod_name = pod.metadata().name().unwrap();
    match pod_name.as_str().rfind('-') {
        Some(idx) => {
            let raw_ord = &(pod_name.as_bytes())[(idx + 1)..];
            let str_ord = String::from_utf8(raw_ord.to_vec()).unwrap();
            match &str_ord.parse::<i32>() {
                Ok(ord) => {
                    let raw_name = &(pod_name.as_bytes())[..idx];
                    let parent_name = String::from_utf8(raw_name.to_vec()).unwrap();
                    Some((parent_name, *ord))
                },
                Err(_) => None,
            }
        },
        None => None
    }
}

fn get_ordinal(pod: &Pod) -> (ord: Option<i32>) 
{
    match get_parent_name_and_ordinal(pod) {
        Some((_, ord)) => Some(ord),
        None => None,
    }
}

fn get_start_ordinal(v_stateful_set: &VStatefulSet) -> (ord: i32) { 0 }

fn get_end_ordinal(v_stateful_set: &VStatefulSet) -> (ord: i32)
{
    get_start_ordinal(v_stateful_set) + v_stateful_set.spec().replicas().unwrap_or(1) - 1
}

fn pod_in_ordinal_range(pod: &Pod, v_stateful_set: &VStatefulSet) -> (res: bool) 
{
    let ord = get_ordinal(pod).unwrap();
    ord >= get_start_ordinal(v_stateful_set) && ord <= get_end_ordinal(v_stateful_set)
}

pub fn error_state(state: VStatefulSetReconcileState) -> (state_prime: VStatefulSetReconcileState)
{
    VStatefulSetReconcileState {
        reconcile_step: VStatefulSetReconcileStep::Error,
        ..state
    }
}

pub fn make_owner_references(v_stateful_set: &VStatefulSet) -> (owner_references: Vec<OwnerReference>)
    // requires v_stateful_set@.well_formed(),
    // ensures owner_references@.map_values(|or: OwnerReference| or@) ==  model_resource::make_owner_references(v_stateful_set@),
{
    let mut owner_references = Vec::new();
    owner_references.push(v_stateful_set.controller_owner_ref());
    // proof {
    //     assert_seqs_equal!(
    //         owner_references@.map_values(|owner_ref: OwnerReference| owner_ref@),
    //         model_resource::make_owner_references(v_stateful_set@)
    //     );
    // }
    owner_references
}

// TODO: This function can be replaced by a map.
// Revisit it if Verus supports Vec.map.
fn objects_to_pods(objs: Vec<DynamicObject>) -> (pods_or_none: Option<Vec<Pod>>)
{
    let mut pods = Vec::new();
    let mut idx = 0;
    while idx < objs.len() {
        let pod_or_error = Pod::unmarshal(objs[idx].clone());
        if pod_or_error.is_ok() {
            pods.push(pod_or_error.unwrap());
        } else {
            return None;
        }
        idx = idx + 1;
    }
    Some(pods)
}

// TODO: re-implement in a way that allows verification.
#[verifier(external_body)]
fn make_pod_name(parent_name: &String, ordinal: i32) -> (name: String)
{
    let mut result = parent_name.to_string();
    result.push_str(new_strlit("-"));
    result.push_str(ordinal.to_string().as_str());
    result
}

fn make_pod(idx: i32, v_stateful_set: &VStatefulSet) -> (pod: Pod)
    requires v_stateful_set@.well_formed(),
{
    let template = v_stateful_set.spec().template();
    let mut pod = Pod::default();
    pod.set_metadata({
        let mut metadata = ObjectMeta::default();
        let labels = template.metadata().unwrap().labels();
        if labels.is_some() {
            metadata.set_labels(labels.unwrap());
        }
        let labels = template.metadata().unwrap().labels();
        if labels.is_some() {
            metadata.set_labels(labels.unwrap());
        }
        let annotations = template.metadata().unwrap().annotations();
        if annotations.is_some() {
            metadata.set_annotations(annotations.unwrap());
        }
        let finalizers = template.metadata().unwrap().finalizers();
        if finalizers.is_some() {
            metadata.set_finalizers(finalizers.unwrap());
        }
        metadata.set_name(
            make_pod_name(&v_stateful_set.metadata().name().unwrap(), idx)
        );
        metadata.set_owner_references(make_owner_references(v_stateful_set));
        metadata
    });
    pod.set_spec(template.spec().unwrap());
    pod
}

}
