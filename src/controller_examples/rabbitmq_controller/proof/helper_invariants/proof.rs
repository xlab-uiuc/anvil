// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
#![allow(unused_imports)]
use super::predicate::*;
use crate::kubernetes_api_objects::{
    api_method::*, common::*, error::*, owner_reference::*, prelude::*, resource::*,
};
use crate::kubernetes_cluster::spec::{
    cluster::*,
    cluster_state_machine::Step,
    controller::common::{ControllerActionInput, ControllerStep},
    message::*,
};
use crate::rabbitmq_controller::{
    common::*,
    proof::{
        helper_invariants::stateful_set_in_etcd_satisfies_unchangeable,
        liveness::resource_match::sub_resource_state_matches, predicate::*, resource::*,
    },
    spec::{resource::make_stateful_set, types::*},
};
use crate::temporal_logic::{defs::*, rules::*};
use crate::vstd_ext::{multiset_lib, seq_lib, string_view::*};
use vstd::{multiset::*, prelude::*, string::*};

verus! {

pub proof fn lemma_always_cr_objects_in_etcd_satisfy_state_validation(spec: TempPred<RMQCluster>)
    requires
        spec.entails(lift_state(RMQCluster::init())),
        spec.entails(always(lift_action(RMQCluster::next()))),
    ensures
        spec.entails(always(lift_state(cr_objects_in_etcd_satisfy_state_validation()))),
{
    let inv = cr_objects_in_etcd_satisfy_state_validation();
    RabbitmqClusterView::marshal_status_preserves_integrity();
    init_invariant(spec, RMQCluster::init(), RMQCluster::next(), inv);
}

pub proof fn lemma_always_the_object_in_schedule_satisfies_state_validation(spec: TempPred<RMQCluster>)
    requires
        spec.entails(lift_state(RMQCluster::init())),
        spec.entails(always(lift_action(RMQCluster::next()))),
    ensures
        spec.entails(always(lift_state(the_object_in_schedule_satisfies_state_validation()))),
{
    let inv = the_object_in_schedule_satisfies_state_validation();
    let stronger_next = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& cr_objects_in_etcd_satisfy_state_validation()(s)
    };
    lemma_always_cr_objects_in_etcd_satisfy_state_validation(spec);
    combine_spec_entails_always_n!(
        spec, lift_action(stronger_next),
        lift_action(RMQCluster::next()),
        lift_state(cr_objects_in_etcd_satisfy_state_validation())
    );
    init_invariant(spec, RMQCluster::init(), stronger_next, inv);
}

pub proof fn lemma_always_the_object_in_reconcile_satisfies_state_validation(spec: TempPred<RMQCluster>)
    requires
        spec.entails(lift_state(RMQCluster::init())),
        spec.entails(always(lift_action(RMQCluster::next()))),
    ensures
        spec.entails(always(lift_state(the_object_in_reconcile_satisfies_state_validation()))),
{
    let inv = the_object_in_reconcile_satisfies_state_validation();
    let stronger_next = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& the_object_in_schedule_satisfies_state_validation()(s)
    };
    lemma_always_the_object_in_schedule_satisfies_state_validation(spec);
    combine_spec_entails_always_n!(
        spec, lift_action(stronger_next),
        lift_action(RMQCluster::next()),
        lift_state(the_object_in_schedule_satisfies_state_validation())
    );
    init_invariant(spec, RMQCluster::init(), stronger_next, inv);
}

pub proof fn lemma_eventually_always_cm_rv_is_the_same_as_etcd_server_cm_if_cm_updated_forall(spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_etcd_is_well_formed()))),
        spec.entails(always(lift_state(object_in_response_at_after_create_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(object_in_response_at_after_update_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(tla_forall(|res: SubResource| lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(res, rabbitmq))))),
        spec.entails(always(tla_forall(|res: SubResource| lift_state(no_delete_resource_request_msg_in_flight(res, rabbitmq))))),
        spec.entails(always(tla_forall(|res: SubResource| lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(res, rabbitmq))))),
        spec.entails(true_pred().leads_to(lift_state(|s: RMQCluster| !s.ongoing_reconciles().contains_key(rabbitmq.object_ref())))),
    ensures
        spec.entails(true_pred().leads_to(always(lift_state(cm_rv_is_the_same_as_etcd_server_cm_if_cm_updated(rabbitmq))))),
{
    always_tla_forall_apply(spec, |res: SubResource| lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(res, rabbitmq)), SubResource::ServerConfigMap);
    always_tla_forall_apply(spec, |res: SubResource| lift_state(no_delete_resource_request_msg_in_flight(res, rabbitmq)), SubResource::ServerConfigMap);
    always_tla_forall_apply(spec, |res: SubResource| lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(res, rabbitmq)), SubResource::ServerConfigMap);
    lemma_eventually_always_cm_rv_is_the_same_as_etcd_server_cm_if_cm_updated(spec, rabbitmq);
}

#[verifier(spinoff_prover)]
pub proof fn lemma_eventually_always_cm_rv_is_the_same_as_etcd_server_cm_if_cm_updated(spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_etcd_is_well_formed()))),
        spec.entails(always(lift_state(no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(object_in_response_at_after_create_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(object_in_response_at_after_update_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(true_pred().leads_to(lift_state(|s: RMQCluster| !s.ongoing_reconciles().contains_key(rabbitmq.object_ref())))),
    ensures
        spec.entails(true_pred().leads_to(always(lift_state(cm_rv_is_the_same_as_etcd_server_cm_if_cm_updated(rabbitmq))))),
{
    let key = rabbitmq.object_ref();
    let inv = cm_rv_is_the_same_as_etcd_server_cm_if_cm_updated(rabbitmq);
    let next = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()(s)
        &&& RMQCluster::every_in_flight_msg_has_unique_id()(s)
        &&& RMQCluster::each_object_in_etcd_is_well_formed()(s_prime)
        &&& no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& object_in_response_at_after_create_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& object_in_response_at_after_update_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq)(s)
    };
    always_to_always_later(spec, lift_state(RMQCluster::each_object_in_etcd_is_well_formed()));
    combine_spec_entails_always_n!(
        spec, lift_action(next), lift_action(RMQCluster::next()),
        lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()),
        lift_state(RMQCluster::every_in_flight_msg_has_unique_id()),
        later(lift_state(RMQCluster::each_object_in_etcd_is_well_formed())),
        lift_state(no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(object_in_response_at_after_create_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(object_in_response_at_after_update_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq))
    );
    leads_to_weaken_temp(
        spec, true_pred(), lift_state(|s: RMQCluster| !s.ongoing_reconciles().contains_key(rabbitmq.object_ref())),
        true_pred(), lift_state(inv)
    );
    leads_to_stable_temp(spec, lift_action(next), true_pred(), lift_state(inv));
}

pub proof fn lemma_eventually_always_object_in_response_at_after_create_resource_step_is_same_as_etcd_forall(
    spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_etcd_is_well_formed()))),
        spec.entails(always(lift_state(RMQCluster::key_of_object_in_matched_ok_create_resp_message_is_same_as_key_of_pending_req(rabbitmq.object_ref())))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_or_pending_req_msg_has_unique_id()))),
        spec.entails(always(tla_forall(|res: SubResource| lift_state(no_delete_resource_request_msg_in_flight(res, rabbitmq))))),
        spec.entails(always(tla_forall(|res: SubResource| lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(res, rabbitmq))))),
        spec.entails(true_pred().leads_to(lift_state(|s: RMQCluster| !s.ongoing_reconciles().contains_key(rabbitmq.object_ref())))),
        spec.entails(always(tla_forall(|res: SubResource| lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(res, rabbitmq))))),
        spec.entails(always(tla_forall(|res: SubResource| lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(res, rabbitmq))))),
    ensures
        spec.entails(true_pred().leads_to(always(lift_state(object_in_response_at_after_create_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq))))),
{
    always_tla_forall_apply(spec, |res: SubResource| lift_state(no_delete_resource_request_msg_in_flight(res, rabbitmq)), SubResource::ServerConfigMap);
    always_tla_forall_apply(spec, |res: SubResource| lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(res, rabbitmq)), SubResource::ServerConfigMap);
    always_tla_forall_apply(spec, |res: SubResource| lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(res, rabbitmq)), SubResource::ServerConfigMap);
    always_tla_forall_apply(spec, |res: SubResource| lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(res, rabbitmq)), SubResource::ServerConfigMap);
    lemma_eventually_always_object_in_response_at_after_create_resource_step_is_same_as_etcd(spec, rabbitmq);
}

#[verifier(spinoff_prover)]
pub proof fn lemma_eventually_always_object_in_response_at_after_create_resource_step_is_same_as_etcd(
    spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::key_of_object_in_matched_ok_create_resp_message_is_same_as_key_of_pending_req(rabbitmq.object_ref())))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_or_pending_req_msg_has_unique_id()))),
        spec.entails(always(lift_state(no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(true_pred().leads_to(lift_state(|s: RMQCluster| !s.ongoing_reconciles().contains_key(rabbitmq.object_ref())))),
        spec.entails(always(lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(SubResource::ServerConfigMap, rabbitmq)))),
    ensures
        spec.entails(true_pred().leads_to(always(lift_state(object_in_response_at_after_create_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq))))),
{
    let key = rabbitmq.object_ref();
    let inv = object_in_response_at_after_create_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq);
    let next = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()(s)
        &&& RMQCluster::every_in_flight_msg_has_unique_id()(s)
        &&& RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()(s)
        &&& RMQCluster::key_of_object_in_matched_ok_create_resp_message_is_same_as_key_of_pending_req(key)(s_prime)
        &&& RMQCluster::every_in_flight_or_pending_req_msg_has_unique_id()(s)
        &&& no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(SubResource::ServerConfigMap, rabbitmq)(s)
    };
    always_to_always_later(spec, lift_state(RMQCluster::key_of_object_in_matched_ok_create_resp_message_is_same_as_key_of_pending_req(key)));
    combine_spec_entails_always_n!(
        spec, lift_action(next), lift_action(RMQCluster::next()),
        lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()),
        lift_state(RMQCluster::every_in_flight_msg_has_unique_id()),
        lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()),
        later(lift_state(RMQCluster::key_of_object_in_matched_ok_create_resp_message_is_same_as_key_of_pending_req(key))),
        lift_state(RMQCluster::every_in_flight_or_pending_req_msg_has_unique_id()),
        lift_state(no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(SubResource::ServerConfigMap, rabbitmq))
    );
    leads_to_weaken_temp(
        spec, true_pred(), lift_state(|s: RMQCluster| !s.ongoing_reconciles().contains_key(rabbitmq.object_ref())),
        true_pred(), lift_state(inv)
    );
    let resource_key = get_request(SubResource::ServerConfigMap, rabbitmq).key;
    let key = rabbitmq.object_ref();
    assert forall |s: RMQCluster, s_prime: RMQCluster| inv(s) && #[trigger] next(s, s_prime) implies inv(s_prime) by {
        let pending_req = s_prime.ongoing_reconciles()[key].pending_req_msg.get_Some_0();
        if at_rabbitmq_step(key, RabbitmqReconcileStep::AfterKRequestStep(ActionKind::Create, SubResource::ServerConfigMap))(s_prime) {
            assert_by(
                s_prime.ongoing_reconciles()[key].pending_req_msg.is_Some()
                && resource_create_request_msg(resource_key)(s_prime.ongoing_reconciles()[key].pending_req_msg.get_Some_0()),
                {
                    let step = choose |step| RMQCluster::next_step(s, s_prime, step);
                    match step {
                        Step::ControllerStep(input) => {
                            let cr_key = input.1.get_Some_0();
                            if cr_key == key {
                                assert(s_prime.ongoing_reconciles()[key].pending_req_msg.is_Some());
                                assert(resource_create_request_msg(resource_key)(s_prime.ongoing_reconciles()[key].pending_req_msg.get_Some_0()));
                            } else {
                                assert(s_prime.ongoing_reconciles()[key] == s.ongoing_reconciles()[key]);
                            }
                        },
                        Step::RestartController() => {
                            assert(false);
                        },
                        _ => {
                            assert(s_prime.ongoing_reconciles()[key] == s.ongoing_reconciles()[key]);
                        }
                    }
                }
            );
            assert forall |msg: RMQMessage| #[trigger] s_prime.in_flight().contains(msg) && Message::resp_msg_matches_req_msg(msg, pending_req) implies resource_create_response_msg(resource_key, s_prime)(msg) by {
                assert(msg.src.is_KubernetesAPI());
                assert(msg.content.is_create_response());
                if msg.content.get_create_response().res.is_Ok() {
                    let step = choose |step| RMQCluster::next_step(s, s_prime, step);
                    if !s.in_flight().contains(msg) {
                        assert(step.is_KubernetesAPIStep());
                        let req = step.get_KubernetesAPIStep_0().get_Some_0();
                        assert(msg.content.get_create_response().res.get_Ok_0().object_ref() == req.content.get_create_request().key());
                        assert(msg.content.get_create_response().res.get_Ok_0().object_ref() == resource_key);
                        assert(msg.content.get_create_response().res.get_Ok_0() == s_prime.resources()[req.content.get_create_request().key()]);
                    } else {
                        assert(s.ongoing_reconciles()[key] == s_prime.ongoing_reconciles()[key]);
                        assert(!s.in_flight().contains(pending_req));
                    }
                }
            }
        }
    }
    leads_to_stable_temp(spec, lift_action(next), true_pred(), lift_state(inv));
}

pub proof fn lemma_eventually_always_object_in_response_at_after_update_resource_step_is_same_as_etcd_forall(
    spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_etcd_is_well_formed()))),
        spec.entails(always(lift_state(RMQCluster::key_of_object_in_matched_ok_update_resp_message_is_same_as_key_of_pending_req(rabbitmq.object_ref())))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_or_pending_req_msg_has_unique_id()))),
        spec.entails(always(tla_forall(|res: SubResource| lift_state(no_delete_resource_request_msg_in_flight(res, rabbitmq))))),
        spec.entails(always(tla_forall(|res: SubResource| lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(res, rabbitmq))))),
        spec.entails(true_pred().leads_to(lift_state(|s: RMQCluster| !s.ongoing_reconciles().contains_key(rabbitmq.object_ref())))),
        spec.entails(always(tla_forall(|res: SubResource| lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(res, rabbitmq))))),
        spec.entails(always(tla_forall(|res: SubResource| lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(res, rabbitmq))))),
    ensures
        spec.entails(true_pred().leads_to(always(lift_state(object_in_response_at_after_update_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq))))),
{
    always_tla_forall_apply(spec, |res: SubResource| lift_state(no_delete_resource_request_msg_in_flight(res, rabbitmq)), SubResource::ServerConfigMap);
    always_tla_forall_apply(spec, |res: SubResource| lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(res, rabbitmq)), SubResource::ServerConfigMap);
    always_tla_forall_apply(spec, |res: SubResource| lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(res, rabbitmq)), SubResource::ServerConfigMap);
    always_tla_forall_apply(spec, |res: SubResource| lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(res, rabbitmq)), SubResource::ServerConfigMap);
    lemma_eventually_always_object_in_response_at_after_update_resource_step_is_same_as_etcd(spec, rabbitmq);
}

#[verifier(spinoff_prover)]
pub proof fn lemma_eventually_always_object_in_response_at_after_update_resource_step_is_same_as_etcd(
    spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_etcd_is_well_formed()))),
        spec.entails(always(lift_state(RMQCluster::key_of_object_in_matched_ok_update_resp_message_is_same_as_key_of_pending_req(rabbitmq.object_ref())))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_or_pending_req_msg_has_unique_id()))),
        spec.entails(always(lift_state(no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(true_pred().leads_to(lift_state(|s: RMQCluster| !s.ongoing_reconciles().contains_key(rabbitmq.object_ref())))),
        spec.entails(always(lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(SubResource::ServerConfigMap, rabbitmq)))),
    ensures
        spec.entails(true_pred().leads_to(always(lift_state(object_in_response_at_after_update_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq))))),
{
    let key = rabbitmq.object_ref();
    let inv = object_in_response_at_after_update_resource_step_is_same_as_etcd(SubResource::ServerConfigMap, rabbitmq);
    let next = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()(s)
        &&& RMQCluster::every_in_flight_msg_has_unique_id()(s)
        &&& RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()(s)
        &&& RMQCluster::each_object_in_etcd_is_well_formed()(s_prime)
        &&& RMQCluster::key_of_object_in_matched_ok_update_resp_message_is_same_as_key_of_pending_req(key)(s_prime)
        &&& RMQCluster::every_in_flight_or_pending_req_msg_has_unique_id()(s)
        &&& no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(SubResource::ServerConfigMap, rabbitmq)(s)
    };
    always_to_always_later(spec, lift_state(RMQCluster::each_object_in_etcd_is_well_formed()));
    always_to_always_later(spec, lift_state(RMQCluster::key_of_object_in_matched_ok_update_resp_message_is_same_as_key_of_pending_req(key)));
    combine_spec_entails_always_n!(
        spec, lift_action(next), lift_action(RMQCluster::next()),
        lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()),
        lift_state(RMQCluster::every_in_flight_msg_has_unique_id()),
        lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()),
        later(lift_state(RMQCluster::each_object_in_etcd_is_well_formed())),
        later(lift_state(RMQCluster::key_of_object_in_matched_ok_update_resp_message_is_same_as_key_of_pending_req(key))),
        lift_state(RMQCluster::every_in_flight_or_pending_req_msg_has_unique_id()),
        lift_state(no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(SubResource::ServerConfigMap, rabbitmq))
    );
    leads_to_weaken_temp(
        spec, true_pred(), lift_state(|s: RMQCluster| !s.ongoing_reconciles().contains_key(rabbitmq.object_ref())),
        true_pred(), lift_state(inv)
    );
    let resource_key = get_request(SubResource::ServerConfigMap, rabbitmq).key;
    let key = rabbitmq.object_ref();
    assert forall |s: RMQCluster, s_prime: RMQCluster| inv(s) && #[trigger] next(s, s_prime) implies inv(s_prime) by {
        let pending_req = s_prime.ongoing_reconciles()[key].pending_req_msg.get_Some_0();
        if at_rabbitmq_step(key, RabbitmqReconcileStep::AfterKRequestStep(ActionKind::Update, SubResource::ServerConfigMap))(s_prime) {
            assert_by(
                s_prime.ongoing_reconciles()[key].pending_req_msg.is_Some()
                && resource_update_request_msg(resource_key)(s_prime.ongoing_reconciles()[key].pending_req_msg.get_Some_0()),
                {
                    let step = choose |step| RMQCluster::next_step(s, s_prime, step);
                    match step {
                        Step::ControllerStep(input) => {
                            let cr_key = input.1.get_Some_0();
                            if cr_key == key {
                                assert(s_prime.ongoing_reconciles()[key].pending_req_msg.is_Some());
                                assert(resource_update_request_msg(resource_key)(s_prime.ongoing_reconciles()[key].pending_req_msg.get_Some_0()));
                            } else {
                                assert(s_prime.ongoing_reconciles()[key] == s.ongoing_reconciles()[key]);
                            }
                        },
                        Step::RestartController() => {
                            assert(false);
                        },
                        _ => {
                            assert(s_prime.ongoing_reconciles()[key] == s.ongoing_reconciles()[key]);
                        }
                    }
                }
            );

            assert forall |msg: RMQMessage| #[trigger] s_prime.in_flight().contains(msg) && Message::resp_msg_matches_req_msg(msg, pending_req) implies resource_update_response_msg(resource_key, s_prime)(msg) by {
                assert(msg.src.is_KubernetesAPI());
                assert(msg.content.is_update_response());
                if msg.content.get_update_response().res.is_Ok() {
                    let step = choose |step| RMQCluster::next_step(s, s_prime, step);
                    if !s.in_flight().contains(msg) {
                        assert(step.is_KubernetesAPIStep());
                        let req = step.get_KubernetesAPIStep_0().get_Some_0();
                        assert(msg.content.get_update_response().res.get_Ok_0().object_ref() == req.content.get_update_request().key());
                        assert(msg.content.get_update_response().res.get_Ok_0().object_ref() == resource_key);
                        assert(msg.content.get_update_response().res.get_Ok_0() == s_prime.resources()[req.content.get_update_request().key()]);
                    } else {
                        assert(s.ongoing_reconciles()[key] == s_prime.ongoing_reconciles()[key]);
                        assert(!s.in_flight().contains(pending_req));
                    }
                }
            }
        }
    }
    leads_to_stable_temp(spec, lift_action(next), true_pred(), lift_state(inv));
}

#[verifier(spinoff_prover)]
pub proof fn lemma_always_response_at_after_get_resource_step_is_resource_get_response(
    spec: TempPred<RMQCluster>, sub_resource: SubResource, rabbitmq: RabbitmqClusterView
)
    requires
        spec.entails(lift_state(RMQCluster::init())),
        spec.entails(always(lift_action(RMQCluster::next()))),
    ensures
        spec.entails(always(lift_state(response_at_after_get_resource_step_is_resource_get_response(sub_resource, rabbitmq)))),
{
    let inv = response_at_after_get_resource_step_is_resource_get_response(sub_resource, rabbitmq);
    let key = rabbitmq.object_ref();
    let resource_key = get_request(sub_resource, rabbitmq).key;
    let next = |s, s_prime| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()(s)
        &&& RMQCluster::key_of_object_in_matched_ok_get_resp_message_is_same_as_key_of_pending_req(key)(s_prime)
    };
    RMQCluster::lemma_always_each_object_in_reconcile_has_consistent_key_and_valid_metadata(spec);
    RMQCluster::lemma_always_key_of_object_in_matched_ok_get_resp_message_is_same_as_key_of_pending_req(spec, key);
    always_to_always_later(spec, lift_state(RMQCluster::key_of_object_in_matched_ok_get_resp_message_is_same_as_key_of_pending_req(key)));
    combine_spec_entails_always_n!(
        spec, lift_action(next), lift_action(RMQCluster::next()),
        lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()),
        later(lift_state(RMQCluster::key_of_object_in_matched_ok_get_resp_message_is_same_as_key_of_pending_req(key)))
    );
    assert forall |s: RMQCluster, s_prime: RMQCluster| inv(s) && #[trigger] next(s, s_prime) implies inv(s_prime) by {
        if at_rabbitmq_step(key, RabbitmqReconcileStep::AfterKRequestStep(ActionKind::Get, sub_resource))(s_prime) {
            let step = choose |step| RMQCluster::next_step(s, s_prime, step);
            match step {
                Step::ControllerStep(input) => {
                    let cr_key = input.1.get_Some_0();
                    if cr_key == key {
                        assert(s_prime.ongoing_reconciles()[key].pending_req_msg.is_Some());
                        assert(resource_get_request_msg(resource_key)(s_prime.ongoing_reconciles()[key].pending_req_msg.get_Some_0()));
                    } else {
                        assert(s_prime.ongoing_reconciles()[key] == s.ongoing_reconciles()[key]);
                    }
                },
                Step::RestartController() => {
                    assert(false);
                },
                _ => {
                    assert(s_prime.ongoing_reconciles()[key] == s.ongoing_reconciles()[key]);
                }
            }
        }
    }
    init_invariant(spec, RMQCluster::init(), next, inv);
}

pub proof fn lemma_eventually_always_every_resource_update_request_implies_at_after_update_resource_step_forall(
    spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(tla_forall(|i| RMQCluster::kubernetes_api_next().weak_fairness(i))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::crash_disabled()))),
        spec.entails(always(lift_state(RMQCluster::busy_disabled()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq)))),
        spec.entails(always(lift_state(RMQCluster::object_in_ok_get_response_has_smaller_rv_than_etcd()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_etcd_is_well_formed()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_req_is_unique()))),
        spec.entails(always(tla_forall(|sub_resource: SubResource| lift_state(RMQCluster::object_in_ok_get_resp_is_same_as_etcd_with_same_rv(get_request(sub_resource, rabbitmq).key))))),
        spec.entails(always(tla_forall(|sub_resource: SubResource| lift_state(response_at_after_get_resource_step_is_resource_get_response(sub_resource, rabbitmq))))),
        spec.entails(always(tla_forall(|sub_resource: SubResource| lift_state(no_delete_resource_request_msg_in_flight(sub_resource, rabbitmq))))),
        spec.entails(always(tla_forall(|sub_resource: SubResource| lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(sub_resource, rabbitmq))))),
        spec.entails(always(tla_forall(|sub_resource: SubResource| lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq))))),
        spec.entails(always(tla_forall(|sub_resource: SubResource| lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq))))),
    ensures
        spec.entails(
            true_pred().leads_to(always(tla_forall(|sub_resource: SubResource| lift_state(every_resource_update_request_implies_at_after_update_resource_step(sub_resource, rabbitmq)))))
        ),
{
    assert forall |sub_resource: SubResource| spec.entails(true_pred().leads_to(always(lift_state(#[trigger] every_resource_update_request_implies_at_after_update_resource_step(sub_resource, rabbitmq))))) by {
        always_tla_forall_apply(spec, |res: SubResource| lift_state(RMQCluster::object_in_ok_get_resp_is_same_as_etcd_with_same_rv(get_request(res, rabbitmq).key)), sub_resource);
        always_tla_forall_apply(spec, |res: SubResource|lift_state(response_at_after_get_resource_step_is_resource_get_response(res, rabbitmq)), sub_resource);
        always_tla_forall_apply(spec, |res: SubResource|lift_state(no_delete_resource_request_msg_in_flight(res, rabbitmq)), sub_resource);
        always_tla_forall_apply(spec, |res: SubResource|lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(res, rabbitmq)), sub_resource);
        always_tla_forall_apply(spec, |res: SubResource|lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(res, rabbitmq)), sub_resource);
        always_tla_forall_apply(spec, |res: SubResource|lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(res, rabbitmq)), sub_resource);
        lemma_eventually_always_every_resource_update_request_implies_at_after_update_resource_step(spec, sub_resource, rabbitmq);
    }
    leads_to_always_tla_forall_subresource(spec, true_pred(), |sub_resource: SubResource| lift_state(every_resource_update_request_implies_at_after_update_resource_step(sub_resource, rabbitmq)));
}

#[verifier(spinoff_prover)]
pub proof fn lemma_eventually_always_every_resource_update_request_implies_at_after_update_resource_step(
    spec: TempPred<RMQCluster>, sub_resource: SubResource, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(tla_forall(|i| RMQCluster::kubernetes_api_next().weak_fairness(i))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::crash_disabled()))),
        spec.entails(always(lift_state(RMQCluster::busy_disabled()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq)))),
        spec.entails(always(lift_state(RMQCluster::object_in_ok_get_response_has_smaller_rv_than_etcd()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_etcd_is_well_formed()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_req_is_unique()))),
        spec.entails(always(lift_state(RMQCluster::object_in_ok_get_resp_is_same_as_etcd_with_same_rv(get_request(sub_resource, rabbitmq).key)))),
        spec.entails(always(lift_state(response_at_after_get_resource_step_is_resource_get_response(sub_resource, rabbitmq)))),
        spec.entails(always(lift_state(no_delete_resource_request_msg_in_flight(sub_resource, rabbitmq)))),
        spec.entails(always(lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(sub_resource, rabbitmq)))),
        spec.entails(always(lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq)))),
        spec.entails(always(lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq)))),
    ensures
        spec.entails(
            true_pred().leads_to(always(lift_state(every_resource_update_request_implies_at_after_update_resource_step(sub_resource, rabbitmq))))
        ),
{
    let key = rabbitmq.object_ref();
    let resource_key = get_request(sub_resource, rabbitmq).key;
    let requirements = |msg: RMQMessage, s: RMQCluster| {
        resource_update_request_msg(resource_key)(msg) ==> {
            &&& at_rabbitmq_step(key, RabbitmqReconcileStep::AfterKRequestStep(ActionKind::Update, sub_resource))(s)
            &&& RMQCluster::pending_k8s_api_req_msg_is(s, key, msg)
            &&& msg.content.get_update_request().obj.metadata.resource_version.is_Some()
            &&& msg.content.get_update_request().obj.metadata.resource_version.get_Some_0() < s.kubernetes_api_state.resource_version_counter
            &&& (
                s.resources().contains_key(resource_key)
                && msg.content.get_update_request().obj.metadata.resource_version == s.resources()[resource_key].metadata.resource_version
            ) ==> (
                update(sub_resource, rabbitmq, s.ongoing_reconciles()[key].local_state, s.resources()[resource_key]).is_Ok()
                && msg.content.get_update_request().obj == update(sub_resource, rabbitmq, s.ongoing_reconciles()[key].local_state, s.resources()[resource_key]).get_Ok_0()
            )
        }
    };
    let stronger_next = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::crash_disabled()(s)
        &&& RMQCluster::busy_disabled()(s)
        &&& RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()(s)
        &&& RMQCluster::every_in_flight_msg_has_unique_id()(s)
        &&& RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq)(s)
        &&& RMQCluster::object_in_ok_get_response_has_smaller_rv_than_etcd()(s)
        &&& RMQCluster::each_object_in_etcd_is_well_formed()(s)
        &&& RMQCluster::each_object_in_etcd_is_well_formed()(s_prime)
        &&& RMQCluster::every_in_flight_req_is_unique()(s)
        &&& RMQCluster::object_in_ok_get_resp_is_same_as_etcd_with_same_rv(get_request(sub_resource, rabbitmq).key)(s)
        &&& response_at_after_get_resource_step_is_resource_get_response(sub_resource, rabbitmq)(s)
        &&& no_delete_resource_request_msg_in_flight(sub_resource, rabbitmq)(s)
        &&& no_update_status_request_msg_in_flight_of_except_stateful_set(sub_resource, rabbitmq)(s)
        &&& object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq)(s)
        &&& resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq)(s)
    };
    assert forall |s, s_prime| #[trigger] stronger_next(s, s_prime)
    implies RMQCluster::every_new_req_msg_if_in_flight_then_satisfies(requirements)(s, s_prime) by {
        assert forall |msg: RMQMessage| (!s.in_flight().contains(msg) || requirements(msg, s)) && #[trigger] s_prime.in_flight().contains(msg)
        implies requirements(msg, s_prime) by {
            if resource_update_request_msg(resource_key)(msg) {
                let step = choose |step| RMQCluster::next_step(s, s_prime, step);
                if !s.in_flight().contains(msg) {
                    lemma_resource_create_or_update_request_msg_implies_key_in_reconcile_equals(sub_resource, rabbitmq, s, s_prime, msg, step);
                    let resp = step.get_ControllerStep_0().0.get_Some_0();
                    assert(RMQCluster::is_ok_get_response_msg()(resp));
                    assert(s.in_flight().contains(resp));
                    assert(resp.content.get_get_response().res.get_Ok_0().metadata.resource_version == msg.content.get_update_request().obj.metadata.resource_version);
                    if s.resources().contains_key(resource_key) && resp.content.get_get_response().res.get_Ok_0().metadata.resource_version == s.resources()[resource_key].metadata.resource_version {
                        assert(resp.content.get_get_response().res.get_Ok_0() == s.resources()[resource_key]);
                        assert(s_prime.resources()[resource_key] == s.resources()[resource_key]);
                    }
                    if sub_resource == SubResource::StatefulSet {
                        let cm_key = get_request(SubResource::ServerConfigMap, rabbitmq).key;
                        assert(s.resources()[cm_key] == s_prime.resources()[cm_key]);
                        assert(s.ongoing_reconciles()[key].local_state.latest_config_map_rv_opt == s_prime.ongoing_reconciles()[key].local_state.latest_config_map_rv_opt)
                    }
                } else {
                    assert(requirements(msg, s));
                    assert(s.ongoing_reconciles()[key] == s_prime.ongoing_reconciles()[key]);
                }
            }
        }
    }
    always_to_always_later(spec, lift_state(RMQCluster::each_object_in_etcd_is_well_formed()));
    invariant_n!(
        spec, lift_action(stronger_next), lift_action(RMQCluster::every_new_req_msg_if_in_flight_then_satisfies(requirements)),
        lift_action(RMQCluster::next()), lift_state(RMQCluster::crash_disabled()), lift_state(RMQCluster::busy_disabled()),
        lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()),
        lift_state(RMQCluster::every_in_flight_msg_has_unique_id()),
        lift_state(RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq)),
        lift_state(RMQCluster::object_in_ok_get_response_has_smaller_rv_than_etcd()),
        lift_state(RMQCluster::each_object_in_etcd_is_well_formed()),
        later(lift_state(RMQCluster::each_object_in_etcd_is_well_formed())),
        lift_state(RMQCluster::every_in_flight_req_is_unique()),
        lift_state(RMQCluster::object_in_ok_get_resp_is_same_as_etcd_with_same_rv(get_request(sub_resource, rabbitmq).key)),
        lift_state(response_at_after_get_resource_step_is_resource_get_response(sub_resource, rabbitmq)),
        lift_state(no_delete_resource_request_msg_in_flight(sub_resource, rabbitmq)),
        lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(sub_resource, rabbitmq)),
        lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq)),
        lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq))
    );

    RMQCluster::lemma_true_leads_to_always_every_in_flight_req_msg_satisfies(spec, requirements);

    temp_pred_equality(
        lift_state(every_resource_update_request_implies_at_after_update_resource_step(sub_resource, rabbitmq)),
        lift_state(RMQCluster::every_in_flight_req_msg_satisfies(requirements)));
}

pub proof fn lemma_eventually_always_object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr_forall(
    spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(tla_forall(|i| RMQCluster::kubernetes_api_next().weak_fairness(i))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::crash_disabled()))),
        spec.entails(always(lift_state(RMQCluster::busy_disabled()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq)))),
    ensures
        spec.entails(
            true_pred().leads_to(
                always(tla_forall(|sub_resource: SubResource| lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq))))
        )),
{
    assert forall |sub_resource: SubResource| spec.entails(true_pred().leads_to(always(lift_state(#[trigger] object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq))))) by {
        lemma_eventually_always_object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(spec, sub_resource, rabbitmq);
    }
    leads_to_always_tla_forall_subresource(spec, true_pred(), |sub_resource: SubResource| lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq)));
}

#[verifier(spinoff_prover)]
pub proof fn lemma_eventually_always_object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(
    spec: TempPred<RMQCluster>, sub_resource: SubResource, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(tla_forall(|i| RMQCluster::kubernetes_api_next().weak_fairness(i))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::crash_disabled()))),
        spec.entails(always(lift_state(RMQCluster::busy_disabled()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq)))),
    ensures
        spec.entails(
            true_pred().leads_to(always(lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq))))
        ),
{
    let key = rabbitmq.object_ref();
    let resource_key = get_request(sub_resource, rabbitmq).key;
    let requirements = |msg: RMQMessage, s: RMQCluster| {
        resource_update_request_msg(resource_key)(msg) ==> {
            &&& at_rabbitmq_step(key, RabbitmqReconcileStep::AfterKRequestStep(ActionKind::Update, sub_resource))(s)
            &&& RMQCluster::pending_k8s_api_req_msg_is(s, key, msg)
            &&& msg.content.get_update_request().obj.metadata.owner_references_only_contains(rabbitmq.controller_owner_ref())
        }
    };
    let stronger_next = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::crash_disabled()(s)
        &&& RMQCluster::busy_disabled()(s)
        &&& RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()(s)
        &&& RMQCluster::every_in_flight_msg_has_unique_id()(s)
        &&& RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq)(s)
    };
    assert forall |s, s_prime| #[trigger] stronger_next(s, s_prime)
    implies RMQCluster::every_new_req_msg_if_in_flight_then_satisfies(requirements)(s, s_prime) by {
        assert forall |msg: RMQMessage| (!s.in_flight().contains(msg) || requirements(msg, s)) && #[trigger] s_prime.in_flight().contains(msg)
        implies requirements(msg, s_prime) by {
            if resource_update_request_msg(resource_key)(msg) {
                let step = choose |step| RMQCluster::next_step(s, s_prime, step);
                if !s.in_flight().contains(msg) {
                    lemma_resource_create_or_update_request_msg_implies_key_in_reconcile_equals(sub_resource, rabbitmq, s, s_prime, msg, step);
                } else {
                    assert(requirements(msg, s));
                    assert(s.ongoing_reconciles()[key] == s_prime.ongoing_reconciles()[key]);
                }
            }
        }
    }
    invariant_n!(
        spec, lift_action(stronger_next), lift_action(RMQCluster::every_new_req_msg_if_in_flight_then_satisfies(requirements)),
        lift_action(RMQCluster::next()), lift_state(RMQCluster::crash_disabled()), lift_state(RMQCluster::busy_disabled()),
        lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()),
        lift_state(RMQCluster::every_in_flight_msg_has_unique_id()),
        lift_state(RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq))
    );

    RMQCluster::lemma_true_leads_to_always_every_in_flight_req_msg_satisfies(spec, requirements);

    temp_pred_equality(
        lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq)),
        lift_state(RMQCluster::every_in_flight_req_msg_satisfies(requirements)));
}

pub proof fn lemma_eventually_always_every_resource_create_request_implies_at_after_create_resource_step_forall(
    spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(tla_forall(|i| RMQCluster::kubernetes_api_next().weak_fairness(i))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::crash_disabled()))),
        spec.entails(always(lift_state(RMQCluster::busy_disabled()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq)))),
    ensures
        spec.entails(
            true_pred().leads_to(
                always(tla_forall(|sub_resource: SubResource| lift_state(every_resource_create_request_implies_at_after_create_resource_step(sub_resource, rabbitmq)))))
        ),
{
    assert forall |sub_resource: SubResource| spec.entails(true_pred().leads_to(always(lift_state(#[trigger] every_resource_create_request_implies_at_after_create_resource_step(sub_resource, rabbitmq))))) by {
        lemma_eventually_always_every_resource_create_request_implies_at_after_create_resource_step(spec, sub_resource, rabbitmq);
    }
    leads_to_always_tla_forall_subresource(spec, true_pred(), |sub_resource: SubResource| lift_state(every_resource_create_request_implies_at_after_create_resource_step(sub_resource, rabbitmq)));
}

#[verifier(spinoff_prover)]
pub proof fn lemma_eventually_always_every_resource_create_request_implies_at_after_create_resource_step(
    spec: TempPred<RMQCluster>, sub_resource: SubResource, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(tla_forall(|i| RMQCluster::kubernetes_api_next().weak_fairness(i))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::crash_disabled()))),
        spec.entails(always(lift_state(RMQCluster::busy_disabled()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_unique_id()))),
        spec.entails(always(lift_state(RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq)))),
    ensures
        spec.entails(
            true_pred().leads_to(always(lift_state(every_resource_create_request_implies_at_after_create_resource_step(sub_resource, rabbitmq))))
        ),
{
    let key = rabbitmq.object_ref();
    let resource_key = get_request(sub_resource, rabbitmq).key;
    let requirements = |msg: RMQMessage, s: RMQCluster| {
        resource_create_request_msg(resource_key)(msg) ==> {
            &&& at_rabbitmq_step(key, RabbitmqReconcileStep::AfterKRequestStep(ActionKind::Create, sub_resource))(s)
            &&& RMQCluster::pending_k8s_api_req_msg_is(s, key, msg)
            &&& make(sub_resource, rabbitmq, s.ongoing_reconciles()[key].local_state).is_Ok()
            &&& msg.content.get_create_request().obj == make(sub_resource, rabbitmq, s.ongoing_reconciles()[key].local_state).get_Ok_0()
        }
    };
    let stronger_next = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::crash_disabled()(s)
        &&& RMQCluster::busy_disabled()(s)
        &&& RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()(s)
        &&& RMQCluster::every_in_flight_msg_has_unique_id()(s)
        &&& RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq)(s)
    };
    assert forall |s, s_prime| #[trigger] stronger_next(s, s_prime)
    implies RMQCluster::every_new_req_msg_if_in_flight_then_satisfies(requirements)(s, s_prime) by {
        assert forall |msg: RMQMessage| (!s.in_flight().contains(msg) || requirements(msg, s)) && #[trigger] s_prime.in_flight().contains(msg)
        implies requirements(msg, s_prime) by {
            if resource_create_request_msg(resource_key)(msg) {
                let step = choose |step| RMQCluster::next_step(s, s_prime, step);
                if !s.in_flight().contains(msg) {
                    lemma_resource_create_or_update_request_msg_implies_key_in_reconcile_equals(sub_resource, rabbitmq, s, s_prime, msg, step);
                } else {
                    assert(requirements(msg, s));
                    assert(s.ongoing_reconciles()[key] == s_prime.ongoing_reconciles()[key]);
                }
            }
        }
    }
    invariant_n!(
        spec, lift_action(stronger_next), lift_action(RMQCluster::every_new_req_msg_if_in_flight_then_satisfies(requirements)),
        lift_action(RMQCluster::next()), lift_state(RMQCluster::crash_disabled()), lift_state(RMQCluster::busy_disabled()),
        lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()),
        lift_state(RMQCluster::every_in_flight_msg_has_unique_id()),
        lift_state(RMQCluster::the_object_in_reconcile_has_spec_and_uid_as(rabbitmq))
    );

    RMQCluster::lemma_true_leads_to_always_every_in_flight_req_msg_satisfies(spec, requirements);

    temp_pred_equality(
        lift_state(every_resource_create_request_implies_at_after_create_resource_step(sub_resource, rabbitmq)),
        lift_state(RMQCluster::every_in_flight_req_msg_satisfies(requirements)));
}

#[verifier(spinoff_prover)]
pub proof fn lemma_always_no_update_status_request_msg_in_flight_of_except_stateful_set(
    spec: TempPred<RMQCluster>, sub_resource: SubResource, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(lift_state(RMQCluster::init())),
        spec.entails(always(lift_action(RMQCluster::next()))),
    ensures
        spec.entails(always(lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(sub_resource, rabbitmq)))),
{
    RMQCluster::lemma_always_each_object_in_etcd_is_well_formed(spec);
    let inv = no_update_status_request_msg_in_flight_of_except_stateful_set(sub_resource, rabbitmq);
    let stronger_next = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::each_object_in_etcd_is_well_formed()(s)
    };
    combine_spec_entails_always_n!(
        spec, lift_action(stronger_next),
        lift_action(RMQCluster::next()),
        lift_state(RMQCluster::each_object_in_etcd_is_well_formed())
    );

    let resource_key = get_request(sub_resource, rabbitmq).key;
    assert forall |s, s_prime: RMQCluster| inv(s) && #[trigger] stronger_next(s, s_prime) implies inv(s_prime) by {
        if sub_resource != SubResource::StatefulSet {
            assert forall |msg: RMQMessage| #[trigger] s_prime.in_flight().contains(msg) && msg.content.is_update_status_request()
            implies msg.content.get_update_status_request().key() != resource_key by {
                if s.in_flight().contains(msg) {
                    assert(msg.content.get_update_status_request().key() != resource_key);
                } else {
                    let step = choose |step: RMQStep| RMQCluster::next_step(s, s_prime, step);
                    match step {
                        Step::ControllerStep(input) => {
                            if input.1.is_Some() {
                                let cr_key = input.1.get_Some_0();
                                if s.ongoing_reconciles().contains_key(cr_key) {
                                    match s.ongoing_reconciles()[cr_key].local_state.reconcile_step {
                                        RabbitmqReconcileStep::Init => {},
                                        RabbitmqReconcileStep::AfterKRequestStep(_, resource) => {
                                            match resource {
                                                SubResource::HeadlessService => {},
                                                SubResource::Service => {},
                                                SubResource::ErlangCookieSecret => {},
                                                SubResource::DefaultUserSecret => {},
                                                SubResource::PluginsConfigMap => {},
                                                SubResource::ServerConfigMap => {},
                                                SubResource::ServiceAccount => {},
                                                SubResource::Role => {},
                                                SubResource::RoleBinding => {},
                                                SubResource::StatefulSet => {},
                                            }
                                        },
                                        _ => {}
                                    }
                                } else {}
                            } else {}
                            assert(!msg.content.is_update_status_request());
                            assert(false);
                        },
                        Step::KubernetesAPIStep(_) => {
                            assert(!msg.content.is_APIRequest());
                            assert(!msg.content.is_update_status_request());
                            assert(false);
                        },
                        Step::ClientStep() => {
                            assert(!msg.content.is_update_status_request());
                            assert(false);
                        },
                        Step::BuiltinControllersStep(_) => {
                            assert(msg.content.get_update_status_request().key().kind == Kind::StatefulSetKind
                                || msg.content.get_update_status_request().key().kind == Kind::DaemonSetKind);
                            assert(msg.content.get_update_status_request().key() != resource_key);
                        },
                        Step::FailTransientlyStep(_) => {
                            assert(!msg.content.is_APIRequest());
                            assert(!msg.content.is_update_status_request());
                            assert(false);
                        },
                        _ => {
                            assert(!s_prime.in_flight().contains(msg));
                            assert(false);
                        }
                    }
                }
            }
        }
    }
    init_invariant(spec, RMQCluster::init(), stronger_next, inv);
}

#[verifier(spinoff_prover)]
pub proof fn lemma_always_no_update_status_request_msg_not_from_bc_in_flight_of_stateful_set(
    spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(lift_state(RMQCluster::init())),
        spec.entails(always(lift_action(RMQCluster::next()))),
    ensures
        spec.entails(always(lift_state(no_update_status_request_msg_not_from_bc_in_flight_of_stateful_set(rabbitmq)))),
{
    RMQCluster::lemma_always_each_object_in_etcd_is_well_formed(spec);
    let inv = no_update_status_request_msg_not_from_bc_in_flight_of_stateful_set(rabbitmq);
    let stronger_next = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::each_object_in_etcd_is_well_formed()(s)
    };
    combine_spec_entails_always_n!(
        spec, lift_action(stronger_next),
        lift_action(RMQCluster::next()),
        lift_state(RMQCluster::each_object_in_etcd_is_well_formed())
    );

    let resource_key = get_request(SubResource::StatefulSet, rabbitmq).key;
    assert forall |s, s_prime: RMQCluster| inv(s) && #[trigger] stronger_next(s, s_prime) implies inv(s_prime) by {
        assert forall |msg: RMQMessage| #[trigger] s_prime.in_flight().contains(msg) && msg.dst.is_KubernetesAPI() && !msg.src.is_BuiltinController() && msg.content.is_update_status_request()
        implies msg.content.get_update_status_request().key() != resource_key by {
            if s.in_flight().contains(msg) {
                assert(msg.content.get_update_status_request().key() != resource_key);
            } else {
                let step = choose |step: RMQStep| RMQCluster::next_step(s, s_prime, step);
                match step {
                    Step::ControllerStep(input) => {
                        if input.1.is_Some() {
                            let cr_key = input.1.get_Some_0();
                            if s.ongoing_reconciles().contains_key(cr_key) {
                                match s.ongoing_reconciles()[cr_key].local_state.reconcile_step {
                                    RabbitmqReconcileStep::Init => {},
                                    RabbitmqReconcileStep::AfterKRequestStep(_, resource) => {
                                        match resource {
                                            SubResource::HeadlessService => {},
                                            SubResource::Service => {},
                                            SubResource::ErlangCookieSecret => {},
                                            SubResource::DefaultUserSecret => {},
                                            SubResource::PluginsConfigMap => {},
                                            SubResource::ServerConfigMap => {},
                                            SubResource::ServiceAccount => {},
                                            SubResource::Role => {},
                                            SubResource::RoleBinding => {},
                                            SubResource::StatefulSet => {},
                                        }
                                    },
                                    _ => {}
                                }
                            } else {}
                        } else {}
                        assert(!msg.content.is_update_status_request());
                        assert(false);
                    },
                    Step::KubernetesAPIStep(_) => {
                        assert(!msg.content.is_APIRequest());
                        assert(!msg.content.is_update_status_request());
                        assert(false);
                    },
                    Step::ClientStep() => {
                        assert(!msg.content.is_update_status_request());
                        assert(false);
                    },
                    Step::BuiltinControllersStep(_) => {
                        assert(msg.src.is_BuiltinController());
                        assert(false);
                    },
                    Step::FailTransientlyStep(_) => {
                        assert(!msg.content.is_APIRequest());
                        assert(!msg.content.is_update_status_request());
                        assert(false);
                    },
                    _ => {
                        assert(!s_prime.in_flight().contains(msg));
                        assert(false);
                    }
                }
            }
        }
    }
    init_invariant(spec, RMQCluster::init(), stronger_next, inv);
}

spec fn make_owner_references_with_name_and_uid(name: StringView, uid: Uid) -> OwnerReferenceView {
    OwnerReferenceView {
        block_owner_deletion: None,
        controller: Some(true),
        kind: RabbitmqClusterView::kind(),
        name: name,
        uid: uid,
    }
}

pub proof fn lemma_always_resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(
    spec: TempPred<RMQCluster>, sub_resource: SubResource, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(lift_state(RMQCluster::init())),
        spec.entails(always(lift_action(RMQCluster::next()))),
    ensures
        spec.entails(always(lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(sub_resource, rabbitmq)))),
{
    let inv = resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(sub_resource, rabbitmq);
    lemma_always_resource_object_create_or_update_request_msg_has_one_controller_ref_and_no_finalizers(spec, sub_resource, rabbitmq);
    let stronger_next = |s, s_prime| {
        &&& RMQCluster::next()(s, s_prime)
        &&& resource_object_create_or_update_request_msg_has_one_controller_ref_and_no_finalizers(sub_resource, rabbitmq)(s)
    };
    combine_spec_entails_always_n!(
        spec, lift_action(stronger_next),
        lift_action(RMQCluster::next()),
        lift_state(resource_object_create_or_update_request_msg_has_one_controller_ref_and_no_finalizers(sub_resource, rabbitmq))
    );
    init_invariant(spec, RMQCluster::init(), stronger_next, inv);
}

spec fn resource_object_create_or_update_request_msg_has_one_controller_ref_and_no_finalizers(
    sub_resource: SubResource, rabbitmq: RabbitmqClusterView
) -> StatePred<RMQCluster> {
    |s: RMQCluster| {
        let key = rabbitmq.object_ref();
        let resource_key = get_request(sub_resource, rabbitmq).key;
        forall |msg: RMQMessage| {
            #[trigger] s.in_flight().contains(msg) ==> {
                &&& resource_update_request_msg(resource_key)(msg)
                    ==> msg.content.get_update_request().obj.metadata.finalizers.is_None()
                        && exists |uid: Uid| #![auto]
                            msg.content.get_update_request().obj.metadata.owner_references == Some(seq![
                                make_owner_references_with_name_and_uid(key.name, uid)
                            ])
                &&& resource_create_request_msg(resource_key)(msg)
                    ==> msg.content.get_create_request().obj.metadata.finalizers.is_None()
                        && exists |uid: Uid| #![auto]
                            msg.content.get_create_request().obj.metadata.owner_references == Some(seq![
                                make_owner_references_with_name_and_uid(key.name, uid)
                            ])
            }
        }
    }
}

#[verifier(spinoff_prover)]
proof fn lemma_always_resource_object_create_or_update_request_msg_has_one_controller_ref_and_no_finalizers(
    spec: TempPred<RMQCluster>, sub_resource: SubResource, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(lift_state(RMQCluster::init())),
        spec.entails(always(lift_action(RMQCluster::next()))),
    ensures
        spec.entails(always(lift_state(resource_object_create_or_update_request_msg_has_one_controller_ref_and_no_finalizers(sub_resource, rabbitmq)))),
{
    let inv = resource_object_create_or_update_request_msg_has_one_controller_ref_and_no_finalizers(sub_resource, rabbitmq);
    let stronger_next = |s, s_prime| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()(s)
    };
    let key = rabbitmq.object_ref();
    let resource_key = get_request(sub_resource, rabbitmq).key;
    RMQCluster::lemma_always_each_object_in_reconcile_has_consistent_key_and_valid_metadata(spec);
    combine_spec_entails_always_n!(
        spec, lift_action(stronger_next),
        lift_action(RMQCluster::next()),
        lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata())
    );
    let create_msg_pred = |msg: RMQMessage| {
        resource_create_request_msg(resource_key)(msg)
        ==> msg.content.get_create_request().obj.metadata.finalizers.is_None()
            && exists |uid: Uid| #![auto]
                msg.content.get_create_request().obj.metadata.owner_references == Some(seq![
                    make_owner_references_with_name_and_uid(key.name, uid)
                ])
    };
    let update_msg_pred = |msg: RMQMessage| {
        resource_update_request_msg(resource_key)(msg)
        ==> msg.content.get_update_request().obj.metadata.finalizers.is_None()
            && exists |uid: Uid| #![auto]
                msg.content.get_update_request().obj.metadata.owner_references == Some(seq![
                    make_owner_references_with_name_and_uid(key.name, uid)
                ])
    };
    assert forall |s, s_prime| inv(s) && #[trigger] stronger_next(s, s_prime) implies inv(s_prime) by {
        assert forall |msg| #[trigger] s_prime.in_flight().contains(msg) implies update_msg_pred(msg) && create_msg_pred(msg) by {
            if !s.in_flight().contains(msg) {
                let step = choose |step| RMQCluster::next_step(s, s_prime, step);
                lemma_resource_create_or_update_request_msg_implies_key_in_reconcile_equals(sub_resource, rabbitmq, s, s_prime, msg, step);
                let cr = s.ongoing_reconciles()[key].triggering_cr;
                if resource_create_request_msg(resource_key)(msg) {
                    assert(msg.content.get_create_request().obj == make(sub_resource, cr, s.ongoing_reconciles()[key].local_state).get_Ok_0());
                    assert(msg.content.get_create_request().obj.metadata.finalizers.is_None());
                    assert(msg.content.get_create_request().obj.metadata.owner_references == Some(seq![
                        make_owner_references_with_name_and_uid(key.name, cr.metadata.uid.get_Some_0())
                    ]));
                }
                if resource_update_request_msg(resource_key)(msg) {
                    assert(step.get_ControllerStep_0().0.get_Some_0().content.is_get_response());
                    assert(step.get_ControllerStep_0().0.get_Some_0().content.get_get_response().res.is_Ok());
                    assert(update(
                        sub_resource, cr, s.ongoing_reconciles()[key].local_state, step.get_ControllerStep_0().0.get_Some_0().content.get_get_response().res.get_Ok_0()
                    ).is_Ok());
                    assert(msg.content.get_update_request().obj == update(
                        sub_resource, cr, s.ongoing_reconciles()[key].local_state, step.get_ControllerStep_0().0.get_Some_0().content.get_get_response().res.get_Ok_0()
                    ).get_Ok_0());
                    assert(msg.content.get_update_request().obj.metadata.owner_references == Some(seq![
                        make_owner_references_with_name_and_uid(key.name, cr.metadata.uid.get_Some_0())
                    ]));
                }

            }
        }
    }
    init_invariant(spec, RMQCluster::init(), stronger_next, inv);
}

/// This lemma is used to show that if an action (which transfers the state from s to s_prime) creates a sub resource object
/// create/update request message (with key as key), it must be a controller action, and the triggering cr is s.ongoing_reconciles()[key].triggering_cr.
///
/// After the action, the controller stays at After(Create/Update, SubResource) step.
///
/// Tips: Talking about both s and s_prime give more information to those using this lemma and also makes the verification faster.
#[verifier(spinoff_prover)]
pub proof fn lemma_resource_create_or_update_request_msg_implies_key_in_reconcile_equals(
    sub_resource: SubResource, rabbitmq: RabbitmqClusterView, s: RMQCluster, s_prime: RMQCluster, msg: RMQMessage, step: RMQStep
)
    requires
        rabbitmq.well_formed(),
        !s.in_flight().contains(msg), s_prime.in_flight().contains(msg),
        RMQCluster::next_step(s, s_prime, step),
        RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()(s),
    ensures
        resource_create_request_msg(get_request(sub_resource, rabbitmq).key)(msg)
        ==> step.is_ControllerStep() && step.get_ControllerStep_0().1.get_Some_0() == rabbitmq.object_ref()
            && at_rabbitmq_step(rabbitmq.object_ref(), RabbitmqReconcileStep::AfterKRequestStep(ActionKind::Get, sub_resource))(s)
            && at_rabbitmq_step(rabbitmq.object_ref(), RabbitmqReconcileStep::AfterKRequestStep(ActionKind::Create, sub_resource))(s_prime)
            && RMQCluster::pending_k8s_api_req_msg_is(s_prime, rabbitmq.object_ref(), msg),
        resource_update_request_msg(get_request(sub_resource, rabbitmq).key)(msg)
        ==> step.is_ControllerStep() && step.get_ControllerStep_0().1.get_Some_0() == rabbitmq.object_ref()
            && at_rabbitmq_step(rabbitmq.object_ref(), RabbitmqReconcileStep::AfterKRequestStep(ActionKind::Get, sub_resource))(s)
            && at_rabbitmq_step(rabbitmq.object_ref(), RabbitmqReconcileStep::AfterKRequestStep(ActionKind::Update, sub_resource))(s_prime)
            && RMQCluster::pending_k8s_api_req_msg_is(s_prime, rabbitmq.object_ref(), msg),
{
    // Since we know that this step creates a create server config map message, it is easy to see that it's a controller action.
    // This action creates a config map, and there are two kinds of config maps, we have to show that only server config map
    // is possible by extra reasoning about the strings.
    let cr_key = step.get_ControllerStep_0().1.get_Some_0();
    let key = rabbitmq.object_ref();
    let cr = s.ongoing_reconciles()[key].triggering_cr;
    let resource_key = get_request(sub_resource, rabbitmq).key;
    if resource_create_request_msg(get_request(sub_resource, rabbitmq).key)(msg) || resource_update_request_msg(get_request(sub_resource, rabbitmq).key)(msg) {
        assert(step.is_ControllerStep());
        assert(s.ongoing_reconciles().contains_key(cr_key));
        let local_step = s.ongoing_reconciles()[cr_key].local_state.reconcile_step;
        let local_step_prime = s_prime.ongoing_reconciles()[cr_key].local_state.reconcile_step;
        assert(local_step_prime.is_AfterKRequestStep());
        assert(local_step.is_AfterKRequestStep() && local_step.get_AfterKRequestStep_0() == ActionKind::Get);
        if resource_create_request_msg(get_request(sub_resource, rabbitmq).key)(msg) {
            assert(local_step_prime.get_AfterKRequestStep_0() == ActionKind::Create);
        }
        if resource_update_request_msg(get_request(sub_resource, rabbitmq).key)(msg) {
            assert(local_step_prime.get_AfterKRequestStep_0() == ActionKind::Update);
        }
        assert_by(
            cr_key == rabbitmq.object_ref() && local_step.get_AfterKRequestStep_1() == sub_resource && RMQCluster::pending_k8s_api_req_msg_is(s_prime, cr_key, msg),
            {
                // It's easy for the verifier to know that cr_key has the same kind and namespace as key.
                match sub_resource {
                    SubResource::ServerConfigMap => {
                        // resource_create_request_msg(key)(msg) requires the msg has a key with name key.name "-server-conf". So we
                        // first show that in this action, cr_key is only possible to add "-server-conf" rather than "-plugins-conf" to reach
                        // such a post state.
                        assert_by(
                            cr_key.name + new_strlit("-plugins-conf")@ != key.name + new_strlit("-server-conf")@,
                            {
                                let str1 = cr_key.name + new_strlit("-plugins-conf")@;
                                let str2 = key.name + new_strlit("-server-conf")@;
                                reveal_strlit("-server-conf");
                                reveal_strlit("-plugins-conf");
                                if str1.len() == str2.len() {
                                    assert(str1[str1.len() - 6] == 's');
                                    assert(str2[str1.len() - 6] == 'r');
                                }
                            }
                        );
                        // Then we show that only if cr_key.name equals key.name, can this message be created in this step.
                        seq_lib::seq_equal_preserved_by_add(key.name, cr_key.name, new_strlit("-server-conf")@);
                    },
                    SubResource::PluginsConfigMap => {
                        assert_by(
                            key.name + new_strlit("-plugins-conf")@ != cr_key.name + new_strlit("-server-conf")@,
                            {
                                let str1 = key.name + new_strlit("-plugins-conf")@;
                                let str2 = cr_key.name + new_strlit("-server-conf")@;
                                reveal_strlit("-server-conf");
                                reveal_strlit("-plugins-conf");
                                if str1.len() == str2.len() {
                                    assert(str1[str1.len() - 6] == 's');
                                    assert(str2[str1.len() - 6] == 'r');
                                }
                            }
                        );
                        seq_lib::seq_equal_preserved_by_add(key.name, cr_key.name, new_strlit("-plugins-conf")@);
                    },
                    SubResource::ErlangCookieSecret => {
                        assert_by(
                            cr_key.name + new_strlit("-default-user")@ != key.name + new_strlit("-erlang-cookie")@,
                            {
                                let str1 = cr_key.name + new_strlit("-default-user")@;
                                let str2 = key.name + new_strlit("-erlang-cookie")@;
                                reveal_strlit("-erlang-cookie");
                                reveal_strlit("-default-user");
                                if str1.len() == str2.len() {
                                    assert(str1[str1.len() - 1] == 'r');
                                    assert(str2[str1.len() - 1] == 'e');
                                }
                            }
                        );
                        // Then we show that only if cr_key.name equals key.name, can this message be created in this step.
                        seq_lib::seq_equal_preserved_by_add(key.name, cr_key.name, new_strlit("-erlang-cookie")@);
                    },
                    SubResource::DefaultUserSecret => {
                        assert_by(
                            key.name + new_strlit("-default-user")@ != cr_key.name + new_strlit("-erlang-cookie")@,
                            {
                                let str1 = key.name + new_strlit("-default-user")@;
                                let str2 = cr_key.name + new_strlit("-erlang-cookie")@;
                                reveal_strlit("-erlang-cookie");
                                reveal_strlit("-default-user");
                                if str1.len() == str2.len() {
                                    assert(str1[str1.len() - 1] == 'r');
                                    assert(str2[str1.len() - 1] == 'e');
                                }
                            }
                        );
                        seq_lib::seq_equal_preserved_by_add(key.name, cr_key.name, new_strlit("-default-user")@);
                    },
                    SubResource::HeadlessService => {
                        assert_by(
                            key.name + new_strlit("-nodes")@ != cr_key.name + new_strlit("-client")@,
                            {
                                let str1 = key.name + new_strlit("-nodes")@;
                                let str2 = cr_key.name + new_strlit("-client")@;
                                reveal_strlit("-client");
                                reveal_strlit("-nodes");
                                if str1.len() == str2.len() {
                                    assert(str1[str1.len() - 1] == 's');
                                    assert(str2[str1.len() - 1] == 't');
                                }
                            }
                        );
                        seq_lib::seq_equal_preserved_by_add(key.name, cr_key.name, new_strlit("-nodes")@);
                    },
                    SubResource::Service => {
                        assert_by(
                            cr_key.name + new_strlit("-nodes")@ != key.name + new_strlit("-client")@,
                            {
                                let str1 = cr_key.name + new_strlit("-nodes")@;
                                let str2 = key.name + new_strlit("-client")@;
                                reveal_strlit("-client");
                                reveal_strlit("-nodes");
                                if str1.len() == str2.len() {
                                    assert(str1[str1.len() - 1] == 's');
                                    assert(str2[str1.len() - 1] == 't');
                                }
                            }
                        );
                        seq_lib::seq_equal_preserved_by_add(key.name, cr_key.name, new_strlit("-client")@);
                    },
                    SubResource::RoleBinding | SubResource::ServiceAccount | SubResource::StatefulSet => {
                        seq_lib::seq_equal_preserved_by_add(key.name, cr_key.name, new_strlit("-server")@);
                    },
                    SubResource::Role => {
                        seq_lib::seq_equal_preserved_by_add(key.name, cr_key.name, new_strlit("-peer-discovery")@);
                    },
                }
            }
        )
    }
}

pub proof fn lemma_eventually_always_no_delete_resource_request_msg_in_flight_forall(
    spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView
)
    requires
        spec.entails(always(lift_state(RMQCluster::each_object_in_etcd_is_well_formed()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::busy_disabled()))),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(tla_forall(|i| RMQCluster::kubernetes_api_next().weak_fairness(i))),
        spec.entails(always(lift_state(RMQCluster::desired_state_is(rabbitmq)))),
        spec.entails(always(tla_forall(|sub_resource: SubResource| lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq))))),
    ensures
        spec.entails(
            true_pred().leads_to(always(tla_forall(|sub_resource: SubResource| lift_state(no_delete_resource_request_msg_in_flight(sub_resource, rabbitmq)))))
        ),
{
    assert forall |sub_resource: SubResource| spec.entails(true_pred().leads_to(always(lift_state(#[trigger] no_delete_resource_request_msg_in_flight(sub_resource, rabbitmq))))) by {
        always_tla_forall_apply(spec, |res: SubResource| lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(res, rabbitmq)), sub_resource);
        lemma_eventually_always_no_delete_resource_request_msg_in_flight(spec, sub_resource, rabbitmq);
    }
    leads_to_always_tla_forall_subresource(spec, true_pred(), |sub_resource: SubResource| lift_state(no_delete_resource_request_msg_in_flight(sub_resource, rabbitmq)));
}

/// This lemma demonstrates how to use kubernetes_cluster::proof::kubernetes_api_liveness::lemma_true_leads_to_always_every_in_flight_req_msg_satisfies
/// (referred to as lemma_X) to prove that the system will eventually enter a state where delete stateful set request messages
/// will never appear in flight.
///
/// As an example, we can look at how this lemma is proved.
/// - Precondition: The preconditions should include all precondtions used by lemma_X and other predicates which show that
///     the newly generated messages are as expected. ("expected" means not delete stateful set request messages in this lemma. Therefore,
///     we provide an invariant stateful_set_has_owner_reference_pointing_to_current_cr so that the grabage collector won't try
///     to send a delete request to delete the messsage.)
/// - Postcondition: spec |= true ~> [](forall |msg| as_expected(msg))
/// - Proof body: The proof consists of three parts.
///   + Come up with "requirements" for those newly created api request messages. Usually, just move the forall |msg| and
///     s.in_flight().contains(msg) in the statepred of final state (no_delete_sts_req_is_in_flight in this lemma, so we can
///     get the requirements in this lemma).
///   + Show that spec |= every_new_req_msg_if_in_flight_then_satisfies. Basically, use two assert forall to show that forall state and
///     its next state and forall messages, if the messages are newly generated, they must satisfy the "requirements" and always satisfies it
///     as long as it is in flight.
///   + Call lemma_X. If a correct "requirements" are provided, we can easily prove the equivalence of every_in_flight_req_msg_satisfies(requirements)
///     and the original statepred.
#[verifier(spinoff_prover)]
pub proof fn lemma_eventually_always_no_delete_resource_request_msg_in_flight(
    spec: TempPred<RMQCluster>, sub_resource: SubResource, rabbitmq: RabbitmqClusterView
)
    requires
        spec.entails(always(lift_state(RMQCluster::each_object_in_etcd_is_well_formed()))),
        spec.entails(always(lift_state(RMQCluster::every_in_flight_msg_has_lower_id_than_allocator()))),
        spec.entails(always(lift_state(RMQCluster::busy_disabled()))),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(tla_forall(|i| RMQCluster::kubernetes_api_next().weak_fairness(i))),
        spec.entails(always(lift_state(RMQCluster::desired_state_is(rabbitmq)))),
        spec.entails(always(lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq))))
    ensures
        spec.entails(
            true_pred().leads_to(always(lift_state(no_delete_resource_request_msg_in_flight(sub_resource, rabbitmq))))
        ),
{
    let key = rabbitmq.object_ref();
    let resource_key = get_request(sub_resource, rabbitmq).key;
    let requirements = |msg: RMQMessage, s: RMQCluster| !{
        &&& msg.dst.is_KubernetesAPI()
        &&& msg.content.is_delete_request()
        &&& msg.content.get_delete_request().key == resource_key
    };

    let stronger_next = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::desired_state_is(rabbitmq)(s)
        &&& resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq)(s)
        &&& RMQCluster::each_object_in_etcd_is_well_formed()(s)
    };
    assert forall |s: RMQCluster, s_prime: RMQCluster| #[trigger] stronger_next(s, s_prime) implies RMQCluster::every_new_req_msg_if_in_flight_then_satisfies(requirements)(s, s_prime) by {
        assert forall |msg: RMQMessage| (!s.in_flight().contains(msg) || requirements(msg, s)) && #[trigger] s_prime.in_flight().contains(msg)
        implies requirements(msg, s_prime) by {
            if s.in_flight().contains(msg) {
                assert(requirements(msg, s));
                assert(requirements(msg, s_prime));
            } else {
                let step = choose |step| RMQCluster::next_step(s, s_prime, step);
                match step {
                    Step::BuiltinControllersStep(_) => {
                        if s.resources().contains_key(resource_key) {
                            let owner_refs = s.resources()[resource_key].metadata.owner_references;
                            assert(owner_refs == Some(seq![rabbitmq.controller_owner_ref()]));
                            assert(owner_reference_to_object_reference(owner_refs.get_Some_0()[0], key.namespace) == key);
                        }
                    },
                    Step::ControllerStep(input) => {
                        let cr_key = input.1.get_Some_0();
                        if s_prime.ongoing_reconciles()[cr_key].pending_req_msg.is_Some() {
                            assert(msg == s_prime.ongoing_reconciles()[cr_key].pending_req_msg.get_Some_0());
                            assert(!s_prime.ongoing_reconciles()[cr_key].pending_req_msg.get_Some_0().content.is_delete_request());
                        }
                    },
                    Step::ClientStep() => {
                        if msg.content.is_delete_request() {
                            assert(msg.content.get_delete_request().key.kind != resource_key.kind);
                        }
                    },
                    _ => {
                        assert(requirements(msg, s_prime));
                    }
                }
            }
        }
    }
    invariant_n!(
        spec, lift_action(stronger_next), lift_action(RMQCluster::every_new_req_msg_if_in_flight_then_satisfies(requirements)),
        lift_action(RMQCluster::next()), lift_state(RMQCluster::desired_state_is(rabbitmq)),
        lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq)),
        lift_state(RMQCluster::each_object_in_etcd_is_well_formed())
    );

    RMQCluster::lemma_true_leads_to_always_every_in_flight_req_msg_satisfies(spec, requirements);
    temp_pred_equality(
        lift_state(no_delete_resource_request_msg_in_flight(sub_resource, rabbitmq)),
        lift_state(RMQCluster::every_in_flight_req_msg_satisfies(requirements))
    );
}

pub proof fn lemma_eventually_always_resource_object_only_has_owner_reference_pointing_to_current_cr_forall(
    spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_state(RMQCluster::busy_disabled()))),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(tla_forall(|i| RMQCluster::kubernetes_api_next().weak_fairness(i))),
        spec.entails(tla_forall(|i| RMQCluster::builtin_controllers_next().weak_fairness(i))),
        spec.entails(always(lift_state(RMQCluster::desired_state_is(rabbitmq)))),
        spec.entails(always(tla_forall(|sub_resource: SubResource| lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(sub_resource, rabbitmq))))),
        spec.entails(always(tla_forall(|sub_resource: SubResource|lift_state(every_resource_create_request_implies_at_after_create_resource_step(sub_resource, rabbitmq))))),
        spec.entails(always(tla_forall(|sub_resource: SubResource|lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq))))),
    ensures
        spec.entails(true_pred().leads_to(always(tla_forall(|sub_resource: SubResource| (lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq))))))),
{
    assert forall |sub_resource: SubResource| spec.entails(true_pred().leads_to(always(lift_state(#[trigger] resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq))))) by {
        always_tla_forall_apply(spec, |res: SubResource| lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(res, rabbitmq)), sub_resource);
        always_tla_forall_apply(spec, |res: SubResource|lift_state(every_resource_create_request_implies_at_after_create_resource_step(res, rabbitmq)), sub_resource);
        always_tla_forall_apply(spec, |res: SubResource|lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(res, rabbitmq)), sub_resource);
        lemma_eventually_always_resource_object_only_has_owner_reference_pointing_to_current_cr(spec, sub_resource, rabbitmq);
    }
    leads_to_always_tla_forall_subresource(spec, true_pred(), |sub_resource: SubResource| lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq)));
}

#[verifier(spinoff_prover)]
pub proof fn lemma_eventually_always_resource_object_only_has_owner_reference_pointing_to_current_cr(
    spec: TempPred<RMQCluster>, sub_resource: SubResource, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_state(RMQCluster::busy_disabled()))),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(tla_forall(|i| RMQCluster::kubernetes_api_next().weak_fairness(i))),
        spec.entails(tla_forall(|i| RMQCluster::builtin_controllers_next().weak_fairness(i))),
        spec.entails(always(lift_state(RMQCluster::desired_state_is(rabbitmq)))),
        spec.entails(always(lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(sub_resource, rabbitmq)))),
        spec.entails(always(lift_state(every_resource_create_request_implies_at_after_create_resource_step(sub_resource, rabbitmq)))),
        spec.entails(always(lift_state(object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq)))),
    ensures
        spec.entails(true_pred().leads_to(always(lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq))))),
{
    let key = get_request(sub_resource, rabbitmq).key;
    let eventual_owner_ref = |owner_ref: Option<Seq<OwnerReferenceView>>| {owner_ref == Some(seq![rabbitmq.controller_owner_ref()])};
    always_weaken(spec, object_in_every_resource_update_request_only_has_owner_references_pointing_to_current_cr(sub_resource, rabbitmq), RMQCluster::every_update_msg_sets_owner_references_as(key, eventual_owner_ref));
    always_weaken(spec, every_resource_create_request_implies_at_after_create_resource_step(sub_resource, rabbitmq), RMQCluster::every_create_msg_sets_owner_references_as(key, eventual_owner_ref));
    always_weaken(spec, resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(sub_resource, rabbitmq), RMQCluster::object_has_no_finalizers(key));

    let state = |s: RMQCluster| {
        RMQCluster::desired_state_is(rabbitmq)(s)
        && resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(sub_resource, rabbitmq)(s)
    };
    invariant_n!(
        spec, lift_state(state), lift_state(RMQCluster::objects_owner_references_violates(key, eventual_owner_ref)).implies(lift_state(RMQCluster::garbage_collector_deletion_enabled(key))),
        lift_state(RMQCluster::desired_state_is(rabbitmq)),
        lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(sub_resource, rabbitmq))
    );
    RMQCluster::lemma_eventually_objects_owner_references_satisfies(spec, key, eventual_owner_ref);
    temp_pred_equality(
        lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(sub_resource, rabbitmq)),
        lift_state(RMQCluster::objects_owner_references_satisfies(key, eventual_owner_ref))
    );
}

pub proof fn leads_to_always_tla_forall_subresource(spec: TempPred<RMQCluster>, p: TempPred<RMQCluster>, a_to_p: FnSpec(SubResource)->TempPred<RMQCluster>)
    requires
        forall |a: SubResource| spec.entails(p.leads_to(always(#[trigger] a_to_p(a)))),
    ensures
        spec.entails(p.leads_to(always(tla_forall(a_to_p)))),
{
    leads_to_always_tla_forall(
        spec, p, a_to_p,
        set![SubResource::HeadlessService, SubResource::Service, SubResource::ErlangCookieSecret, SubResource::DefaultUserSecret,
        SubResource::PluginsConfigMap, SubResource::ServerConfigMap, SubResource::ServiceAccount, SubResource::Role,
        SubResource::RoleBinding, SubResource::StatefulSet]
    );
}

// Below are invariants that only hold after the config map matches the desired state

#[verifier(spinoff_prover)]
pub proof fn lemma_eventually_always_stateful_set_not_exists_or_matches_or_no_more_status_update(
    spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView
)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(tla_forall(|i| RMQCluster::kubernetes_api_next().weak_fairness(i))),
        spec.entails(tla_forall(|i| RMQCluster::builtin_controllers_next().weak_fairness(i))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_etcd_is_well_formed()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()))),
        spec.entails(always(lift_state(RMQCluster::desired_state_is(rabbitmq)))),
        spec.entails(always(lift_state(every_resource_create_request_implies_at_after_create_resource_step(SubResource::StatefulSet, rabbitmq)))),
        spec.entails(always(lift_state(every_resource_update_request_implies_at_after_update_resource_step(SubResource::StatefulSet, rabbitmq)))),
        spec.entails(always(lift_state(object_in_etcd_satisfies_unchangeable(SubResource::StatefulSet, rabbitmq)))),
        spec.entails(always(lift_state(stateful_set_in_etcd_satisfies_unchangeable(rabbitmq)))),
        spec.entails(always(lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(SubResource::StatefulSet, rabbitmq)))),
        spec.entails(always(lift_state(cm_rv_is_the_same_as_etcd_server_cm_if_cm_updated(rabbitmq)))),
        spec.entails(always(lift_state(sub_resource_state_matches(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(no_update_status_request_msg_not_from_bc_in_flight_of_stateful_set(rabbitmq)))),
        spec.entails(always(lift_action(cm_rv_stays_unchanged(rabbitmq)))),
    ensures
        spec.entails(
            true_pred().leads_to(always(lift_state(stateful_set_not_exists_or_matches_or_no_more_status_update(rabbitmq))))
        ),
{
    let sts_key = get_request(SubResource::StatefulSet, rabbitmq).key;
    let cm_key = get_request(SubResource::ServerConfigMap, rabbitmq).key;
    let make_fn = |rv: StringView| make_stateful_set(rabbitmq, rv);
    let stronger_inv = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::each_object_in_etcd_is_well_formed()(s)
        &&& RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()(s)
        &&& RMQCluster::desired_state_is(rabbitmq)(s)
        &&& every_resource_create_request_implies_at_after_create_resource_step(SubResource::StatefulSet, rabbitmq)(s)
        &&& every_resource_update_request_implies_at_after_update_resource_step(SubResource::StatefulSet, rabbitmq)(s)
        &&& object_in_etcd_satisfies_unchangeable(SubResource::StatefulSet, rabbitmq)(s)
        &&& stateful_set_in_etcd_satisfies_unchangeable(rabbitmq)(s)
        &&& resource_object_only_has_owner_reference_pointing_to_current_cr(SubResource::StatefulSet, rabbitmq)(s)
        &&& cm_rv_is_the_same_as_etcd_server_cm_if_cm_updated(rabbitmq)(s)
        &&& sub_resource_state_matches(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& no_update_status_request_msg_not_from_bc_in_flight_of_stateful_set(rabbitmq)(s)
        &&& cm_rv_stays_unchanged(rabbitmq)(s, s_prime)
    };

    assert forall |s, s_prime| #[trigger] stronger_inv(s, s_prime)
    implies RMQCluster::every_in_flight_create_req_msg_for_this_sts_matches(sts_key, cm_key, make_fn)(s) by {
        assert forall |msg| {
            &&& #[trigger] s.network_state.in_flight.contains(msg)
            &&& msg.dst.is_KubernetesAPI()
            &&& msg.content.is_create_request()
            &&& msg.content.get_create_request().namespace == sts_key.namespace
            &&& msg.content.get_create_request().obj.metadata.name == Some(sts_key.name)
            &&& msg.content.get_create_request().obj.kind == sts_key.kind
        } implies {
            &&& msg.content.get_create_request().obj == make_fn(int_to_string_view(s.resources()[cm_key].metadata.resource_version.get_Some_0())).marshal()
        } by {}
    }
    invariant_n!(
        spec, lift_action(stronger_inv), lift_state(RMQCluster::every_in_flight_create_req_msg_for_this_sts_matches(sts_key, cm_key, make_fn)),
        lift_state(RMQCluster::each_object_in_etcd_is_well_formed()),
        lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()),
        lift_state(RMQCluster::desired_state_is(rabbitmq)),
        lift_state(every_resource_create_request_implies_at_after_create_resource_step(SubResource::StatefulSet, rabbitmq)),
        lift_state(every_resource_update_request_implies_at_after_update_resource_step(SubResource::StatefulSet, rabbitmq)),
        lift_state(object_in_etcd_satisfies_unchangeable(SubResource::StatefulSet, rabbitmq)),
        lift_state(stateful_set_in_etcd_satisfies_unchangeable(rabbitmq)),
        lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(SubResource::StatefulSet, rabbitmq)),
        lift_state(cm_rv_is_the_same_as_etcd_server_cm_if_cm_updated(rabbitmq)),
        lift_state(sub_resource_state_matches(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(no_update_status_request_msg_not_from_bc_in_flight_of_stateful_set(rabbitmq)),
        lift_action(cm_rv_stays_unchanged(rabbitmq))
    );

    assert forall |s, s_prime| #[trigger] stronger_inv(s, s_prime)
    implies RMQCluster::every_in_flight_update_req_msg_for_this_sts_matches(sts_key, cm_key, make_fn)(s) by {
        assert forall |msg| {
            &&& #[trigger] s.network_state.in_flight.contains(msg)
            &&& msg.dst.is_KubernetesAPI()
            &&& msg.content.is_update_request()
            &&& msg.content.get_update_request().key() == sts_key
        } implies {
            &&& msg.content.get_update_request().obj.metadata.resource_version.is_Some()
            &&& {
                &&& s.resources().contains_key(sts_key)
                &&& msg.content.get_update_request().obj.metadata.resource_version == s.resources()[sts_key].metadata.resource_version
            } ==> {
                let rv = int_to_string_view(s.resources()[cm_key].metadata.resource_version.get_Some_0());
                let made_sts = make_fn(rv);
                let obj = msg.content.get_update_request().obj;
                &&& StatefulSetView::unmarshal(obj).is_Ok()
                &&& StatefulSetView::unmarshal(obj).get_Ok_0().spec.is_Some()
                &&& StatefulSetView::unmarshal(obj).get_Ok_0().spec.get_Some_0().replicas == made_sts.spec.get_Some_0().replicas
                &&& StatefulSetView::unmarshal(obj).get_Ok_0().spec.get_Some_0().template == made_sts.spec.get_Some_0().template
                &&& StatefulSetView::unmarshal(obj).get_Ok_0().spec.get_Some_0().persistent_volume_claim_retention_policy == made_sts.spec.get_Some_0().persistent_volume_claim_retention_policy
                &&& obj.metadata.labels == made_sts.metadata.labels
                &&& obj.metadata.annotations == made_sts.metadata.annotations
            }
        } by {
            StatefulSetView::marshal_spec_preserves_integrity();
            StatefulSetView::marshal_status_preserves_integrity();
        }
    }
    invariant_n!(
        spec, lift_action(stronger_inv), lift_state(RMQCluster::every_in_flight_update_req_msg_for_this_sts_matches(sts_key, cm_key, make_fn)),
        lift_state(RMQCluster::each_object_in_etcd_is_well_formed()),
        lift_state(RMQCluster::each_object_in_reconcile_has_consistent_key_and_valid_metadata()),
        lift_state(RMQCluster::desired_state_is(rabbitmq)),
        lift_state(every_resource_create_request_implies_at_after_create_resource_step(SubResource::StatefulSet, rabbitmq)),
        lift_state(every_resource_update_request_implies_at_after_update_resource_step(SubResource::StatefulSet, rabbitmq)),
        lift_state(object_in_etcd_satisfies_unchangeable(SubResource::StatefulSet, rabbitmq)),
        lift_state(stateful_set_in_etcd_satisfies_unchangeable(rabbitmq)),
        lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(SubResource::StatefulSet, rabbitmq)),
        lift_state(cm_rv_is_the_same_as_etcd_server_cm_if_cm_updated(rabbitmq)),
        lift_state(sub_resource_state_matches(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(no_update_status_request_msg_not_from_bc_in_flight_of_stateful_set(rabbitmq)),
        lift_action(cm_rv_stays_unchanged(rabbitmq))
    );

    temp_pred_equality(lift_action(cm_rv_stays_unchanged(rabbitmq)), lift_action(RMQCluster::obj_rv_stays_unchanged(cm_key)));

    RMQCluster::lemma_true_leads_to_always_stateful_set_not_exist_or_updated_or_no_more_pending_req(spec, sts_key, cm_key, make_fn);

    assert forall |s, s_prime| #[trigger] stronger_inv(s, s_prime) && RMQCluster::stateful_set_not_exist_or_updated_or_no_more_status_from_bc(sts_key, cm_key, make_fn)(s)
    implies stateful_set_not_exists_or_matches_or_no_more_status_update(rabbitmq)(s) by {
        StatefulSetView::marshal_spec_preserves_integrity();
        StatefulSetView::marshal_status_preserves_integrity();
    }

    leads_to_always_enhance(spec, lift_action(stronger_inv), true_pred(),
        lift_state(RMQCluster::stateful_set_not_exist_or_updated_or_no_more_status_from_bc(sts_key, cm_key, make_fn)),
        lift_state(stateful_set_not_exists_or_matches_or_no_more_status_update(rabbitmq))
    );
}

#[verifier(spinoff_prover)]
pub proof fn lemma_always_cm_rv_stays_unchanged(spec: TempPred<RMQCluster>, rabbitmq: RabbitmqClusterView)
    requires
        rabbitmq.well_formed(),
        spec.entails(always(lift_action(RMQCluster::next()))),
        spec.entails(always(lift_state(RMQCluster::each_object_in_etcd_is_well_formed()))),
        spec.entails(always(lift_state(every_resource_update_request_implies_at_after_update_resource_step(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(sub_resource_state_matches(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(SubResource::ServerConfigMap, rabbitmq)))),
        spec.entails(always(lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq)))),
    ensures
        spec.entails(always(lift_action(cm_rv_stays_unchanged(rabbitmq)))),
{
    let sts_key = get_request(SubResource::StatefulSet, rabbitmq).key;
    let cm_key = get_request(SubResource::ServerConfigMap, rabbitmq).key;
    let make_fn = |rv: StringView| make_stateful_set(rabbitmq, rv);
    let stronger_inv = |s: RMQCluster, s_prime: RMQCluster| {
        &&& RMQCluster::next()(s, s_prime)
        &&& RMQCluster::each_object_in_etcd_is_well_formed()(s)
        &&& every_resource_update_request_implies_at_after_update_resource_step(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& sub_resource_state_matches(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(SubResource::ServerConfigMap, rabbitmq)(s)
        &&& resource_object_only_has_owner_reference_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq)(s)
    };

    invariant_n!(
        spec, lift_action(stronger_inv), lift_action(cm_rv_stays_unchanged(rabbitmq)),
        lift_action(RMQCluster::next()),
        lift_state(RMQCluster::each_object_in_etcd_is_well_formed()),
        lift_state(every_resource_update_request_implies_at_after_update_resource_step(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(no_update_status_request_msg_in_flight_of_except_stateful_set(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(no_delete_resource_request_msg_in_flight(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(sub_resource_state_matches(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(resource_object_has_no_finalizers_or_timestamp_and_only_has_controller_owner_ref(SubResource::ServerConfigMap, rabbitmq)),
        lift_state(resource_object_only_has_owner_reference_pointing_to_current_cr(SubResource::ServerConfigMap, rabbitmq))
    );
}

}