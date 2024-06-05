// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
#![allow(unused_imports)]
use vstd::prelude::*;

verus! {

#[is_variant]
pub enum VStatefulSetReconcileStep {
    Init,
    AfterListPods,
    AfterCreatePod(usize),
    Done,
    Error,
}

impl std::marker::Copy for VStatefulSetReconcileStep {}

impl std::clone::Clone for VStatefulSetReconcileStep {
    #[verifier(external_body)]
    fn clone(&self) -> (result: Self)
        ensures result == self
    { *self }
}

impl View for VStatefulSetReconcileStep {
    type V = VStatefulSetReconcileStepView;

    open spec fn view(&self) -> VStatefulSetReconcileStepView {
        match self {
            VStatefulSetReconcileStep::Init => VStatefulSetReconcileStepView::Init,
            VStatefulSetReconcileStep::AfterListPods => VStatefulSetReconcileStepView::AfterListPods,
            VStatefulSetReconcileStep::AfterCreatePod(i) => VStatefulSetReconcileStepView::AfterCreatePod(*i as nat),
            VStatefulSetReconcileStep::Done => VStatefulSetReconcileStepView::Done,
            VStatefulSetReconcileStep::Error => VStatefulSetReconcileStepView::Error,
        }
    }
}

#[is_variant]
pub enum VStatefulSetReconcileStepView {
    Init,
    AfterListPods,
    AfterCreatePod(nat),
    Done,
    Error,
}


}
