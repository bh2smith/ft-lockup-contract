use crate::{
    schedule::Schedule,
    termination::{TerminationConfig, VestingConditions},
    util::{current_timestamp_sec, ZERO_NEAR},
};
use near_sdk::{json_types::U128, near, require, AccountId, NearToken};

pub type LockupIndex = u64;

#[near(serializers = [borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub struct LockupClaim {
    pub index: LockupIndex,
    pub claim_amount: NearToken,
    pub is_final: bool,
}

#[near(serializers = [borsh, json])]
#[derive(Debug, PartialEq, Clone)]
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
        let lockup = Self {
            account_id,
            schedule: Schedule::new_unlocked_since(total_balance, timestamp),
            claimed_balance: NearToken::from_near(0),
            termination_config: None,
        };
        // Always validate before construction.
        lockup.assert_valid(total_balance);
        lockup
    }

    pub fn claim(&mut self, index: LockupIndex, claim_amount: NearToken) -> LockupClaim {
        let unlocked_balance = self.schedule.unlocked_balance(current_timestamp_sec());
        let balance_claimed_new = self
            .claimed_balance
            .checked_add(claim_amount)
            .expect("attempt to add with overflow");
        require!(
            unlocked_balance >= balance_claimed_new,
            format!("too big claim_amount for lockup {}", index)
        );

        self.claimed_balance = balance_claimed_new;
        LockupClaim {
            // TODO: This index field is not relevant to lockup (is purley for external users)
            index,
            claim_amount,
            is_final: balance_claimed_new == self.schedule.total_balance(),
        }
    }

    pub fn assert_valid(&self, total_balance: NearToken) {
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

    pub fn into_lockup(&self, payer_id: &AccountId) -> Lockup {
        let vesting_schedule = self.vesting_schedule.clone();
        let lockup = Lockup {
            account_id: self.account_id.clone(),
            schedule: self.schedule.clone(),
            claimed_balance: ZERO_NEAR,
            termination_config: vesting_schedule.map(|vesting_schedule| TerminationConfig {
                beneficiary_id: payer_id.clone(),
                vesting_schedule,
            }),
        };
        lockup.assert_valid(lockup.schedule.total_balance());
        lockup
    }
}

#[cfg(test)]
mod tests {

    use near_sdk::serde_json;

    use crate::ONE_YOCTO;

    use super::*;

    #[test]
    fn test_lockup_new_unlocked_since() {
        let account_id: AccountId = "x.near".parse().unwrap();
        let total_balance = ONE_YOCTO;
        let timestamp = U128(1);
        let lockup = Lockup::new_unlocked_since(account_id.clone(), total_balance, timestamp);
        assert_eq!(
            lockup,
            Lockup {
                account_id,
                schedule: Schedule::new_unlocked_since(total_balance, timestamp),
                claimed_balance: ZERO_NEAR,
                termination_config: None
            }
        );
        // Bonus check validity.
        lockup.assert_valid(total_balance);
    }

    #[test]
    fn test_lockup_claim_success() {
        let account_id: AccountId = "x.near".parse().unwrap();
        let total_balance = ONE_YOCTO;
        let timestamp = U128(1);
        let mut lockup = Lockup::new_unlocked_since(account_id.clone(), total_balance, timestamp);
        let claim = lockup.claim(0, ZERO_NEAR);
        assert_eq!(
            claim,
            LockupClaim {
                index: 0,
                claim_amount: ZERO_NEAR,
                is_final: false
            }
        );
    }

    #[test]
    #[should_panic = "too big claim_amount for lockup 0"]
    fn test_lockup_claim_fails_too_early() {
        let account_id: AccountId = "x.near".parse().unwrap();
        let total_balance = ONE_YOCTO;
        let timestamp = U128(1);
        let mut lockup = Lockup::new_unlocked_since(account_id.clone(), total_balance, timestamp);
        lockup.claim(0, ONE_YOCTO);
    }

    // TODO - test lockup.claim
    // #[test]
    // #[should_panic = "attempt to add with overflow"]
    // fn test_lockup_claim_fails_overflow() {
    //     let account_id: AccountId = "x.near".parse().unwrap();
    //     let total_balance = ONE_YOCTO;
    //     let timestamp = U128(1);
    //     // TODO - move time forward in near_sdk::env
    //     let mut lockup = Lockup::new_unlocked_since(account_id.clone(), total_balance, timestamp);
    //     lockup.claim(0, ONE_YOCTO);
    // }

    #[test]
    #[should_panic = "The initial lockup claimed balance should be 0"]
    fn test_assert_valid_fails_initial_claimed() {
        let total_balance = ONE_YOCTO;
        let timestamp = U128(1);
        let lockup = Lockup {
            account_id: "x.near".parse().unwrap(),
            schedule: Schedule::new_unlocked_since(total_balance, timestamp),
            claimed_balance: NearToken::from_near(1),
            termination_config: None,
        };
        lockup.assert_valid(total_balance)
    }

    #[test]
    fn test_assert_valid_hashed_schedule() {
        let account_id: AccountId = "x.near".parse().unwrap();
        let total_balance = ONE_YOCTO;
        let timestamp = U128(1);
        let schedule = Schedule::new_unlocked_since(total_balance, timestamp);
        let lockup = Lockup {
            account_id: account_id.clone(),
            schedule: schedule.clone(),
            claimed_balance: ZERO_NEAR,
            termination_config: Some(TerminationConfig {
                beneficiary_id: account_id,
                vesting_schedule: VestingConditions::Hash(schedule.hash().into()),
            }),
        };
        lockup.assert_valid(total_balance)
    }

    #[test]
    fn test_assert_valid_alt_schedule() {
        let account_id: AccountId = "x.near".parse().unwrap();
        let total_balance = ONE_YOCTO;
        let timestamp = U128(1);
        let schedule = Schedule::new_unlocked_since(total_balance, timestamp);
        let lockup = Lockup {
            account_id: account_id.clone(),
            schedule: schedule.clone(),
            claimed_balance: ZERO_NEAR,
            termination_config: Some(TerminationConfig {
                beneficiary_id: account_id,
                vesting_schedule: VestingConditions::Schedule(schedule),
            }),
        };
        lockup.assert_valid(total_balance)
    }

    #[test]
    fn test_lockup_create_into_lockup() {
        // env is working on a fresh blockchain starting from time 0
        let account_id: AccountId = "x.near".parse().unwrap();
        let beneficiary_id: AccountId = "p.near".parse().unwrap();
        let total_balance = ONE_YOCTO;
        let timestamp = U128(1);
        let schedule = Schedule::new_unlocked_since(total_balance, timestamp);
        let lockup_create = LockupCreate {
            account_id: account_id.clone(),
            schedule: schedule.clone(),
            vesting_schedule: Some(VestingConditions::SameAsLockupSchedule),
        };
        let lockup = lockup_create.into_lockup(&beneficiary_id);
        assert_eq!(
            lockup,
            Lockup {
                account_id,
                schedule,
                claimed_balance: ZERO_NEAR,
                termination_config: Some(TerminationConfig {
                    beneficiary_id,
                    vesting_schedule: VestingConditions::SameAsLockupSchedule
                })
            }
        );
    }

    #[test]
    fn test_serialization_lockup_create() {
        let account_id: AccountId = "x.near".parse().unwrap();
        let total_balance = ONE_YOCTO;
        let timestamp = U128(1);
        let lockup_create = LockupCreate {
            account_id: account_id.clone(),
            schedule: Schedule::new_unlocked_since(total_balance, timestamp),
            vesting_schedule: Some(VestingConditions::SameAsLockupSchedule),
        };

        // Serialize to JSON
        let json_data = serde_json::to_string(&lockup_create).unwrap();
        // Deserialize from JSON
        let deserialized_json: LockupCreate = serde_json::from_str(&json_data).unwrap();

        // Serialize to BORSH
        let borsh_data = borsh::to_vec(&lockup_create).unwrap();
        // Deserialize from BORSH
        let deserialized_borsh: LockupCreate =
            borsh::BorshDeserialize::try_from_slice(&borsh_data).unwrap();

        // Assertions
        assert_eq!(
            lockup_create, deserialized_json,
            "JSON serialization failed"
        );
        assert_eq!(
            lockup_create, deserialized_borsh,
            "BORSH serialization failed"
        );
    }
}
