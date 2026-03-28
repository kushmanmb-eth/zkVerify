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

//! Migrate from storage v4 to v5.
//!
//! The change in v5 is the removal of the `User<A>` wrapper for the domain `owner` field.
//! The owner is now represented as `Option<A>` where `None` means manager-owned (no specific
//! account) and `Some(account)` means account-owned.

mod v4;

use alloc::vec::Vec;
use frame_support::{migrations::VersionedMigration, traits::UncheckedOnRuntimeUpgrade};
use sp_core::Get;

/// Implements [`UncheckedOnRuntimeUpgrade`], migrating the state of this pallet from V4 to V5.
///
/// In V4 of the template, the value of the [`crate::Domains`] `StorageMap` uses `User<A>` for
/// the domain owner, which is either `Account(A)` or `Manager`.
///
/// We migrate every domain by converting the `User<A>` owner to `Option<A>`:
/// - `User::Account(a)` → `Some(a)`
/// - `User::Manager` → `None`
pub struct InnerMigrateV4ToV5<T>(core::marker::PhantomData<T>);

impl<T: crate::Config> UncheckedOnRuntimeUpgrade for InnerMigrateV4ToV5<T> {
    /// Migrate the storage from V4 to V5.
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        let old_storage = v4::Domains::<T>::drain().collect::<Vec<_>>();
        let (reads, mut writes) = (old_storage.len() as u64, old_storage.len() as u64);
        let converted = old_storage
            .into_iter()
            .map(|(domain_id, domain)| {
                (
                    domain_id,
                    crate::Domain::<T>(crate::data::DomainEntry {
                        id: domain.id,
                        owner: domain.owner.account().cloned(),
                        state: domain.state,
                        next: domain.next,
                        max_aggregation_size: domain.max_aggregation_size,
                        should_publish: domain.should_publish,
                        publish_queue_size: domain.publish_queue_size,
                        ticket_domain: domain.ticket_domain,
                        ticket_allowlist: domain.ticket_allowlist,
                        aggregate_rules: domain.aggregate_rules,
                        proof_rules: domain.proof_rules,
                        delivery: domain.delivery,
                    }),
                )
            })
            .collect::<Vec<_>>();
        writes += converted.len() as u64;
        for (domain_id, migrated_domain) in converted.into_iter() {
            crate::Domains::<T>::insert(domain_id, migrated_domain);
        }
        T::DbWeight::get().reads_writes(reads, writes)
    }
}

/// [`UncheckedOnRuntimeUpgrade`] implementation [`InnerMigrateV4ToV5`] wrapped in a
/// [`VersionedMigration`](frame_support::migrations::VersionedMigration), which ensures that:
/// - The migration only runs once when the on-chain storage version is 4
/// - The on-chain storage version is updated to `5` after the migration executes
/// - Reads/Writes from checking/settings the on-chain storage version are accounted for
pub type MigrateV4ToV5<T> = VersionedMigration<
    4, // The migration will only execute when the on-chain storage version is 4
    5, // The on-chain storage version will be set to 5 after the migration is complete
    InnerMigrateV4ToV5<T>,
    crate::Pallet<T>,
    <T as frame_system::Config>::DbWeight,
>;

#[cfg(test)]
mod tests {
    use super::v4;
    use super::*;
    use crate::data::{AggregateSecurityRules, DomainState, ProofSecurityRules, User};
    use crate::mock::*;
    use frame_support::weights::RuntimeDbWeight;
    use frame_support::{BoundedBTreeMap, BoundedVec};
    use sp_core::Get;

    fn create_old_domain(
        id: u32,
        owner: v4::User<u64>,
        state: v4::DomainState,
    ) -> v4::Domain<Test> {
        v4::Domain::<Test> {
            id,
            owner,
            state,
            next: v4::AggregationEntry {
                id: 42,
                size: 16,
                statements: BoundedVec::default(),
            },
            max_aggregation_size: 32,
            should_publish: BoundedBTreeMap::new(),
            publish_queue_size: 5,
            aggregate_rules: AggregateSecurityRules::Untrusted,
            ticket_domain: None,
            ticket_allowlist: None,
            proof_rules: ProofSecurityRules::Untrusted,
            delivery: crate::data::DeliveryParams::new(
                123_u64,
                crate::data::Delivery::new(hp_dispatch::Destination::None, 100, 33),
            ),
        }
    }

    #[test]
    fn successful_migration() {
        test().execute_with(|| {
            // CLEAN THE TEST STORAGE
            v4::Domains::<Test>::drain().count();

            v4::Domains::<Test>::insert(
                23,
                create_old_domain(23, User::from(123), DomainState::Ready),
            );
            v4::Domains::<Test>::insert(
                42,
                create_old_domain(42, User::from(321), DomainState::Hold),
            );
            // Manager-owned domain
            v4::Domains::<Test>::insert(
                1,
                create_old_domain(1, User::Manager, DomainState::Removable),
            );
            v4::Domains::<Test>::insert(
                2,
                create_old_domain(2, User::from(42), DomainState::Removable),
            );

            // Perform runtime upgrade
            let weight = InnerMigrateV4ToV5::<Test>::on_runtime_upgrade();

            // Check that all domains were migrated
            assert_eq!(crate::Domains::<Test>::iter().count(), 4);

            let domain_data = |id| {
                let crate::Domain::<Test>(crate::data::DomainEntry { owner, state, .. }) =
                    crate::Domains::<Test>::take(id).unwrap();
                (owner, state)
            };

            // Account-owned domains: User::Account(a) → Some(a)
            assert_eq!(domain_data(23), (Some(123_u64), DomainState::Ready,));
            assert_eq!(domain_data(42), (Some(321_u64), DomainState::Hold,));
            assert_eq!(domain_data(2), (Some(42_u64), DomainState::Removable,));
            // Manager-owned domain: User::Manager → None
            assert_eq!(domain_data(1), (None, DomainState::Removable,));

            // Check that weight is as expected
            assert_eq!(
                weight,
                <<Test as frame_system::Config>::DbWeight as Get<RuntimeDbWeight>>::get()
                    .reads_writes(4, 8)
            );
        })
    }
}
