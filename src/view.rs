use crate::lockup::{Lockup, LockupCreate, LockupIndex};
use crate::schedule::Schedule;
use crate::termination::{TerminationConfig, VestingConditions};
use crate::util::{current_timestamp_sec, ZERO_NEAR};
use crate::ContractExt;
use crate::{Contract, VERSION};
use near_sdk::json_types::{Base58CryptoHash, U128};
use near_sdk::{near, AccountId, NearToken};

#[near(serializers = [borsh, json])]
#[derive(Debug)]
pub struct LockupView {
    pub account_id: AccountId,
    pub schedule: Schedule,
    pub claimed_balance: NearToken,
    /// An optional configuration that allows vesting/lockup termination.
    pub termination_config: Option<TerminationConfig>,

    pub total_balance: NearToken,
    pub unclaimed_balance: NearToken,
    /// The current timestamp
    pub timestamp: U128,
}

impl From<Lockup> for LockupView {
    fn from(lockup: Lockup) -> Self {
        let total_balance = lockup.schedule.total_balance();
        let timestamp = current_timestamp_sec();
        let unclaimed_balance = lockup
            .schedule
            .unlocked_balance(timestamp)
            .saturating_sub(lockup.claimed_balance);
        let Lockup {
            account_id,
            schedule,
            claimed_balance,
            termination_config,
        } = lockup;
        Self {
            account_id,
            schedule,
            claimed_balance,
            termination_config,
            total_balance,
            unclaimed_balance,
            timestamp,
        }
    }
}

#[near(serializers = [borsh, json])]
pub struct LockupCreateView {
    pub account_id: AccountId,
    pub schedule: Schedule,
    pub vesting_schedule: Option<VestingConditions>,

    pub claimed_balance: NearToken,
    pub total_balance: NearToken,
    pub unclaimed_balance: NearToken,
    /// The current timestamp
    pub timestamp: U128,
}

impl From<LockupCreate> for LockupCreateView {
    fn from(lockup_create: LockupCreate) -> Self {
        let total_balance = lockup_create.schedule.total_balance();
        let timestamp = current_timestamp_sec();
        let unclaimed_balance = lockup_create.schedule.unlocked_balance(timestamp);
        let LockupCreate {
            account_id,
            schedule,
            vesting_schedule,
        } = lockup_create;
        Self {
            account_id,
            schedule,
            vesting_schedule,
            claimed_balance: ZERO_NEAR,
            total_balance,
            unclaimed_balance,
            timestamp,
        }
    }
}

#[near]
impl Contract {
    pub fn get_token_account_id(&self) -> AccountId {
        self.token_account_id.clone()
    }

    pub fn get_account_lockups(&self, account_id: AccountId) -> Vec<(LockupIndex, LockupView)> {
        self.internal_get_account_lockups(&account_id)
            .into_iter()
            .map(|(lockup_index, lockup)| (lockup_index, lockup.into()))
            .collect()
    }

    pub fn get_lockup(&self, index: LockupIndex) -> Option<LockupView> {
        self.lockups.get(index as _).map(|lockup| lockup.into())
    }

    pub fn get_lockups(&self, indices: Vec<LockupIndex>) -> Vec<(LockupIndex, LockupView)> {
        indices
            .into_iter()
            .filter_map(|index| self.get_lockup(index).map(|lockup| (index, lockup)))
            .collect()
    }

    pub fn get_num_lockups(&self) -> u64 {
        self.lockups.len() as _
    }

    pub fn get_lockups_paged(
        &self,
        from_index: Option<LockupIndex>,
        limit: Option<LockupIndex>,
    ) -> Vec<(LockupIndex, LockupView)> {
        let from_index = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(self.get_num_lockups());
        (from_index..std::cmp::min(self.get_num_lockups(), limit))
            .filter_map(|index| self.get_lockup(index).map(|lockup| (index, lockup)))
            .collect()
    }

    pub fn get_deposit_whitelist(&self) -> Vec<AccountId> {
        self.deposit_whitelist.to_vec()
    }

    pub fn hash_schedule(&self, schedule: Schedule) -> Base58CryptoHash {
        schedule.hash().into()
    }

    pub fn validate_schedule(
        &self,
        schedule: Schedule,
        total_balance: NearToken,
        termination_schedule: Option<Schedule>,
    ) {
        schedule.assert_valid(total_balance);
        if let Some(termination_schedule) = termination_schedule {
            termination_schedule.assert_valid(total_balance);
            schedule.assert_valid_termination_schedule(&termination_schedule);
        }
    }

    pub fn get_version(&self) -> String {
        VERSION.into()
    }
}
