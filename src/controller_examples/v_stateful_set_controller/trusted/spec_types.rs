// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
use crate::external_api::spec::{EmptyAPI, EmptyTypeView};
use crate::kubernetes_api_objects::error::*;
use crate::kubernetes_api_objects::spec::{
    api_resource::*, label_selector::*, pod_template_spec::*, prelude::*,
};
use crate::kubernetes_cluster::spec::{cluster::*, cluster_state_machine::*, message::*};
use crate::v_stateful_set_controller::trusted::step::*;
use crate::vstd_ext::string_view::*;
use vstd::prelude::*;

verus! {

pub type VSTSStep = Step<VSTSMessage>;

pub type VSTSCluster = Cluster<VStatefulSetView, EmptyAPI, VStatefulSetReconciler>;

pub type VSTSMessage = Message<EmptyTypeView, EmptyTypeView>;

pub struct VStatefulSetReconciler {}

pub struct VStatefulSetReconcileState {
    pub reconcile_step: VStatefulSetReconcileStepView,
    pub replicas: Option<Seq<Option<PodView>>>,
    pub condemned: Option<Seq<PodView>>,
}

pub struct VStatefulSetView {
    pub metadata: ObjectMetaView,
    pub spec: VStatefulSetSpecView,
    pub status: Option<VStatefulSetStatusView>,
}

pub type VStatefulSetStatusView = EmptyStatusView;

impl VStatefulSetView {
    pub open spec fn well_formed(self) -> bool {
        true // Just assume everything's well-formed for now: no verification.
        // &&& self.metadata.name.is_Some()
        // &&& self.metadata.namespace.is_Some()
        // &&& self.metadata.uid.is_Some()
        // // TODO: ensure that the following is consistent with k8s's StatefulSet
        // &&& self.spec.template.is_Some()
        // &&& self.spec.template.get_Some_0().metadata.is_Some()
        // &&& self.spec.template.get_Some_0().spec.is_Some()
    }

    pub open spec fn controller_owner_ref(self) -> OwnerReferenceView {
        OwnerReferenceView {
            block_owner_deletion: Some(true),
            controller: Some(true),
            kind: Self::kind(),
            name: self.metadata.name.get_Some_0(),
            uid: self.metadata.uid.get_Some_0(),
        }
    }
}

impl ResourceView for VStatefulSetView {
    type Spec = VStatefulSetSpecView;
    type Status = Option<VStatefulSetStatusView>;

    open spec fn default() -> VStatefulSetView {
        VStatefulSetView {
            metadata: ObjectMetaView::default(),
            spec: arbitrary(), // TODO: specify the default value for spec
            status: None,
        }
    }

    open spec fn metadata(self) -> ObjectMetaView { self.metadata }

    open spec fn kind() -> Kind { Kind::CustomResourceKind }

    open spec fn object_ref(self) -> ObjectRef {
        ObjectRef {
            kind: Self::kind(),
            name: self.metadata.name.get_Some_0(),
            namespace: self.metadata.namespace.get_Some_0(),
        }
    }

    proof fn object_ref_is_well_formed() {}

    open spec fn spec(self) -> VStatefulSetSpecView { self.spec }

    open spec fn status(self) -> Option<VStatefulSetStatusView> { self.status }

    open spec fn marshal(self) -> DynamicObjectView {
        DynamicObjectView {
            kind: Self::kind(),
            metadata: self.metadata,
            spec: VStatefulSetView::marshal_spec(self.spec),
            status: VStatefulSetView::marshal_status(self.status),
        }
    }

    open spec fn unmarshal(obj: DynamicObjectView) -> Result<VStatefulSetView, ParseDynamicObjectError> {
        if obj.kind != Self::kind() {
            Err(ParseDynamicObjectError::UnmarshalError)
        } else if !VStatefulSetView::unmarshal_spec(obj.spec).is_Ok() {
            Err(ParseDynamicObjectError::UnmarshalError)
        } else if !VStatefulSetView::unmarshal_status(obj.status).is_Ok() {
            Err(ParseDynamicObjectError::UnmarshalError)
        } else {
            Ok(VStatefulSetView {
                metadata: obj.metadata,
                spec: VStatefulSetView::unmarshal_spec(obj.spec).get_Ok_0(),
                status: VStatefulSetView::unmarshal_status(obj.status).get_Ok_0(),
            })
        }
    }

    proof fn marshal_preserves_integrity() {
        VStatefulSetView::marshal_spec_preserves_integrity();
        VStatefulSetView::marshal_status_preserves_integrity();
    }

    proof fn marshal_preserves_metadata() {}

    proof fn marshal_preserves_kind() {}

    closed spec fn marshal_spec(s: VStatefulSetSpecView) -> Value;

    closed spec fn unmarshal_spec(v: Value) -> Result<VStatefulSetSpecView, ParseDynamicObjectError>;

    closed spec fn marshal_status(s: Option<VStatefulSetStatusView>) -> Value;

    closed spec fn unmarshal_status(v: Value) -> Result<Option<VStatefulSetStatusView>, ParseDynamicObjectError>;

    #[verifier(external_body)]
    proof fn marshal_spec_preserves_integrity() {}

    #[verifier(external_body)]
    proof fn marshal_status_preserves_integrity() {}

    proof fn unmarshal_result_determined_by_unmarshal_spec_and_status() {}

    open spec fn state_validation(self) -> bool {
        true
    }

    open spec fn transition_validation(self, old_obj: VStatefulSetView) -> bool {
        true
    }

}

impl CustomResourceView for VStatefulSetView {
    proof fn kind_is_custom_resource() {}
}

pub struct VStatefulSetSpecView {
    pub service_name: StringView,
    pub selector: LabelSelectorView,
    pub template: PodTemplateSpecView,
    pub replicas: Option<i32>,
}

}
