use crate::*;

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
        termination_timestamp: TimestampSec,
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
