use crate::{
    lockup::{Lockup, LockupIndex},
    Contract, StorageKey,
};
use near_sdk::{collections::UnorderedSet, require, AccountId};
use std::collections::HashSet;

impl Contract {
    pub(crate) fn assert_deposit_allowlist(&self, account_id: &AccountId) {
        require!(
            self.deposit_allowlist.contains(account_id),
            "Not in deposit allowlist"
        );
    }

    pub(crate) fn internal_add_lockup(&mut self, lockup: &Lockup) -> LockupIndex {
        let index = self.lockups.len() as LockupIndex;
        self.lockups.push(lockup);
        let mut indices = self
            .account_lockups
            .get(&lockup.account_id)
            .unwrap_or(UnorderedSet::new(StorageKey::AccountLockups));
        indices.insert(&index);
        self.internal_save_account_lockups(&lockup.account_id, indices);
        index
    }

    pub(crate) fn internal_save_account_lockups(
        &mut self,
        account_id: &AccountId,
        indices: UnorderedSet<LockupIndex>,
    ) {
        if indices.is_empty() {
            self.account_lockups.remove(account_id);
        } else {
            self.account_lockups.insert(account_id, &indices);
        }
    }

    pub(crate) fn internal_get_account_lockups(
        &self,
        account_id: &AccountId,
    ) -> Vec<(LockupIndex, Lockup)> {
        self.account_lockups
            .get(account_id)
            .unwrap_or(UnorderedSet::new(StorageKey::AccountLockups))
            .iter()
            .map(|lockup_index| (lockup_index, self.lockups.get(lockup_index as _).unwrap()))
            .collect()
    }

    pub(crate) fn internal_get_account_lockups_by_id(
        &self,
        account_id: &AccountId,
        lockup_ids: &HashSet<LockupIndex>,
    ) -> Vec<(LockupIndex, Lockup)> {
        let account_lockup_ids = self
            .account_lockups
            .get(account_id)
            .unwrap_or(UnorderedSet::new(StorageKey::AccountLockups));

        lockup_ids
            .iter()
            .map(|&lockup_index| {
                require!(
                    account_lockup_ids.contains(&lockup_index),
                    format!("lockup not found for account: {}", lockup_index),
                );
                let lockup = self.lockups.get(lockup_index).unwrap();
                (lockup_index, lockup)
            })
            .collect()
    }
}
