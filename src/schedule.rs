use crate::*;
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq)]
pub struct Checkpoint {
    // TODO: USE BLOCK HEIGHT INSTEAD!
    /// The unix-timestamp in seconds since the epoch.
    pub timestamp: u128,
    pub balance: NearToken,
}

#[near(serializers = [borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub struct Schedule(pub Vec<Checkpoint>);

impl Schedule {
    pub fn new_zero_balance_from_to(start_timestamp: U128, finish_timestamp: U128) -> Self {
        assert!(finish_timestamp > start_timestamp, "Invariant");

        Self(vec![
            Checkpoint {
                timestamp: start_timestamp.0,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: finish_timestamp.0,
                balance: ZERO_NEAR,
            },
        ])
    }

    pub fn new_unlocked_since(total_balance: NearToken, timestamp: U128) -> Self {
        assert!(timestamp.0 > 0, "Invariant");
        Self(vec![
            Checkpoint {
                timestamp: timestamp.0 - 1,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: timestamp.0,
                balance: total_balance,
            },
        ])
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_unlocked(total_balance: NearToken) -> Self {
        Self::new_unlocked_since(total_balance, 1.into())
    }

    pub fn assert_valid(&self, total_balance: NearToken) {
        assert!(self.0.len() >= 2, "At least two checkpoints is required");
        assert_eq!(
            self.0.first().unwrap().balance,
            ZERO_NEAR,
            "The first checkpoint balance should be 0"
        );
        for i in 1..self.0.len() {
            assert!(self.0[i - 1].timestamp < self.0[i].timestamp, "The timestamp of checkpoint #{} should be less than the timestamp of the next checkpoint", i - 1);
            assert!(self.0[i - 1].balance <= self.0[i].balance, "The balance of checkpoint #{} should be not greater than the balance of the next checkpoint", i - 1);
        }
        assert!(
            self.total_balance() > ZERO_NEAR,
            "expected total balance to be positive",
        );
        assert_eq!(
            self.total_balance(),
            total_balance,
            "The schedule's total balance doesn't match the transferred balance"
        );
    }

    /// Verifies that this schedule is ahead of the given termination schedule at any point of time.
    /// Assumes they have equal total balance and both schedules are valid.
    pub fn assert_valid_termination_schedule(&self, termination_schedule: &Schedule) {
        for checkpoint in &self.0 {
            assert!(
                checkpoint.balance
                    <= termination_schedule.unlocked_balance(checkpoint.timestamp.into()),
                "The lockup schedule is ahead of the termination schedule at timestamp {}",
                checkpoint.timestamp
            );
        }
        for checkpoint in &termination_schedule.0 {
            assert!(
                checkpoint.balance >= self.unlocked_balance(checkpoint.timestamp.into()),
                "The lockup schedule is ahead of the termination schedule at timestamp {}",
                checkpoint.timestamp
            );
        }
    }

    pub fn unlocked_balance(&self, current_timestamp: U128) -> NearToken {
        // Using binary search by time to find the current checkpoint.
        let index = match self
            .0
            .binary_search_by_key(&current_timestamp, |checkpoint| checkpoint.timestamp.into())
        {
            // Exact timestamp found
            Ok(index) => index,
            // No match, the next index is given.
            Err(index) => {
                if index == 0 {
                    // Not started
                    return ZERO_NEAR;
                }
                index - 1
            }
        };
        let checkpoint = &self.0[index];
        if index + 1 == self.0.len() {
            // The last checkpoint. Fully unlocked.
            return checkpoint.balance;
        }
        let next_checkpoint = &self.0[index + 1];

        let total_duration = next_checkpoint.timestamp - checkpoint.timestamp;
        let passed_duration = current_timestamp.0 - checkpoint.timestamp;
        let vested = checkpoint.balance.as_yoctonear()
            + passed_duration
                * (next_checkpoint.balance.as_yoctonear() - checkpoint.balance.as_yoctonear())
                / total_duration;
        NearToken::from_yoctonear(vested)
    }

    pub fn total_balance(&self) -> NearToken {
        self.0.last().unwrap().balance
    }

    /// Terminates the lockup schedule earlier.
    /// Assumes new_total_balance is not greater than the current total balance.
    pub fn terminate(&mut self, new_total_balance: NearToken, finish_timestamp: U128) {
        if new_total_balance == ZERO_NEAR {
            // finish_timestamp is a hint, only used for fully unvested schedules
            // can be overwritten to preserve schedule invariants
            // used to preserve part of the schedule before the termination happens
            let start_timestamp: U128 = self.0[0].timestamp.into();
            let finish_timestamp = if finish_timestamp.0 > start_timestamp.0 {
                finish_timestamp
            } else {
                (start_timestamp.0 + 1).into()
            };
            self.0 = Self::new_zero_balance_from_to(start_timestamp, finish_timestamp).0;
            return;
        }
        assert!(
            new_total_balance <= self.0.last().unwrap().balance,
            "Invariant"
        );
        while let Some(checkpoint) = self.0.pop() {
            if self.0.last().unwrap().balance < new_total_balance {
                let prev_checkpoint = self.0.last().unwrap().clone();
                let timestamp_diff = checkpoint.timestamp - prev_checkpoint.timestamp;
                let balance_diff =
                    checkpoint.balance.as_yoctonear() - prev_checkpoint.balance.as_yoctonear();
                let required_balance_diff =
                    new_total_balance.as_yoctonear() - prev_checkpoint.balance.as_yoctonear();
                // Computing the new timestamp rounding up
                let new_timestamp = prev_checkpoint.timestamp
                    + ((timestamp_diff * required_balance_diff + (balance_diff - 1))
                        / balance_diff);
                // Ensure this funky math can be cast back to u64:
                assert!(
                    new_timestamp <= u64::MAX as u128,
                    "timestamp arithmetic mixed with balances"
                );
                self.0.push(Checkpoint {
                    timestamp: new_timestamp,
                    balance: new_total_balance,
                });
                return;
            }
        }
        unreachable!();
    }
    pub fn hash(&self) -> CryptoHash {
        let value_hash = env::sha256(borsh::to_vec(&self.0).unwrap().as_slice());
        let mut res = CryptoHash::default();
        res.copy_from_slice(&value_hash);

        res
    }
}
