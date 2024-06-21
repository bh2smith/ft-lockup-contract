use borsh::BorshSerialize;
use near_sdk::{
    assert_one_yocto,
    collections::{LookupMap, UnorderedSet, Vector},
    env,
    json_types::U128,
    log, near, serde_json, AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise,
    PromiseOrValue,
};
use near_sdk_contract_tools::standard::nep297::Event;
use std::collections::HashMap;

pub mod callbacks;
pub mod event;
pub mod ft_token_receiver;
pub mod internal;
pub mod lockup;
pub mod schedule;
pub mod termination;
pub mod util;
pub mod view;

use crate::{event::*, lockup::*, schedule::*, util::*};

pub type TokenAccountId = AccountId;

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const GAS_FOR_FT_TRANSFER: Gas = Gas::from_gas(15_000_000_000_000);
const GAS_FOR_AFTER_FT_TRANSFER: Gas = Gas::from_gas(20_000_000_000_000);

const ONE_YOCTO: NearToken = NearToken::from_yoctonear(1);

#[near(contract_state, serializers = [borsh])]
#[derive(PanicOnDefault)]
pub struct Contract {
    pub token_account_id: AccountId,

    pub lockups: Vector<Lockup>,

    pub account_lockups: LookupMap<AccountId, UnorderedSet<LockupIndex>>,

    /// account ids that can perform all actions:
    /// - manage deposit_allowlist
    /// - create lockups, terminate lockups
    pub deposit_allowlist: UnorderedSet<AccountId>,
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Lockups,
    AccountLockups,
    DepositAllowlist,
}

#[near]
impl Contract {
    #[init]
    pub fn new(token_account_id: AccountId, deposit_allowlist: Vec<AccountId>) -> Self {
        let mut deposit_allowlist_set = UnorderedSet::new(StorageKey::DepositAllowlist);
        deposit_allowlist_set.extend(deposit_allowlist.clone());
        FtLockupNew {
            token_account_id: token_account_id.clone(),
        }
        .emit();
        FtLockupAddToDepositAllowlist {
            account_ids: deposit_allowlist,
        }
        .emit();
        Self {
            lockups: Vector::new(StorageKey::Lockups),
            account_lockups: LookupMap::new(StorageKey::AccountLockups),
            token_account_id,
            deposit_allowlist: deposit_allowlist_set,
        }
    }

    pub fn claim(
        &mut self,
        amounts: Option<Vec<(LockupIndex, Option<NearToken>)>>,
    ) -> PromiseOrValue<NearToken> {
        let account_id = env::predecessor_account_id();

        let (claim_amounts, mut lockups_by_id) = if let Some(amounts) = amounts {
            let lockups_by_id: HashMap<LockupIndex, Lockup> = self
                .internal_get_account_lockups_by_id(
                    &account_id,
                    &amounts.iter().map(|x| x.0).collect(),
                )
                .into_iter()
                .collect();
            let amounts: HashMap<LockupIndex, NearToken> = amounts
                .into_iter()
                .map(|(lockup_id, amount)| {
                    (
                        lockup_id,
                        match amount {
                            Some(amount) => amount,
                            None => {
                                let lockup =
                                    lockups_by_id.get(&lockup_id).expect("lockup not found");
                                let unlocked_balance =
                                    lockup.schedule.unlocked_balance(current_timestamp_sec());
                                unlocked_balance.saturating_sub(lockup.claimed_balance)
                            }
                        },
                    )
                })
                .collect();
            (amounts, lockups_by_id)
        } else {
            let lockups_by_id: HashMap<LockupIndex, Lockup> = self
                .internal_get_account_lockups(&account_id)
                .into_iter()
                .collect();
            let amounts: HashMap<LockupIndex, NearToken> = lockups_by_id
                .iter()
                .map(|(lockup_id, lockup)| {
                    let unlocked_balance =
                        lockup.schedule.unlocked_balance(current_timestamp_sec());
                    let amount: NearToken = unlocked_balance.saturating_sub(lockup.claimed_balance);

                    (*lockup_id, amount)
                })
                .collect();
            (amounts, lockups_by_id)
        };

        let account_id = env::predecessor_account_id();
        let mut lockup_claims = vec![];
        let mut total_claim_amount = 0;
        for (lockup_index, lockup_claim_amount) in claim_amounts {
            let lockup = lockups_by_id.get_mut(&lockup_index).unwrap();
            let lockup_claim = lockup.claim(lockup_index, lockup_claim_amount);

            if lockup_claim.claim_amount.as_yoctonear() > 0 {
                log!(
                    "Claiming {} form lockup #{}",
                    lockup_claim.claim_amount,
                    lockup_index
                );
                total_claim_amount += lockup_claim.claim_amount.as_yoctonear();
                self.lockups.replace(lockup_index, lockup);
                lockup_claims.push(lockup_claim);
            }
        }
        log!("Total claim {}", total_claim_amount);

        if total_claim_amount > 0 {
            PromiseOrValue::from(
                Promise::new(self.token_account_id.clone())
                    .function_call(
                        "ft_transfer".to_string(),
                        serde_json::json!({
                            "receiver_id": account_id,
                            "amount": U128::from(total_claim_amount),
                            "memo": Some(format!(
                                "Claiming unlocked {} balance from {}",
                                total_claim_amount,
                                env::current_account_id()
                            ))
                        })
                        .to_string()
                        .into_bytes(),
                        ONE_YOCTO,
                        GAS_FOR_FT_TRANSFER,
                    )
                    .then(
                        callbacks::callbacks::ext(env::current_account_id())
                            .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                            .after_ft_transfer(account_id, lockup_claims),
                    ),
            )
        } else {
            PromiseOrValue::Value(ZERO_NEAR)
        }
    }

    #[payable]
    pub fn terminate(
        &mut self,
        lockup_index: LockupIndex,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: Option<U128>,
    ) -> PromiseOrValue<NearToken> {
        assert_one_yocto();
        self.assert_deposit_allowlist(&env::predecessor_account_id());
        let mut lockup = self
            .lockups
            .get(lockup_index as _)
            .expect("Lockup not found");
        let current_timestamp = current_timestamp_sec();
        let termination_timestamp = termination_timestamp.unwrap_or(current_timestamp);
        assert!(
            termination_timestamp >= current_timestamp,
            "expected termination_timestamp >= now",
        );
        let (unvested_balance, beneficiary_id) =
            lockup.terminate(hashed_schedule, termination_timestamp);
        self.lockups.replace(lockup_index as _, &lockup);

        // no need to store empty lockup
        if lockup.schedule.total_balance() == ZERO_NEAR {
            let lockup_account_id: AccountId = lockup.account_id;
            let mut indices = self
                .account_lockups
                .get(&lockup_account_id)
                .unwrap_or(UnorderedSet::new(StorageKey::AccountLockups));
            indices.remove(&lockup_index);
            self.internal_save_account_lockups(&lockup_account_id, indices);
        }

        FtLockupTerminateLockup {
            id: lockup_index,
            termination_timestamp,
            unvested_balance,
        }
        .emit();

        if unvested_balance.as_yoctonear() > 0 {
            PromiseOrValue::from(
                Promise::new(self.token_account_id.clone())
                    .function_call(
                        "ft_transfer".to_string(),
                        serde_json::json!({
                            "receiver_id": beneficiary_id,
                            "amount": unvested_balance,
                            "memo": Some(format!("Terminated lockup #{}", lockup_index))
                        })
                        .to_string()
                        .into_bytes(),
                        ONE_YOCTO,
                        GAS_FOR_FT_TRANSFER,
                    )
                    .then(
                        callbacks::callbacks::ext(env::current_account_id())
                            .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                            .after_lockup_termination(beneficiary_id, unvested_balance),
                    ),
            )
        } else {
            PromiseOrValue::Value(ZERO_NEAR)
        }
    }

    // preserving both options for API compatibility
    #[payable]
    pub fn add_to_deposit_allowlist(
        &mut self,
        account_id: Option<AccountId>,
        account_ids: Option<Vec<AccountId>>,
    ) {
        assert_one_yocto();
        self.assert_deposit_allowlist(&env::predecessor_account_id());
        let account_ids = if let Some(account_ids) = account_ids {
            account_ids
        } else {
            vec![account_id.expect("expected either account_id or account_ids")]
        };
        for account_id in &account_ids {
            self.deposit_allowlist.insert(account_id);
        }
        FtLockupAddToDepositAllowlist { account_ids }.emit()
    }

    // preserving both options for API compatibility
    #[payable]
    pub fn remove_from_deposit_allowlist(
        &mut self,
        account_id: Option<AccountId>,
        account_ids: Option<Vec<AccountId>>,
    ) {
        assert_one_yocto();
        self.assert_deposit_allowlist(&env::predecessor_account_id());
        let account_ids = if let Some(account_ids) = account_ids {
            account_ids
        } else {
            vec![account_id.expect("expected either account_id or account_ids")]
        };
        for account_id in &account_ids {
            self.deposit_allowlist.remove(account_id);
        }
        assert!(
            !self.deposit_allowlist.is_empty(),
            "cannot remove all accounts from deposit allowlist",
        );
        FtLockupRemoveFromDepositAllowlist { account_ids }.emit()
    }
}
