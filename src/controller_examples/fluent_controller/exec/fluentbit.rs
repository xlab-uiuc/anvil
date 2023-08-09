// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
use crate::fluent_controller::spec::fluentbit::*;
use crate::kubernetes_api_objects::error::ParseDynamicObjectError;
use crate::kubernetes_api_objects::{
    api_resource::*, common::*, dynamic::*, marshal::*, object_meta::*, resource::*,
    resource_requirements::*,
};
use crate::pervasive_ext::string_view::*;
use vstd::prelude::*;

verus! {

#[verifier(external_body)]
pub struct FluentBit {
    inner: deps_hack::FluentBit
}

impl FluentBit {
    pub spec fn view(&self) -> FluentBitView;

    #[verifier(external_body)]
    pub fn metadata(&self) -> (metadata: ObjectMeta)
        ensures
            metadata@ == self@.metadata,
    {
        ObjectMeta::from_kube(self.inner.metadata.clone())
    }

    #[verifier(external_body)]
    pub fn spec(&self) -> (spec: FluentBitSpec)
        ensures
            spec@ == self@.spec,
    {
        FluentBitSpec { inner: self.inner.spec.clone() }
    }

    #[verifier(external)]
    pub fn into_kube(self) -> deps_hack::FluentBit {
        self.inner
    }

    #[verifier(external_body)]
    pub fn api_resource() -> (res: ApiResource)
        ensures
            res@.kind == FluentBitView::kind(),
    {
        ApiResource::from_kube(deps_hack::kube::api::ApiResource::erase::<deps_hack::FluentBit>(&()))
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
    pub fn from_dynamic_object(obj: DynamicObject) -> (res: Result<FluentBit, ParseDynamicObjectError>)
        ensures
            res.is_Ok() == FluentBitView::from_dynamic_object(obj@).is_Ok(),
            res.is_Ok() ==> res.get_Ok_0()@ == FluentBitView::from_dynamic_object(obj@).get_Ok_0(),
    {
        let parse_result = obj.into_kube().try_parse::<deps_hack::FluentBit>();
        if parse_result.is_ok() {
            let res = FluentBit { inner: parse_result.unwrap() };
            Ok(res)
        } else {
            Err(ParseDynamicObjectError::ExecError)
        }
    }
}

impl ResourceWrapper<deps_hack::FluentBit> for FluentBit {
    #[verifier(external)]
    fn from_kube(inner: deps_hack::FluentBit) -> FluentBit {
        FluentBit {
            inner: inner
        }
    }

    #[verifier(external)]
    fn into_kube(self) -> deps_hack::FluentBit {
        self.inner
    }
}

#[verifier(external_body)]
pub struct FluentBitSpec {
    inner: deps_hack::FluentBitSpec,
}

impl FluentBitSpec {
    pub spec fn view(&self) -> FluentBitSpecView;

    #[verifier(external_body)]
    pub fn fluentbit_config(&self) -> (fluentbit_config: String)
        ensures
            fluentbit_config@ == self@.fluentbit_config,
    {
        String::from_rust_string(self.inner.fluentbit_config.to_string())
    }

    #[verifier(external_body)]
    pub fn parsers_config(&self) -> (parsers_config: String)
        ensures
            parsers_config@ == self@.parsers_config,
    {
        String::from_rust_string(self.inner.parsers_config.to_string())
    }

    #[verifier(external_body)]
    pub fn resources(&self) -> (resources: ResourceRequirements)
        ensures
            resources@ == self@.resources,
    {
        ResourceRequirements::from_kube(self.inner.resources.clone())
    }
}

}