// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
use crate::kubernetes_api_objects::container::*;
use crate::kubernetes_api_objects::object_meta::*;
use crate::kubernetes_api_objects::pod::*;
use crate::kubernetes_api_objects::resource::*;
use crate::kubernetes_api_objects::volume::*;
use crate::vstd_ext::string_map::*;
use vstd::prelude::*;
use vstd::string::*;

verus! {
// Tests for config map volume source
#[test]
#[verifier(external)]
pub fn test_default() {
    let config_map_volume_source = ConfigMapVolumeSource::default();
    assert_eq!(config_map_volume_source.into_kube(), deps_hack::k8s_openapi::api::core::v1::ConfigMapVolumeSource::default());
}

#[test]
#[verifier(external)]
pub fn test_set_name() {
    let mut config_map_volume_source = ConfigMapVolumeSource::default();
    config_map_volume_source.set_name(new_strlit("name").to_string());
    assert_eq!("name".to_string(), config_map_volume_source.into_kube().name.unwrap());
}
}