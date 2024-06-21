use crate::schedule::Schedule;
use crate::termination::{TerminationConfig, VestingConditions};
use crate::util::{current_timestamp_sec, ZERO_NEAR};
use near_sdk::json_types::U128;
use near_sdk::{near, AccountId, NearToken};

pub type LockupIndex = u64;

#[near(serializers = [borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub struct LockupClaim {
    pub index: LockupIndex,
    pub claim_amount: NearToken,
    pub is_final: bool,
}

#[near(serializers = [borsh, json])]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq, Clone))]
pub struct Lockup {
    pub account_id: AccountId,
    pub schedule: Schedule,
    pub claimed_balance: NearToken,
    /// An optional configuration that allows vesting/lockup termination.
    pub termination_config: Option<TerminationConfig>,
}

impl Lockup {
    pub fn new_unlocked_since(
        account_id: AccountId,
        total_balance: NearToken,
        timestamp: U128,
    ) -> Self {
        Self {
            account_id,
            schedule: Schedule::new_unlocked_since(total_balance, timestamp),
            claimed_balance: NearToken::from_near(0),
            termination_config: None,
        }
    }

    pub fn new_unlocked(account_id: AccountId, total_balance: NearToken) -> Self {
        Self::new_unlocked_since(account_id, total_balance, U128(1))
    }

    pub fn claim(&mut self, index: LockupIndex, claim_amount: NearToken) -> LockupClaim {
        let unlocked_balance = self.schedule.unlocked_balance(current_timestamp_sec());
        let balance_claimed_new = self
            .claimed_balance
            .checked_add(claim_amount)
            .expect("attempt to add with overflow");
        assert!(
            unlocked_balance >= balance_claimed_new,
            "too big claim_amount for lockup {}",
            index,
        );

        self.claimed_balance = balance_claimed_new;
        LockupClaim {
            index,
            claim_amount,
            is_final: balance_claimed_new == self.schedule.total_balance(),
        }
    }

    pub fn assert_new_valid(&self, total_balance: NearToken) {
        assert_eq!(
            self.claimed_balance, ZERO_NEAR,
            "The initial lockup claimed balance should be 0"
        );
        self.schedule.assert_valid(total_balance);

        if let Some(termination_config) = &self.termination_config {
            match &termination_config.vesting_schedule {
                VestingConditions::SameAsLockupSchedule => {
                    // Ok, using lockup schedule.
                }
                VestingConditions::Hash(_hash) => {
                    // Ok, using unknown hash. Can't verify.
                }
                VestingConditions::Schedule(schedule) => {
                    schedule.assert_valid(total_balance);
                    self.schedule.assert_valid_termination_schedule(schedule);
                }
            }
        }
    }
}

#[near(serializers = [borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub struct LockupCreate {
    pub account_id: AccountId,
    pub schedule: Schedule,
    pub vesting_schedule: Option<VestingConditions>,
}

impl LockupCreate {
    pub fn new_unlocked(account_id: AccountId, total_balance: NearToken) -> Self {
        Self {
            account_id,
            schedule: Schedule::new_unlocked(total_balance),
            vesting_schedule: None,
        }
    }
}

impl LockupCreate {
    pub fn into_lockup(&self, payer_id: &AccountId) -> Lockup {
        let vesting_schedule = self.vesting_schedule.clone();
        Lockup {
            account_id: self.account_id.clone(),
            schedule: self.schedule.clone(),
            claimed_balance: ZERO_NEAR,
            termination_config: vesting_schedule.map(|vesting_schedule| TerminationConfig {
                beneficiary_id: payer_id.clone(),
                vesting_schedule,
            }),
        }
    }
}
