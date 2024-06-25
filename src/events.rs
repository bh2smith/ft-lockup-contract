use crate::lockup::{Lockup, LockupIndex};
use near_sdk::{json_types::U128, AccountId, NearToken};
use near_sdk_contract_tools::event;

/// Events to be generated by the contract according to NEP-297

#[event(version = "1.0.0", standard = "ft-lockup")]
pub struct FtLockupNew {
    pub token_id: AccountId,
}

#[event(version = "1.0.0", standard = "ft-lockup")]
pub struct FtLockupAddToDepositAllowlist {
    pub account_ids: Vec<AccountId>,
}

#[event(version = "1.0.0", standard = "ft-lockup")]
pub struct FtLockupRemoveFromDepositAllowlist {
    pub account_ids: Vec<AccountId>,
}

#[event(version = "1.0.0", standard = "ft-lockup")]
pub struct FtLockupCreateLockup {
    pub id: LockupIndex,
    pub account_id: AccountId,
    pub balance: NearToken,
    pub start: U128,
    pub finish: U128,
    pub terminatable: bool,
}

impl From<(LockupIndex, Lockup)> for FtLockupCreateLockup {
    fn from(tuple: (LockupIndex, Lockup)) -> Self {
        let (id, lockup) = tuple;
        Self {
            id,
            account_id: lockup.account_id,
            balance: lockup.schedule.total_balance(),
            start: U128(lockup.schedule.0.first().unwrap().timestamp),
            finish: U128(lockup.schedule.0.last().unwrap().timestamp),
            terminatable: lockup.termination_config.is_some(),
        }
    }
}

#[event(version = "1.0.0", standard = "ft-lockup")]
pub struct FtLockupClaimLockup {
    pub id: LockupIndex,
    pub amount: NearToken,
}

#[event(version = "1.0.0", standard = "ft-lockup")]
pub struct ClaimLockups {
    pub id: LockupIndex,
    pub amount: NearToken,
}

#[event(version = "1.0.0", standard = "ft-lockup")]
pub struct FtLockupTerminateLockup {
    pub id: LockupIndex,
    pub termination_timestamp: U128,
    pub unvested_balance: NearToken,
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::{
        serde_json,
        serde_json::{json, Value},
        test_utils,
        test_utils::VMContextBuilder,
        testing_env, VMContext,
    };
    use near_sdk_contract_tools::standard::nep297::Event;

    pub const PACKAGE_NAME: &str = "ft-lockup";
    pub const VERSION: &str = "1.0.0";

    pub fn get_context() -> VMContext {
        VMContextBuilder::new().is_view(true).build()
    }

    pub fn assert_equal_logs(expected_log: Value, log_str: &str) {
        let actual_log: Value = serde_json::from_str(&log_str.replace("EVENT_JSON:", "")).unwrap();
        assert_eq!(actual_log, expected_log)
    }

    #[test]
    fn test_ft_lockup_init() {
        testing_env!(get_context());

        let token_id = "token.near".parse().unwrap();
        FtLockupNew { token_id }.emit();
        let expected_log = json!({
            "standard": PACKAGE_NAME,
            "version": VERSION,
            "event": "ft_lockup_new",
            "data": { "token_id": "token.near" },
        });
        assert_equal_logs(expected_log, &test_utils::get_logs()[0]);
    }

    #[test]
    fn test_ft_lockup_add_to_deposit_allowlist() {
        testing_env!(get_context());

        let account_ids: Vec<AccountId> = ["alice.near", "bob.near"]
            .iter()
            .map(|&x| x.parse().unwrap())
            .collect();

        FtLockupAddToDepositAllowlist { account_ids }.emit();
        let expected_log = json!({
            "standard": PACKAGE_NAME,
            "version": VERSION,
            "event": "ft_lockup_add_to_deposit_allowlist",
            "data": { "account_ids": ["alice.near", "bob.near"] },
        });
        assert_equal_logs(expected_log, &test_utils::get_logs()[0]);
    }

    #[test]
    fn test_ft_lockup_remove_from_deposit_allowlist() {
        testing_env!(get_context());

        let account_ids: Vec<AccountId> = ["alice.near", "bob.near"]
            .iter()
            .map(|&x| x.parse().unwrap())
            .collect();
        FtLockupRemoveFromDepositAllowlist { account_ids }.emit();

        assert_equal_logs(
            json!({
                "standard": PACKAGE_NAME,
                "version": VERSION,
                "event": "ft_lockup_remove_from_deposit_allowlist",
                "data": { "account_ids": ["alice.near", "bob.near"] },
            }),
            &test_utils::get_logs()[0],
        )
    }

    #[test]
    fn test_ft_lockup_create_lockup() {
        testing_env!(get_context());

        let account_id: AccountId = "alice.near".parse().unwrap();
        let balance: NearToken = NearToken::from_yoctonear(10_000);
        let timestamp = U128(1_500_000_000);
        let lockup = Lockup::new_unlocked_since(account_id.clone(), balance, timestamp);
        let lockup_id: LockupIndex = 100;

        let event: FtLockupCreateLockup = (100, lockup).into();
        event.emit();
        assert_equal_logs(
            json!({
                "standard": PACKAGE_NAME,
                "version": VERSION,
                "event": "ft_lockup_create_lockup",
                "data":
                    {
                        "id": lockup_id,
                        "account_id": account_id,
                        "balance": balance,
                        "start": U128(timestamp.0 - 1),
                        "finish": timestamp,
                        "terminatable": false,
                    },
            }),
            &test_utils::get_logs()[0],
        )
    }

    #[test]
    fn test_ft_lockup_claim_lockup() {
        testing_env!(get_context());

        let lockup_id: LockupIndex = 100;
        let amount: NearToken = NearToken::from_yoctonear(10_000);

        FtLockupClaimLockup {
            id: lockup_id,
            amount,
        }
        .emit();
        assert_equal_logs(
            json!({
                "standard": PACKAGE_NAME,
                "version": VERSION,
                "event": "ft_lockup_claim_lockup",
                "data":
                    {
                        "id": lockup_id,
                        "amount": amount,
                    },
            }),
            &test_utils::get_logs()[0],
        )
    }

    #[test]
    fn test_ft_lockup_terminate_lockup() {
        testing_env!(get_context());

        let lockup_id: LockupIndex = 100;
        let termination_timestamp = U128(1_800_000_000);
        let unvested_balance = NearToken::from_yoctonear(10_000);

        FtLockupTerminateLockup {
            id: lockup_id,
            termination_timestamp,
            unvested_balance,
        }
        .emit();
        assert_equal_logs(
            json!({
                "standard": PACKAGE_NAME,
                "version": VERSION,
                "event": "ft_lockup_terminate_lockup",
                "data":
                    {
                        "id": lockup_id,
                        "termination_timestamp": termination_timestamp,
                        "unvested_balance": unvested_balance,
                    },
            }),
            &test_utils::get_logs()[0],
        )
    }
}
