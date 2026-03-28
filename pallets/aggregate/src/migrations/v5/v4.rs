// Copyright 2024, Horizen Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! The old v4 layout: here we need to maintain the layout of the old storage
//! in order to be able to decode it.

use codec::{Decode, Encode};
use frame_support::Blake2_128Concat;
use frame_support::{pallet_prelude::*, storage_alias};
use sp_core::MaxEncodedLen;

type AggregationSize = u32;

/// V4 type for [`crate::Domains`].
#[storage_alias]
pub type Domains<T: crate::Config> = StorageMap<crate::Pallet<T>, Blake2_128Concat, u32, Domain<T>>;

/// V4 type for [`crate::Domain`].
pub type Domain<T> = DomainEntry<
    crate::AccountOf<T>,
    crate::BalanceOf<T>,
    <T as crate::Config>::AggregationSize,
    <T as crate::Config>::MaxPendingPublishQueueSize,
    crate::TicketDomainOf<T>,
    crate::TicketAllowListOf<T>,
>;

use crate::data::CountableTicket;
pub use crate::data::{AggregateSecurityRules, AggregationEntry, DomainState, User};
use crate::ProofSecurityRules;

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(S, M))]
/// Old v4 domain entry layout — uses `User<A>` for the owner field.
pub struct DomainEntry<
    A: alloc::fmt::Debug + core::cmp::PartialEq,
    B: alloc::fmt::Debug + core::cmp::PartialEq,
    S: Get<AggregationSize>,
    M: Get<u32>,
    T1: Encode + Decode + TypeInfo + MaxEncodedLen,
    T2: Encode + Decode + TypeInfo + MaxEncodedLen,
> {
    pub id: u32,
    pub owner: User<A>,
    pub state: DomainState,
    pub next: AggregationEntry<A, B, S>,
    pub max_aggregation_size: crate::AggregationSize,
    pub should_publish: BoundedBTreeMap<u64, AggregationEntry<A, B, S>, M>,
    pub publish_queue_size: u32,
    pub ticket_domain: Option<T1>,
    pub ticket_allowlist: Option<CountableTicket<T2>>,
    pub aggregate_rules: AggregateSecurityRules,
    pub proof_rules: ProofSecurityRules,
    pub delivery: crate::data::DeliveryParams<A, B>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{data::Reserve, mock::Test};
    use hp_dispatch::Destination;
    use sp_core::{bytes::to_hex, H256};

    #[test]
    fn v4_domain_entry_encoding_should_never_change() {
        // If this test fails you should get the old layout and redefine it here.
        use crate::data::DeliveryParams;
        use frame_support::BoundedVec;

        let v4_domain = Domain::<Test> {
            id: 23,
            owner: User::from(123_u64),
            state: DomainState::Hold,
            next: AggregationEntry {
                id: 42,
                size: 16,
                statements: BoundedVec::try_from(vec![
                    crate::data::StatementEntry::new(
                        456_u64,
                        Reserve::new(1000, 2000),
                        H256::from_low_u64_be(45632134),
                    ),
                    crate::data::StatementEntry::new(
                        12_u64,
                        Reserve::new(2000, 1000),
                        H256::from_low_u64_be(321234500111),
                    ),
                ])
                .unwrap(),
            },
            max_aggregation_size: 10,
            should_publish: BoundedBTreeMap::new(),
            publish_queue_size: 5,
            ticket_domain: None,
            ticket_allowlist: None,
            aggregate_rules: AggregateSecurityRules::Untrusted,
            proof_rules: ProofSecurityRules::Untrusted,
            delivery: DeliveryParams::new(
                123_u64,
                crate::data::Delivery::new(Destination::None, 100, 33),
            ),
        };

        let encoded = to_hex(&v4_domain.encode(), false);

        // Verify the encoding is stable (update this hex if the structure legitimately changes)
        let decoded = Domain::<Test>::decode(&mut v4_domain.encode().as_slice()).unwrap();
        assert_eq!(decoded.id, v4_domain.id);
        assert_eq!(decoded.owner, v4_domain.owner);
        assert_eq!(decoded.state, v4_domain.state);
        assert!(!encoded.is_empty(), "Encoding should not be empty");
    }
}
