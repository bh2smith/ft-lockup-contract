mod setup;
#[cfg(test)]
mod e2e_view {
    use super::*;

    use near_sdk::{require, NearToken};
    use setup::*;

    #[tokio::test]
    async fn test_hash_schedule() {
        let e = Setup::init(None).await;
        let amount = NearToken::from_near(60_000);
        let (lockup_schedule, vesting_schedule) = lockup_vesting_schedule_2(amount);
        assert_eq!(
            e.hash_schedule(&vesting_schedule).await,
            e.hash_schedule(&vesting_schedule).await
        );
        assert_ne!(
            e.hash_schedule(&vesting_schedule).await,
            e.hash_schedule(&lockup_schedule).await,
        );
    }

    #[tokio::test]
    #[should_panic = "lockup schedule is ahead of the termination schedule at timestamp 126144000"]
    async fn test_validate_schedule() {
        let e = Setup::init(None).await;
        let users = Accounts::init(&e).await;
        let amount = NearToken::from_near(60_000);

        let lockups = e.get_account_lockups(users.alice.id()).await;
        require!(lockups.is_empty());

        let (lockup_schedule, vesting_schedule) = lockup_vesting_schedule_2(amount);

        e.validate_schedule(&lockup_schedule, amount, Some(&vesting_schedule))
            .await;

        let incompatible_vesting_schedule = Schedule(vec![
            Checkpoint {
                timestamp: ONE_YEAR_SEC * 4,
                balance: ZERO_NEAR,
            },
            Checkpoint {
                timestamp: ONE_YEAR_SEC * 4 + 1,
                balance: amount,
            },
        ]);
        e.validate_schedule(
            &lockup_schedule,
            amount,
            Some(&incompatible_vesting_schedule),
        )
        .await;
    }

    #[tokio::test]
    async fn test_get_lockups() {
        let e = Setup::init(None).await;
        let users = Accounts::init(&e).await;
        let amount = NearToken::from_near(1);
        let lockups = e.get_account_lockups(users.alice.id()).await;
        assert!(lockups.is_empty());

        // create some lockups
        for user in [&users.alice, &users.bob, &users.charlie] {
            let balance = e
                .add_lockup(
                    &e.owner,
                    amount,
                    &LockupCreate::new_unlocked(user.id().clone(), amount),
                )
                .await;
            assert_eq!(balance.0, amount.as_yoctonear());
        }

        // get_num_lockups
        let num_lockups = e.get_num_lockups().await;
        assert_eq!(num_lockups, 3);

        // get_lockups by indices
        let res = e.get_lockups(&[2, 0]).await;
        assert_eq!(res.len(), 2);
        assert_eq!(&res[0].1.account_id, users.charlie.id());
        assert_eq!(&res[1].1.account_id, users.alice.id());

        // get_lockups_paged from to
        let res = e.get_lockups_paged(Some(1), Some(2)).await;
        assert_eq!(res.len(), 1);
        assert_eq!(&res[0].1.account_id, users.bob.id());

        // get_lockups_paged from
        let res = e.get_lockups_paged(Some(1), None).await;
        assert_eq!(res.len(), 2);
        assert_eq!(&res[0].1.account_id, users.bob.id());
        assert_eq!(&res[1].1.account_id, users.charlie.id());

        // get_lockups_paged to
        let res = e.get_lockups_paged(None, Some(2)).await;
        assert_eq!(res.len(), 2);
        assert_eq!(&res[0].1.account_id, users.alice.id());
        assert_eq!(&res[1].1.account_id, users.bob.id());

        // get_lockups_paged all
        let res = e.get_lockups_paged(None, None).await;
        assert_eq!(res.len(), 3);
        assert_eq!(&res[0].1.account_id, users.alice.id());
        assert_eq!(&res[1].1.account_id, users.bob.id());
        assert_eq!(&res[2].1.account_id, users.charlie.id());
    }

    #[tokio::test]
    async fn test_get_token_id() {
        let e = Setup::init(None).await;

        let result = e.get_token_id().await;
        assert_eq!(&result, e.token.id());
    }

    #[tokio::test]
    async fn test_get_version() {
        let e = Setup::init(None).await;

        let result = e.get_version().await;
        assert_eq!(result, env!("CARGO_PKG_VERSION").to_string());
    }
}
