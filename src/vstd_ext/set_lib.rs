// Copyright 2022 VMware, Inc.
// SPDX-License-Identifier: MIT
#![allow(unused_imports)]
use vstd::prelude::*;
use vstd::set::*;
use vstd::set_lib::*;

verus! {

pub proof fn finite_set_to_seq_contains_all_set_elements<A>(s: Set<A>)
    requires s.finite(),
    ensures forall |e: A| #![auto] s.contains(e) <==> s.to_seq().contains(e)
    {
        _finite_set_to_seq_contains_all_set_elements(s);
        _finite_seq_to_set_contains_all_set_elements(s);
    }

pub proof fn _finite_set_to_seq_contains_all_set_elements<A>(s: Set<A>)
    requires s.finite(),
    ensures forall |e: A| #![auto] s.contains(e) ==> s.to_seq().contains(e),
    decreases s.len()
    {
        if s.len() != 0 {
            assert forall |e: A| #[trigger] s.contains(e) implies s.to_seq().contains(e) by {
                _element_in_finite_set_exists_in_set_to_seq(s, e);
            }
        }
    }

pub proof fn _element_in_finite_set_exists_in_set_to_seq<A>(s: Set<A>, e: A)
    requires s.finite(), s.contains(e),
    ensures s.to_seq().contains(e),
    decreases s.len()
    {
        if s.len() != 0 {
            // need choose() to be not-random
            let e2 = s.choose();
            if e2 == e {
                assert (s.to_seq() == Seq::empty().push(e) + s.remove(e).to_seq());
                assert (s.to_seq()[0] == e && s.to_seq().contains(e));
            } else {
                assert(s.remove(e2).contains(e));
                _element_in_finite_set_exists_in_set_to_seq(s.remove(e2), e);
                assert (s.remove(e2).to_seq().contains(e));
                assert (s.to_seq() == Seq::empty().push(e2) + s.remove(e2).to_seq());
                assert (s.to_seq().subrange(1, s.to_seq().len() as int) == s.remove(e2).to_seq());
                assert (s.to_seq().subrange(1, s.to_seq().len() as int).contains(e));
                assert (s.to_seq().contains(e));
            }
        }
    }

pub proof fn _finite_seq_to_set_contains_all_set_elements<A>(s: Set<A>)
    requires s.finite(),
    ensures forall |e: A| #![auto] s.contains(e) <== s.to_seq().contains(e),
    decreases s.len()
    {
        if s.len() != 0 {
            assert forall |e: A| #[trigger] s.to_seq().contains(e) implies s.contains(e) by {
                _element_in_seq_exists_in_original_finite(s, e);
            }
        }
    }

pub proof fn _element_in_seq_exists_in_original_finite<A>(s: Set<A>, e: A)
    requires s.finite(), s.to_seq().contains(e),
    ensures s.contains(e),
    decreases s.len()
    {
        if s.len() != 0 {
            // need choose() to be not-random
            let e2 = s.choose();
            if e2 == e {
                assert (s.contains(e));
            } else {
                assert (s.to_seq() == Seq::empty().push(e2) + s.remove(e2).to_seq());
                assert (s.to_seq().subrange(1, s.to_seq().len() as int) == s.remove(e2).to_seq());
                assert (s.to_seq()[0] == e2);
                assert (s.remove(e2).to_seq().contains(e));
                _element_in_seq_exists_in_original_finite(s.remove(e2), e);
                assert (s.remove(e2).contains(e));
                assert (s.contains(e2));
                assert (s == s.remove(e2).insert(e2));
                assert (s.contains(e));
            }
        }
    }
}
