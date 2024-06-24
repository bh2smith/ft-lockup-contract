use crate::{lockup::Lockup, schedule::Schedule, util::ZERO_NEAR};
use near_sdk::{
    json_types::{Base58CryptoHash, U128},
    near, AccountId, CryptoHash, NearToken,
};

#[near(serializers = [borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub enum VestingConditions {
    SameAsLockupSchedule,
    Hash(Base58CryptoHash),
    Schedule(Schedule),
}

#[near(serializers = [borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub struct TerminationConfig {
    /// The account ID who paid for the lockup creation
    /// and will receive unvested balance upon termination
    pub beneficiary_id: AccountId,
    /// An optional vesting schedule
    pub vesting_schedule: VestingConditions,
}

impl Lockup {
    pub fn terminate(
        &mut self,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: U128,
    ) -> (NearToken, AccountId) {
        let termination_config = self
            .termination_config
            .take()
            .expect("No termination config");
        let total_balance = self.schedule.total_balance();
        let vested_balance = match &termination_config.vesting_schedule {
            VestingConditions::SameAsLockupSchedule => &self.schedule,
            VestingConditions::Hash(hash) => {
                let schedule = hashed_schedule
                    .as_ref()
                    .expect("Revealed schedule required for the termination");
                let hash: CryptoHash = (*hash).into();
                assert_eq!(
                    hash,
                    schedule.hash(),
                    "The revealed schedule hash doesn't match"
                );
                schedule.assert_valid(total_balance);
                self.schedule.assert_valid_termination_schedule(schedule);
                schedule
            }
            VestingConditions::Schedule(schedule) => schedule,
        }
        .unlocked_balance(termination_timestamp);
        let unvested_balance = total_balance.saturating_sub(vested_balance);
        if unvested_balance > ZERO_NEAR {
            self.schedule
                .terminate(vested_balance, termination_timestamp);
        }
        (unvested_balance, termination_config.beneficiary_id)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_terminate() {
        let account_id: AccountId = "x.near".parse().unwrap();
        let total_balance = NearToken::from_near(1);
        let timestamp = U128(1);
        let schedule = Schedule::new_unlocked_since(total_balance, timestamp);
        let mut lockup = Lockup {
            account_id: account_id.clone(),
            schedule: schedule.clone(),
            claimed_balance: ZERO_NEAR,
            termination_config: Some(TerminationConfig {
                beneficiary_id: account_id.clone(),
                vesting_schedule: VestingConditions::SameAsLockupSchedule,
            }),
        };

        let (unvested_amount, beneficiary) = lockup.terminate(None, timestamp);
        assert_eq!(unvested_amount.as_yoctonear(), 0);
        assert_eq!(beneficiary, account_id);
    }

    #[test]
    #[should_panic = "No termination config"]
    fn test_terminate_fail() {
        let account_id: AccountId = "x.near".parse().unwrap();
        let total_balance = NearToken::from_near(1);
        let timestamp = U128(1);
        let schedule = Schedule::new_unlocked_since(total_balance, timestamp);

        let mut lockup = Lockup {
            account_id: account_id.clone(),
            schedule: schedule.clone(),
            claimed_balance: ZERO_NEAR,
            termination_config: None,
        };

        lockup.terminate(None, timestamp);
    }

    #[test]
    #[should_panic = "Revealed schedule required for the termination"]
    fn test_terminate_hashed_without_schedule() {
        let account_id: AccountId = "x.near".parse().unwrap();
        let total_balance = NearToken::from_near(1);
        let timestamp = U128(1);
        let schedule = Schedule::new_unlocked_since(total_balance, timestamp);

        let mut lockup = Lockup {
            account_id: account_id.clone(),
            schedule: schedule.clone(),
            claimed_balance: ZERO_NEAR,
            termination_config: Some(TerminationConfig {
                beneficiary_id: account_id.clone(),
                vesting_schedule: VestingConditions::Hash(schedule.hash().into()),
            }),
        };

        lockup.terminate(None, timestamp);
    }

    #[test]
    fn test_terminate_hashed_with_schedule() {
        let account_id: AccountId = "x.near".parse().unwrap();
        let total_balance = NearToken::from_near(1);
        let timestamp = U128(1);
        let schedule = Schedule::new_unlocked_since(total_balance, timestamp);

        let mut lockup = Lockup {
            account_id: account_id.clone(),
            schedule: schedule.clone(),
            claimed_balance: ZERO_NEAR,
            termination_config: Some(TerminationConfig {
                beneficiary_id: account_id.clone(),
                vesting_schedule: VestingConditions::Hash(schedule.hash().into()),
            }),
        };

        let (unvested_amount, beneficiary) = lockup.terminate(Some(schedule), timestamp);
        assert_eq!(unvested_amount.as_yoctonear(), 0);
        assert_eq!(beneficiary, account_id);
    }

    #[test]
    fn test_terminate_with_schedule_vesting_conditions() {
        let account_id: AccountId = "x.near".parse().unwrap();
        let total_balance = NearToken::from_near(1);
        let timestamp = U128(1);
        let schedule = Schedule::new_unlocked_since(total_balance, timestamp);

        let mut lockup = Lockup {
            account_id: account_id.clone(),
            schedule: schedule.clone(),
            claimed_balance: ZERO_NEAR,
            termination_config: Some(TerminationConfig {
                beneficiary_id: account_id.clone(),
                vesting_schedule: VestingConditions::Schedule(schedule),
            }),
        };

        let (unvested_amount, beneficiary) = lockup.terminate(None, timestamp);
        assert_eq!(unvested_amount.as_yoctonear(), 0);
        assert_eq!(beneficiary, account_id);
    }
}
