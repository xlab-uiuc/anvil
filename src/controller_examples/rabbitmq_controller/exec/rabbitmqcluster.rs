// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
use crate::kubernetes_api_objects::error::ParseDynamicObjectError;
use crate::kubernetes_api_objects::{
    api_resource::*, common::*, dynamic::*, marshal::*, object_meta::*, resource::*,
};
use crate::pervasive_ext::string_view::*;
use crate::rabbitmq_controller::spec::rabbitmqcluster::*;
use vstd::prelude::*;

verus! {

#[verifier(external_body)]
pub struct RabbitmqCluster {
    inner: deps_hack::RabbitmqCluster
}


impl RabbitmqCluster {
    pub spec fn view(&self) -> RabbitmqClusterView;

    #[verifier(external_body)]
    pub fn name(&self) -> (name: Option<String>)
        ensures
            self@.name().is_Some() == name.is_Some(),
            name.is_Some() ==> name.get_Some_0()@ == self@.name().get_Some_0(),
    {
        match &self.inner.metadata.name {
            std::option::Option::Some(n) => Option::Some(String::from_rust_string(n.to_string())),
            std::option::Option::None => Option::None,
        }
    }

    #[verifier(external_body)]
    pub fn namespace(&self) -> (namespace: Option<String>)
        ensures
            self@.namespace().is_Some() == namespace.is_Some(),
            namespace.is_Some() ==> namespace.get_Some_0()@ == self@.namespace().get_Some_0(),
    {
        match &self.inner.metadata.namespace {
            std::option::Option::Some(n) => Option::Some(String::from_rust_string(n.to_string())),
            std::option::Option::None => Option::None,
        }
    }

    #[verifier(external_body)]
    pub fn spec(&self) -> (spec: RabbitmqClusterSpec)
        ensures
            spec@ == self@.spec,
    {
        RabbitmqClusterSpec { inner: self.inner.spec.clone() }
    }


    #[verifier(external)]
    pub fn into_kube(self) -> deps_hack::RabbitmqCluster {
        self.inner
    }

    #[verifier(external_body)]
    pub fn api_resource() -> (res: ApiResource)
        ensures
            res@.kind == RabbitmqClusterView::kind(),
    {
        ApiResource::from_kube(deps_hack::kube::api::ApiResource::erase::<deps_hack::RabbitmqCluster>(&()))
    }

    // NOTE: This function assumes serde_json::to_string won't fail!
    #[verifier(external_body)]
    pub fn to_dynamic_object(self) -> (obj: DynamicObject)
        ensures
            obj@ == self@.to_dynamic_object(),
    {
        // TODO: this might be unnecessarily slow
        DynamicObject::from_kube(
            deps_hack::k8s_openapi::serde_json::from_str(&deps_hack::k8s_openapi::serde_json::to_string(&self.inner).unwrap()).unwrap()
        )
    }

    #[verifier(external_body)]
    pub fn from_dynamic_object(obj: DynamicObject) -> (res: Result<RabbitmqCluster, ParseDynamicObjectError>)
        ensures
            res.is_Ok() == RabbitmqClusterView::from_dynamic_object(obj@).is_Ok(),
            res.is_Ok() ==> res.get_Ok_0()@ == RabbitmqClusterView::from_dynamic_object(obj@).get_Ok_0(),
    {
        let parse_result = obj.into_kube().try_parse::<deps_hack::RabbitmqCluster>();
        if parse_result.is_ok() {
            let res = RabbitmqCluster { inner: parse_result.unwrap() };
            Result::Ok(res)
        } else {
            Result::Err(ParseDynamicObjectError::ExecError)
        }
    }
}

impl ResourceWrapper<deps_hack::RabbitmqCluster> for RabbitmqCluster {
    #[verifier(external)]
    fn from_kube(inner: deps_hack::RabbitmqCluster) -> RabbitmqCluster {
        RabbitmqCluster {
            inner: inner
        }
    }

    #[verifier(external)]
    fn into_kube(self) -> deps_hack::RabbitmqCluster {
        self.inner
    }
}

#[verifier(external_body)]
pub struct RabbitmqClusterSpec {
    inner: deps_hack::RabbitmqClusterSpec,
}


impl RabbitmqClusterSpec {
    pub spec fn view(&self) -> RabbitmqClusterSpecView;

    #[verifier(external_body)]
    pub fn replicas(&self) -> (replicas: i32)
        ensures
            replicas as int == self@.replicas,
    {
        self.inner.replicas
    }

    #[verifier(external_body)]
    pub fn rabbitmq_config(&self) -> (rabbitmq_config: Option<RabbitmqClusterConfigurationSpec>)
        ensures
            self@.rabbitmq_config.is_Some() == rabbitmq_config.is_Some(),
            rabbitmq_config.is_Some() ==> rabbitmq_config.get_Some_0()@ == self@.rabbitmq_config.get_Some_0(),
    {
        match &self.inner.rabbitmq_config {
            std::option::Option::Some(n) => Option::Some(RabbitmqClusterConfigurationSpec { inner: n.clone()}),
            std::option::Option::None => Option::None,
        }
    }
}


#[verifier(external_body)]
pub struct RabbitmqClusterConfigurationSpec {
    inner: deps_hack::RabbitmqClusterConfigurationSpec,
}

impl RabbitmqClusterConfigurationSpec {
    pub spec fn view(&self) -> RabbitmqClusterConfigurationSpecView;

    #[verifier(external_body)]
    pub fn additional_config(&self) -> (additional_config: Option<String>)
        ensures
            self@.additional_config.is_Some() == additional_config.is_Some(),
            additional_config.is_Some() ==> additional_config.get_Some_0()@ == self@.additional_config.get_Some_0(),
    {
        match &self.inner.additional_config {
            std::option::Option::Some(n) => Option::Some(String::from_rust_string(n.to_string())),
            std::option::Option::None => Option::None,
        }
    }
}


}