use near_sdk::{AccountId, log, near, NearToken, serde_json};
use near_sdk::json_types::U128;
use crate::lockup::{Lockup, LockupIndex};
use crate::{PACKAGE_NAME, VERSION};

/// Events to be generated by the contract according to NEP-297

#[near(serializers = [json])]
pub struct FtLockupNew {
    pub token_account_id: AccountId,
}

#[near(serializers = [json])]
pub struct FtLockupAddToDepositWhitelist {
    pub account_ids: Vec<AccountId>,
}

#[near(serializers = [json])]
pub struct FtLockupRemoveFromDepositWhitelist {
    pub account_ids: Vec<AccountId>,
}

#[near(serializers = [json])]
pub struct FtLockupAddToDraftOperatorsWhitelist {
    pub account_ids: Vec<AccountId>,
}

#[near(serializers = [json])]
pub struct FtLockupRemoveFromDraftOperatorsWhitelist {
    pub account_ids: Vec<AccountId>,
}

#[near(serializers = [json])]
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

#[near(serializers = [json])]
pub struct FtLockupClaimLockup {
    pub id: LockupIndex,
    pub amount: NearToken,
}

#[near(serializers = [json])]
pub struct FtLockupTerminateLockup {
    pub id: LockupIndex,
    pub termination_timestamp: U128,
    pub unvested_balance: NearToken,
}

#[near(serializers = [json])]
pub(crate) enum FtLockup {
    New(FtLockupNew),
    AddToDepositWhitelist(FtLockupAddToDepositWhitelist),
    RemoveFromDepositWhitelist(FtLockupRemoveFromDepositWhitelist),
    CreateLockup(Vec<FtLockupCreateLockup>),
    ClaimLockup(Vec<FtLockupClaimLockup>),
    TerminateLockup(Vec<FtLockupTerminateLockup>),
}

#[near(serializers = [json])]
pub(crate) struct NearEvent {
    standard: String,
    version: String,
    #[serde(flatten)]
    event_kind: FtLockup,
}

impl From<FtLockup> for NearEvent {
    fn from(event_kind: FtLockup) -> Self {
        Self {
            standard: PACKAGE_NAME.into(),
            version: VERSION.into(),
            event_kind,
        }
    }
}

impl NearEvent {
    fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    fn to_json_event_string(&self) -> String {
        format!("EVENT_JSON:{}", self.to_json_string())
    }

    pub(crate) fn emit(self) {
        log!("{}", &self.to_json_event_string());
    }
}

pub(crate) fn emit(event_kind: FtLockup) {
    NearEvent::from(event_kind).emit();
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::serde_json::json;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::{test_utils, testing_env, VMContext};

    pub fn get_context() -> VMContext {
        VMContextBuilder::new().is_view(true).build()
    }

    #[test]
    fn test_ft_lockup_init() {
        testing_env!(get_context());

        let token_account_id = "token.near".parse().unwrap();
        emit(FtLockup::New(FtLockupNew { token_account_id }));
        assert_eq!(
            test_utils::get_logs()[0],
            format!(
                r"EVENT_JSON:{}",
                json!({
                    "standard": PACKAGE_NAME,
                    "version": VERSION,
                    "event": "ft_lockup_new",
                    "data": { "token_account_id": "token.near" },
                })
                .to_string(),
            )
        );
    }

    #[test]
    fn test_ft_lockup_add_to_deposit_whitelist() {
        testing_env!(get_context());

        let account_ids: Vec<AccountId> = vec!["alice.near", "bob.near"]
            .iter()
            .map(|&x| x.parse().unwrap())
            .collect();
        emit(FtLockup::AddToDepositWhitelist(
            FtLockupAddToDepositWhitelist { account_ids },
        ));
        assert_eq!(
            test_utils::get_logs()[0],
            format!(
                r"EVENT_JSON:{}",
                json!({
                    "standard": PACKAGE_NAME,
                    "version": VERSION,
                    "event": "ft_lockup_add_to_deposit_whitelist",
                    "data": { "account_ids": ["alice.near", "bob.near"] },
                })
                .to_string(),
            )
        );
    }

    #[test]
    fn test_ft_lockup_remove_from_deposit_whitelist() {
        testing_env!(get_context());

        let account_ids: Vec<AccountId> = vec!["alice.near", "bob.near"]
            .iter()
            .map(|&x| x.parse().unwrap())
            .collect();
        emit(FtLockup::RemoveFromDepositWhitelist(
            FtLockupRemoveFromDepositWhitelist { account_ids },
        ));
        assert_eq!(
            test_utils::get_logs()[0],
            format!(
                r"EVENT_JSON:{}",
                json!({
                    "standard": PACKAGE_NAME,
                    "version": VERSION,
                    "event": "ft_lockup_remove_from_deposit_whitelist",
                    "data": { "account_ids": ["alice.near", "bob.near"] },
                })
                .to_string(),
            )
        );
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

        emit(FtLockup::CreateLockup(vec![event]));
        assert_eq!(
            test_utils::get_logs()[0],
            format!(
                r"EVENT_JSON:{}",
                json!({
                    "standard": PACKAGE_NAME,
                    "version": VERSION,
                    "event": "ft_lockup_create_lockup",
                    "data": [
                        {
                            "id": lockup_id,
                            "account_id": account_id,
                            "balance": balance,
                            "start": timestamp.0 - 1,
                            "finish": timestamp,
                            "terminatable": false,
                        },
                    ],
                })
                .to_string(),
            )
        );
    }

    #[test]
    fn test_ft_lockup_claim_lockup() {
        testing_env!(get_context());

        let lockup_id: LockupIndex = 100;
        let amount: NearToken = NearToken::from_yoctonear(10_000);

        let event = FtLockupClaimLockup {
            id: lockup_id,
            amount,
        };

        emit(FtLockup::ClaimLockup(vec![event]));
        assert_eq!(
            test_utils::get_logs()[0],
            format!(
                r"EVENT_JSON:{}",
                json!({
                    "standard": PACKAGE_NAME,
                    "version": VERSION,
                    "event": "ft_lockup_claim_lockup",
                    "data": [
                        {
                            "id": lockup_id,
                            "amount": amount,
                        },
                    ],
                })
                .to_string(),
            )
        );
    }

    #[test]
    fn test_ft_lockup_terminate_lockup() {
        testing_env!(get_context());

        let lockup_id: LockupIndex = 100;
        let termination_timestamp = U128(1_800_000_000);
        let unvested_balance = NearToken::from_yoctonear(10_000);

        let event = FtLockupTerminateLockup {
            id: lockup_id,
            termination_timestamp,
            unvested_balance,
        };

        emit(FtLockup::TerminateLockup(vec![event]));
        assert_eq!(
            test_utils::get_logs()[0],
            format!(
                r"EVENT_JSON:{}",
                json!({
                    "standard": PACKAGE_NAME,
                    "version": VERSION,
                    "event": "ft_lockup_terminate_lockup",
                    "data": [
                        {
                            "id": lockup_id,
                            "termination_timestamp": termination_timestamp,
                            "unvested_balance": unvested_balance,
                        },
                    ],
                })
                .to_string(),
            )
        );
    }
}
