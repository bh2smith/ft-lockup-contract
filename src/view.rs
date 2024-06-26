use crate::{
    lockup::{Lockup, LockupCreate, LockupIndex},
    schedule::Schedule,
    termination::{TerminationConfig, VestingConditions},
    util::{current_timestamp_sec, ZERO_NEAR},
    Contract, ContractExt, VERSION,
};
use near_sdk::{
    json_types::{Base58CryptoHash, U128},
    near, AccountId, NearToken,
};

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
    pub fn get_token_id(&self) -> AccountId {
        self.token_id.clone()
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

    pub fn get_deposit_allowlist(&self) -> Vec<AccountId> {
        self.deposit_allowlist.to_vec()
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

#[cfg(test)]
mod tests {
    use crate::Checkpoint;

    use super::*;

    #[test]
    fn test_nano_to_sec() {
        let account_id = "x.near".parse().unwrap();
        let amount = NearToken::from_near(10_000);
        let schedule = Schedule(vec![
            Checkpoint {
                timestamp: 100,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 200,
                balance: amount,
            },
        ]);
        schedule.assert_valid(amount);
        let lockup_create = LockupCreate {
            account_id,
            schedule: schedule.clone(),
            vesting_schedule: None,
        };
        // let lockup = lockup_create.into_lockup(&"y.near".parse().unwrap());
        let lockup_view = LockupCreateView::from(lockup_create);
        assert_eq!(lockup_view.total_balance, amount);
        assert_eq!(lockup_view.claimed_balance, ZERO_NEAR);
        assert_eq!(lockup_view.unclaimed_balance, ZERO_NEAR);
    }
}
