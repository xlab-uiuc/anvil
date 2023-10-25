// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
use crate::kubernetes_api_objects::{
    api_resource::*, common::*, dynamic::*, label_selector::*, marshal::*, object_meta::*,
    persistent_volume_claim::*, pod_template_spec::*, resource::*, stateful_set::*,
};
use crate::vstd_ext::string_map::*;
use vstd::prelude::*;
use vstd::string::*;

verus! {
// Tests for stateful set
#[test]
#[verifier(external)]
pub fn test_default() {
    let stateful_set = StatefulSet::default();
    assert_eq!(
        stateful_set.into_kube(),
        deps_hack::k8s_openapi::api::apps::v1::StatefulSet::default()
    );
}

#[test]
#[verifier(external)]
pub fn test_set_metadata() {
    let mut object_meta = ObjectMeta::default();
    object_meta.set_name(new_strlit("name").to_string());
    object_meta.set_namespace(new_strlit("namespace").to_string());
    let mut stateful_set = StatefulSet::default();
    stateful_set.set_metadata(object_meta.clone());
    assert_eq!(object_meta.into_kube(), stateful_set.into_kube().metadata);
}

#[test]
#[verifier(external)]
pub fn test_metadata() {
    let mut object_meta = ObjectMeta::default();
    object_meta.set_name(new_strlit("name").to_string());
    object_meta.set_namespace(new_strlit("namespace").to_string());
    let mut stateful_set = StatefulSet::default();
    stateful_set.set_metadata(object_meta.clone());
    assert_eq!(object_meta.into_kube(), stateful_set.metadata().into_kube());
}

#[test]
#[verifier(external)]
pub fn test_set_spec() {
    let mut stateful_set_spec = StatefulSetSpec::default();
    stateful_set_spec.set_replicas(1);
    let mut stateful_set = StatefulSet::default();
    stateful_set.set_spec(stateful_set_spec.clone());
    assert_eq!(stateful_set_spec.into_kube(), stateful_set.into_kube().spec.unwrap());
}

#[test]
#[verifier(external)]
pub fn test_spec() {
    let mut stateful_set_spec = StatefulSetSpec::default();
    stateful_set_spec.set_replicas(1024);
    let mut stateful_set = StatefulSet::default();
    let temp = stateful_set.spec();
    if !temp.is_none() {
        panic!("StatefulSet spec should be None, but it's not.");
    }
    stateful_set.set_spec(stateful_set_spec.clone());
    assert_eq!(stateful_set_spec.into_kube(), stateful_set.spec().unwrap().into_kube());
}

#[test]
#[verifier(external)]
pub fn test_api_resource() {
    let api_resource = StatefulSet::api_resource();
    assert_eq!(api_resource.into_kube().kind, "StatefulSet");
}
}