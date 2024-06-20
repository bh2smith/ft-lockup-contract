mod setup;

use crate::setup::*;
use near_sdk::NearToken;

pub(crate) const ZERO_NEAR: NearToken = NearToken::from_near(0);

#[test]
fn test_lockup_claim_logic() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(10000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup_create = LockupCreate {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC - 1,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount,
            },
        ]),
        vesting_schedule: None,
    };
    let balance: NearToken = e.add_lockup(&e.owner, amount, &lockup_create).unwrap_json();
    assert_eq!(balance, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[0].1.unclaimed_balance, ZERO_NEAR);

    // Claim attempt before unlock.
    let res: NearToken = e.claim(&users.alice).unwrap_json();
    assert_eq!(res, ZERO_NEAR);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);

    // Set time to the first checkpoint.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC - 1);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[0].1.unclaimed_balance, ZERO_NEAR);

    // Set time to the second checkpoint.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[0].1.unclaimed_balance, amount);

    // Attempt to claim. No storage deposit for Alice.
    let res: NearToken = e.claim(&users.alice).unwrap_json();
    assert_eq!(res, ZERO_NEAR);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[0].1.unclaimed_balance, amount);

    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);

    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, ZERO_NEAR);

    // Claim tokens.
    let res: NearToken = e.claim(&users.alice).unwrap_json();
    assert_eq!(res, amount);
    // User's lockups should be empty, since fully claimed.
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // Manually checking the lockup by index
    let lockup = e.get_lockup(0);
    assert_eq!(lockup.claimed_balance, amount);
    assert_eq!(lockup.unclaimed_balance, ZERO_NEAR);

    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount);
}

#[test]
fn test_lockup_linear() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup_create = LockupCreate {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount,
            },
        ]),
        vesting_schedule: None,
    };
    let balance: NearToken = e.add_lockup(&e.owner, amount, &lockup_create).unwrap_json();
    assert_eq!(balance, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[0].1.unclaimed_balance, ZERO_NEAR);

    // 1/3 unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 3);

    // Claim tokens
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: NearToken = e.claim(&users.alice).unwrap_json();
    assert_eq!(res, amount / 3);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 3);

    // Check lockup after claim
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, ZERO_NEAR);

    // 1/2 unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 2);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 6);

    // Remove storage from token to verify claim refund.
    // Note, this burns `amount / 3` tokens.
    storage_force_unregister(&users.alice, TOKEN_ID);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, ZERO_NEAR);

    // Trying to claim, should fail and refund the amount back to the lockup
    let res: NearToken = e.claim(&users.alice).unwrap_json();
    assert_eq!(res, ZERO_NEAR);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 6);

    // Claim again but with storage deposit
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: NearToken = e.claim(&users.alice).unwrap_json();
    assert_eq!(res, amount / 6);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 6);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount / 2);
    assert_eq!(lockups[0].1.unclaimed_balance, ZERO_NEAR);

    // 2/3 unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, amount / 2);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 6);

    // Claim tokens
    let res: NearToken = e.claim(&users.alice).unwrap_json();
    assert_eq!(res, amount / 6);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, amount * 2 / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, ZERO_NEAR);

    // Claim again with no unclaimed_balance
    let res: NearToken = e.claim(&users.alice).unwrap_json();
    assert_eq!(res, ZERO_NEAR);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, amount * 2 / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, ZERO_NEAR);

    // full unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, amount * 2 / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 3);

    // Final claim
    let res: NearToken = e.claim(&users.alice).unwrap_json();
    assert_eq!(res, amount / 3);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount * 2 / 3);

    // User's lockups should be empty, since fully claimed.
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // Manually checking the lockup by index
    let lockup = e.get_lockup(0);
    assert_eq!(lockup.claimed_balance, amount);
    assert_eq!(lockup.unclaimed_balance, ZERO_NEAR);
}

#[test]
fn test_lockup_cliff_amazon() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = NearToken::from_yoctonear(6_000);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup_create = LockupCreate {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC - 1,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount.saturating_div(10),
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2,
                balance: amount.saturating_mul(3).saturating_div(10),
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 3,
                balance: amount.saturating_mul(6).saturating_div(10),
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4,
                balance: amount,
            },
        ]),
        vesting_schedule: None,
    };
    let balance: NearToken = e.add_lockup(&e.owner, amount, &lockup_create).unwrap_json();
    assert_eq!(balance, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[0].1.unclaimed_balance, ZERO_NEAR);

    // 1/12 time. pre-cliff unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[0].1.unclaimed_balance, ZERO_NEAR);

    // 1/4 time. cliff unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[0].1.unclaimed_balance, amount.saturating_div(10));

    // 3/8 time. cliff unlock + 1/2 of 2nd year.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC + ONE_YEAR_SEC / 2);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(
        lockups[0].1.unclaimed_balance,
        amount.saturating_mul(2).saturating_div(10)
    );

    // 1/2 time.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(
        lockups[0].1.unclaimed_balance,
        amount.saturating_mul(3).saturating_div(10)
    );

    // 1/2 + 1/12 time.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(
        lockups[0].1.unclaimed_balance,
        amount.saturating_mul(4).saturating_div(10)
    );

    // 1/2 + 2/12 time.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC * 2 / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(
        lockups[0].1.unclaimed_balance,
        amount.saturating_mul(5).saturating_div(10)
    );

    // 3/4 time.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(
        lockups[0].1.unclaimed_balance,
        amount.saturating_mul(6).saturating_div(10)
    );

    // 7/8 time.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 3 + ONE_YEAR_SEC / 2);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(
        lockups[0].1.unclaimed_balance,
        amount.saturating_mul(8).saturating_div(10)
    );

    // full unlock.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, amount);

    // after unlock.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 5);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, amount);

    // attempt to claim without storage.
    let res: NearToken = e.claim(&users.alice).unwrap_json();
    assert_eq!(res, ZERO_NEAR);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, ZERO_NEAR);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, amount);

    // Claim tokens
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: NearToken = e.claim(&users.alice).unwrap_json();
    assert_eq!(res, amount);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount);

    // Check lockup after claim
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup = e.get_lockup(0);
    assert_eq!(lockup.claimed_balance, amount);
    assert_eq!(lockup.unclaimed_balance, ZERO_NEAR);
}

#[test]
fn test_claim_specific_lockups_with_specific_amounts_success() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let lockup_create = LockupCreate {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount,
            },
        ]),
        vesting_schedule: None,
    };

    let balance: NearToken = e.add_lockup(&e.owner, amount, &lockup_create).unwrap_json();
    assert_eq!(balance, amount);
    let balance: NearToken = e.add_lockup(&e.owner, amount, &lockup_create).unwrap_json();
    assert_eq!(balance, amount);
    let balance: NearToken = e.add_lockup(&e.owner, amount, &lockup_create).unwrap_json();
    assert_eq!(balance, amount);

    // Set time to half unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 2);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 3);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 2);
    assert_eq!(lockups[1].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[1].1.unclaimed_balance, amount / 2);
    assert_eq!(lockups[2].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[2].1.unclaimed_balance, amount / 2);

    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);

    // CLAIM
    let res: NearToken = e
        .claim_specific_lockups(
            &users.alice,
            &vec![(2, None), (1, Some((amount / 3).into()))],
        )
        .unwrap_json();
    assert_eq!(res, amount / 3 + amount / 2);

    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 3);
    assert_eq!(lockups[0].1.claimed_balance, ZERO_NEAR);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 2);
    assert_eq!(lockups[1].1.claimed_balance, amount / 3);
    assert_eq!(lockups[1].1.unclaimed_balance, amount / 6);
    assert_eq!(lockups[2].1.claimed_balance, amount / 2);
    assert_eq!(lockups[2].1.unclaimed_balance, ZERO_NEAR);

    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 3 + amount / 2);
}

#[test]
fn test_claim_specific_lockups_with_specific_amounts_fail() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = NearToken::from_yoctonear(60000);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let lockup_create = LockupCreate {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount,
            },
        ]),
        vesting_schedule: None,
    };

    let balance: NearToken = e.add_lockup(&e.owner, amount, &lockup_create).unwrap_json();
    assert_eq!(balance, amount);
    let balance: NearToken = e.add_lockup(&e.owner, amount, &lockup_create).unwrap_json();
    assert_eq!(balance, amount);

    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);

    // Set time to half unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 2);

    // CLAIM not existing lockup
    let res = e.claim_specific_lockups(
        &users.bob,
        &vec![(9, Some((amount.saturating_div(3)).into()))],
    );
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("lockup not found for account"));

    // CLAIM by wrong user
    let res = e.claim_specific_lockups(
        &users.bob,
        &vec![
            (1, Some((amount.saturating_div(3)).into())),
            (0, Some((amount.saturating_div(4)).into())),
        ],
    );
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("lockup not found for account"));

    // CLAIM by wrong user without amount
    let res = e.claim_specific_lockups(&users.bob, &vec![(1, None)]);
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("lockup not found for account"));

    // CLAIM with too big amount
    let res = e.claim_specific_lockups(
        &users.alice,
        &vec![
            (1, Some(amount.saturating_mul(2).saturating_div(3))),
            (0, Some(amount.saturating_div(4))),
        ],
    );
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("too big claim_amount for lockup"));
}

#[test]
fn test_claim_specific_lockups_overflow() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let lockup_create = LockupCreate {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount,
            },
        ]),
        vesting_schedule: None,
    };

    let balance: NearToken = e.add_lockup(&e.owner, amount, &lockup_create).unwrap_json();
    assert_eq!(balance, amount);

    // Set time to half unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 2);

    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);

    // claim part
    let res = e.claim_specific_lockups(&users.alice, &vec![(0, Some((amount / 4).into()))]);
    assert!(res.is_ok());
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 4);

    // claim with overflow
    let res = e.claim_specific_lockups(&users.alice, &vec![(0, Some(u128::MAX.into()))]);
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("attempt to add with overflow"));
}
