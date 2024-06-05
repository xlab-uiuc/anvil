// Copyright 2024 VMware, Inc.
// SPDX-License-Identifier: MIT
use crate::kubernetes_api_objects::error::ParseDynamicObjectError;
use crate::kubernetes_api_objects::exec::{
    api_resource::*, label_selector::*, pod_template_spec::*, prelude::*,
};
use crate::kubernetes_api_objects::spec::resource::*;
use crate::v_stateful_set_controller::trusted::{spec_types, step::*};
use crate::vstd_ext::{string_map::*, string_view::*};
use deps_hack::kube::Resource;
use vstd::prelude::*;

verus! {

/// VStatefulSetReconcileState describes the local state with which the reconcile functions makes decisions.
pub struct VStatefulSetReconcileState {
    pub reconcile_step: VStatefulSetReconcileStep,
    pub replicas: Option<Vec<Option<Pod>>>,
    pub condemned: Option<Vec<Pod>>,
}

impl View for VStatefulSetReconcileState {
    type V = spec_types::VStatefulSetReconcileState;

    open spec fn view(&self) -> spec_types::VStatefulSetReconcileState {
        spec_types::VStatefulSetReconcileState {
            reconcile_step: self.reconcile_step@,
            replicas: match self.replicas {
                Some(fp) => Some(fp@.map_values(|p: Option<Pod>| 
                    match p {
                        Some(fp) => Some(fp@),
                        None => None,
                    })),
                None => None,
            },
            condemned: match self.condemned {
                Some(fp) => Some(fp@.map_values(|p: Pod| p@)),
                None => None,
            },
        }
    }
}

#[verifier(external_body)]
pub struct VStatefulSet {
    inner: deps_hack::VStatefulSet
}

impl View for VStatefulSet {
    type V = spec_types::VStatefulSetView;

    spec fn view(&self) -> spec_types::VStatefulSetView;
}

impl VStatefulSet {
    #[verifier(external_body)]
    pub fn metadata(&self) -> (metadata: ObjectMeta)
        ensures metadata@ == self@.metadata,
    {
        ObjectMeta::from_kube(self.inner.metadata.clone())
    }

    #[verifier(external_body)]
    pub fn spec(&self) -> (spec: VStatefulSetSpec)
        ensures spec@ == self@.spec,
    {
        VStatefulSetSpec { inner: self.inner.spec.clone() }
    }

    #[verifier(external_body)]
    pub fn api_resource() -> (res: ApiResource)
        ensures res@.kind == spec_types::VStatefulSetView::kind(),
    {
        ApiResource::from_kube(deps_hack::kube::api::ApiResource::erase::<deps_hack::VStatefulSet>(&()))
    }

    #[verifier(external_body)]
    pub fn controller_owner_ref(&self) -> (owner_reference: OwnerReference)
        ensures owner_reference@ == self@.controller_owner_ref(),
    {
        // We can safely unwrap here because the trait method implementation always returns a Some(...)
        OwnerReference::from_kube(self.inner.controller_owner_ref(&()).unwrap())
    }

    // NOTE: This function assumes serde_json::to_string won't fail!
    #[verifier(external_body)]
    pub fn marshal(self) -> (obj: DynamicObject)
        ensures obj@ == self@.marshal(),
    {
        // TODO: this might be unnecessarily slow
        DynamicObject::from_kube(deps_hack::k8s_openapi::serde_json::from_str(&deps_hack::k8s_openapi::serde_json::to_string(&self.inner).unwrap()).unwrap())
    }

    #[verifier(external_body)]
    pub fn unmarshal(obj: DynamicObject) -> (res: Result<VStatefulSet, ParseDynamicObjectError>)
        ensures
            res.is_Ok() == spec_types::VStatefulSetView::unmarshal(obj@).is_Ok(),
            res.is_Ok() ==> res.get_Ok_0()@ == spec_types::VStatefulSetView::unmarshal(obj@).get_Ok_0(),
    {
        let parse_result = obj.into_kube().try_parse::<deps_hack::VStatefulSet>();
        if parse_result.is_ok() {
            let res = VStatefulSet { inner: parse_result.unwrap() };
            Ok(res)
        } else {
            Err(ParseDynamicObjectError::ExecError)
        }
    }
}

#[verifier(external)]
impl ResourceWrapper<deps_hack::VStatefulSet> for VStatefulSet {
    fn from_kube(inner: deps_hack::VStatefulSet) -> VStatefulSet { VStatefulSet { inner: inner } }

    fn into_kube(self) -> deps_hack::VStatefulSet { self.inner }
}

#[verifier(external_body)]
pub struct VStatefulSetSpec {
    inner: deps_hack::VStatefulSetSpec,
}

impl VStatefulSetSpec {
    pub spec fn view(&self) -> spec_types::VStatefulSetSpecView;

    #[verifier(external_body)]
    pub fn service_name(&self) -> (service_name: String)
        ensures service_name@ == self@.service_name
    {
        self.inner.service_name.clone()
    }

    #[verifier(external_body)]
    pub fn selector(&self) -> (selector: LabelSelector)
        ensures selector@ == self@.selector
    {
        LabelSelector::from_kube(self.inner.selector.clone())
    }

    #[verifier(external_body)]
    pub fn template(&self) -> (template: PodTemplateSpec)
        ensures template@ == self@.template
    {
        PodTemplateSpec::from_kube(self.inner.template.clone())
    }

    #[verifier(external_body)]
    pub fn replicas(&self) -> (replicas: Option<i32>)
        ensures
            self@.replicas.is_Some() == replicas.is_Some(),
            replicas.is_Some() ==> replicas.get_Some_0() as int == self@.replicas.get_Some_0(),
    {
        self.inner.replicas
    }
}

}
