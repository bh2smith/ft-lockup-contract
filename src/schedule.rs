use crate::util::ZERO_NEAR;
use near_sdk::{env, json_types::U128, near, require, CryptoHash, NearToken};

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq)]
pub struct Checkpoint {
    /// The unix-timestamp in seconds since the epoch.
    pub timestamp: u128,
    pub balance: NearToken,
}

#[near(serializers = [borsh, json])]
#[derive(Debug, PartialEq, Clone)]
pub struct Schedule(pub Vec<Checkpoint>);

impl Schedule {
    pub fn new_zero_balance_from_to(start_timestamp: U128, finish_timestamp: U128) -> Self {
        require!(finish_timestamp > start_timestamp, "Invariant");

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
        require!(timestamp.0 > 0, "Invariant");
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

    pub fn new_unlocked(total_balance: NearToken) -> Self {
        Self::new_unlocked_since(total_balance, 1.into())
    }

    pub fn assert_valid(&self, total_balance: NearToken) {
        require!(self.0.len() >= 2, "at least two checkpoints are required");
        assert_eq!(
            self.0.first().unwrap().balance,
            ZERO_NEAR,
            "first checkpoint balance must be 0"
        );
        for i in 1..self.0.len() {
            require!(self.0[i - 1].timestamp < self.0[i].timestamp, format!("The timestamp of checkpoint #{} should be less than the timestamp of the next checkpoint", i - 1));
            require!(self.0[i - 1].balance <= self.0[i].balance, format!("The balance of checkpoint #{} should be not greater than the balance of the next checkpoint", i - 1));
        }
        require!(
            self.total_balance() > ZERO_NEAR,
            "total balance must be positive",
        );
        require!(
            self.total_balance() == total_balance,
            "expected total balance doesn't match transferred balance"
        );
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

    pub fn hash(&self) -> CryptoHash {
        let value_hash = env::sha256(borsh::to_vec(&self.0).unwrap().as_slice());
        let mut res = CryptoHash::default();
        res.copy_from_slice(&value_hash);
        res
    }

    /// Terminates the lockup schedule earlier.
    /// Assumes new_total_balance is not greater than the current total balance.
    /// This method is unaware of the vested tokens.
    /// External logic is responsible for preserving vested tokens!
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
            // Note that the zero balance schedule is technically "invalid" (via assert_valid)
            self.0 = Self::new_zero_balance_from_to(start_timestamp, finish_timestamp).0;
            return;
        }
        require!(
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
                require!(
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
    }

    /// Verifies that this schedule is ahead of the given termination schedule at any point of time.
    /// Assumes they have equal total balance and both schedules are valid.
    pub fn assert_valid_termination_schedule(&self, termination_schedule: &Schedule) {
        for checkpoint in &self.0 {
            require!(
                checkpoint.balance
                    <= termination_schedule.unlocked_balance(checkpoint.timestamp.into()),
                format!(
                    "The lockup schedule is ahead of the termination schedule at timestamp {}",
                    checkpoint.timestamp
                )
            );
        }
        for checkpoint in &termination_schedule.0 {
            require!(
                checkpoint.balance >= self.unlocked_balance(checkpoint.timestamp.into()),
                format!(
                    "The termination schedule is ahead of the lockup schedule at timestamp {}",
                    checkpoint.timestamp
                )
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const ONE_NEAR: NearToken = NearToken::from_near(1);

    #[test]
    fn test_new_happy_paths() {
        // new_zero_balance_from_to
        let s = Schedule::new_zero_balance_from_to(1.into(), 2.into());
        assert_eq!(
            s.0,
            vec![
                Checkpoint {
                    timestamp: 1,
                    balance: ZERO_NEAR
                },
                Checkpoint {
                    timestamp: 2,
                    balance: ZERO_NEAR
                }
            ]
        );

        // new_unlocked_since
        let s = Schedule::new_unlocked_since(ONE_NEAR, 2.into());
        s.assert_valid(ONE_NEAR);
        assert_eq!(
            s.0,
            vec![
                Checkpoint {
                    timestamp: 1,
                    balance: ZERO_NEAR
                },
                Checkpoint {
                    timestamp: 2,
                    balance: ONE_NEAR
                }
            ]
        );

        // new_unlocked
        let s = Schedule::new_unlocked(ONE_NEAR);
        s.assert_valid(ONE_NEAR);
        assert_eq!(
            s.0,
            vec![
                Checkpoint {
                    timestamp: 0,
                    balance: ZERO_NEAR
                },
                Checkpoint {
                    timestamp: 1,
                    balance: ONE_NEAR
                }
            ]
        );
    }

    #[test]
    fn test_hash() {
        assert_eq!(
            Schedule::new_zero_balance_from_to(1.into(), 2.into()).hash(),
            CryptoHash::from([
                168, 164, 240, 83, 54, 140, 1, 48, 183, 69, 219, 112, 104, 138, 134, 92, 20, 112,
                208, 172, 156, 163, 209, 3, 237, 87, 150, 161, 233, 181, 121, 157
            ])
        );

        assert_eq!(
            Schedule::new_unlocked_since(ONE_NEAR, 2.into()).hash(),
            CryptoHash::from([
                204, 53, 93, 162, 50, 151, 41, 9, 233, 242, 255, 116, 241, 160, 255, 101, 195, 216,
                169, 137, 123, 61, 196, 108, 81, 33, 151, 90, 226, 233, 207, 94
            ])
        );

        assert_eq!(
            Schedule::new_unlocked(ONE_NEAR).hash(),
            CryptoHash::from([
                19, 192, 155, 98, 188, 217, 56, 51, 184, 154, 37, 171, 141, 221, 211, 25, 193, 50,
                133, 253, 55, 5, 231, 67, 100, 77, 139, 148, 174, 43, 12, 182
            ])
        );
    }

    #[test]
    fn test_assert_valid_success() {
        let two_near = NearToken::from_near(2);
        let schedule = Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 10,
                balance: ONE_NEAR,
            },
            Checkpoint {
                timestamp: 20,
                balance: NearToken::from_near(2),
            },
        ]);
        schedule.assert_valid(two_near)
    }

    #[test]
    #[should_panic = "The timestamp of checkpoint #0 should be less than the timestamp of the next checkpoint"]
    fn test_assert_valid_fail_increasing_time() {
        let schedule = Schedule(vec![
            Checkpoint {
                timestamp: 1,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 0,
                balance: ONE_NEAR,
            },
        ]);
        schedule.assert_valid(ONE_NEAR)
    }

    #[test]
    #[should_panic = "The balance of checkpoint #1 should be not greater than the balance of the next checkpoint"]
    fn test_assert_valid_fail_increasing_balance() {
        let schedule = Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 1,
                balance: NearToken::from_near(2),
            },
            Checkpoint {
                timestamp: 2,
                balance: ONE_NEAR,
            },
        ]);
        schedule.assert_valid(ONE_NEAR)
    }

    #[test]
    #[should_panic = "at least two checkpoints are required"]
    fn test_assert_valid_fail_num_checkpoints() {
        Schedule(vec![Checkpoint {
            timestamp: 0,
            balance: ZERO_NEAR,
        }])
        .assert_valid(ZERO_NEAR)
    }

    #[test]
    #[should_panic = "first checkpoint balance must be 0"]
    fn test_assert_valid_fail_zero_start() {
        Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: ONE_NEAR,
            },
            Checkpoint {
                timestamp: 0,
                balance: ONE_NEAR,
            },
        ])
        .assert_valid(ZERO_NEAR)
    }

    #[test]
    #[should_panic = "total balance must be positive"]
    fn test_assert_valid_fail_positive_total() {
        Schedule::new_zero_balance_from_to(1.into(), 2.into()).assert_valid(ZERO_NEAR)
    }

    #[test]
    #[should_panic = "expected total balance doesn't match transferred balance"]
    fn test_assert_valid_fail_total_balance() {
        Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 1,
                balance: ONE_NEAR,
            },
        ])
        .assert_valid(ZERO_NEAR)
    }

    #[test]
    fn test_unlocked_balance() {
        // Simple linear vesting between two checkpoints.
        let now = 100;
        let two_near = NearToken::from_near(2);
        let s = Schedule(vec![
            Checkpoint {
                timestamp: now - 50,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: now + 50,
                balance: two_near,
            },
        ]);
        assert_eq!(
            s.unlocked_balance(75.into()),
            NearToken::from_yoctonear(ONE_NEAR.as_yoctonear() / 2)
        );
        assert_eq!(s.unlocked_balance(now.into()), ONE_NEAR);
        // SLightly more complex example.
        let s = Schedule(vec![
            Checkpoint {
                timestamp: 50,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 100,
                balance: two_near,
            },
            Checkpoint {
                timestamp: 200,
                balance: NearToken::from_near(4),
            },
        ]);
        assert_eq!(s.unlocked_balance(50.into()), ZERO_NEAR);
        assert_eq!(s.unlocked_balance(100.into()), two_near);
        assert_eq!(s.unlocked_balance(150.into()), NearToken::from_near(3));
        assert_eq!(s.unlocked_balance(200.into()), NearToken::from_near(4));
    }

    #[test]
    fn test_termination() {
        let two_near = NearToken::from_near(2);
        let four_near = NearToken::from_near(4);
        let mut s = Schedule(vec![
            Checkpoint {
                timestamp: 50,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 100,
                balance: two_near,
            },
            Checkpoint {
                timestamp: 200,
                balance: four_near,
            },
        ]);
        s.terminate(ONE_NEAR, 100.into());
        assert_eq!(s.unlocked_balance(50.into()), ZERO_NEAR);
        assert_eq!(s.unlocked_balance(100.into()), ONE_NEAR);
        assert_eq!(s.unlocked_balance(200.into()), ONE_NEAR);

        s.terminate(ZERO_NEAR, 100.into());
        assert_eq!(s.unlocked_balance(50.into()), ZERO_NEAR);
        assert_eq!(s.unlocked_balance(100.into()), ZERO_NEAR);
        assert_eq!(s.unlocked_balance(200.into()), ZERO_NEAR);

        s.terminate(ZERO_NEAR, 50.into());
    }

    #[test]
    fn test_valid_termination_schedule_passes() {
        let two_near = NearToken::from_near(2);
        let four_near = NearToken::from_near(4);
        let s = Schedule(vec![
            Checkpoint {
                timestamp: 50,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 100,
                balance: two_near,
            },
            Checkpoint {
                timestamp: 200,
                balance: four_near,
            },
        ]);
        s.assert_valid_termination_schedule(&Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 200,
                balance: four_near,
            },
        ]));
    }

    #[test]
    #[should_panic = "The lockup schedule is ahead of the termination schedule at timestamp 200"]
    fn test_valid_termination_schedule_panics() {
        let two_near = NearToken::from_near(2);
        let four_near = NearToken::from_near(4);
        let s = Schedule(vec![
            Checkpoint {
                timestamp: 50,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 100,
                balance: two_near,
            },
            Checkpoint {
                timestamp: 200,
                balance: four_near,
            },
        ]);
        s.assert_valid_termination_schedule(&Schedule(vec![
            Checkpoint {
                timestamp: 50,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 100,
                balance: two_near,
            },
        ]));
        s.assert_valid_termination_schedule(&s);
    }

    #[test]
    #[should_panic = "The lockup schedule is ahead of the termination schedule at timestamp 100"]
    fn test_valid_termination_schedule_lockup_ahead_of_termination() {
        let two_near = NearToken::from_near(2);
        let four_near = NearToken::from_near(4);
        let s = Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 100,
                balance: two_near,
            },
            Checkpoint {
                timestamp: 200,
                balance: two_near,
            },
            Checkpoint {
                timestamp: 300,
                balance: four_near,
            },
        ]);
        let termination_schedule = Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 300,
                balance: four_near,
            },
        ]);
        s.assert_valid_termination_schedule(&termination_schedule);
    }

    #[test]
    #[should_panic = "The termination schedule is ahead of the lockup schedule at timestamp 200"]
    fn test_valid_termination_schedule_panics_case_b() {
        let two_near = NearToken::from_near(2);
        let four_near = NearToken::from_near(4);
        let termination_schedule = Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 100,
                balance: two_near,
            },
            Checkpoint {
                timestamp: 200,
                balance: two_near,
            },
            Checkpoint {
                timestamp: 300,
                balance: four_near,
            },
        ]);
        let s = Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: 300,
                balance: four_near,
            },
        ]);
        s.assert_valid_termination_schedule(&termination_schedule);
    }
}
