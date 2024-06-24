use crate::{events::FtLockupCreateLockup, lockup::LockupCreate, Contract, ContractExt};
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::{
    env, json_types::U128, log, near, serde_json, AccountId, NearToken, PromiseOrValue,
};
use near_sdk_contract_tools::standard::nep297::Event;

#[near(serializers = [json])]
pub enum FtMessage {
    LockupCreate(LockupCreate),
}

#[near]
impl FungibleTokenReceiver for Contract {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        assert_eq!(
            env::predecessor_account_id(),
            self.token_account_id,
            "Invalid token ID"
        );
        self.assert_deposit_allowlist(&sender_id);
        let amount = NearToken::from_yoctonear(amount.0);
        let ft_message: FtMessage = serde_json::from_str(&msg).unwrap();
        match ft_message {
            FtMessage::LockupCreate(lockup_create) => {
                let lockup = lockup_create.into_lockup(&sender_id);
                lockup.assert_new_valid(amount);
                let index = self.internal_add_lockup(&lockup);
                log!(
                    "Created new lockup for {} with index {}",
                    lockup.account_id,
                    index
                );
                FtLockupCreateLockup::from((index, lockup)).emit()
            }
        }
        PromiseOrValue::Value(0.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        schedule::{Checkpoint, Schedule},
        util::ZERO_NEAR,
    };
    use near_sdk::{
        test_utils::{accounts, VMContextBuilder},
        testing_env,
    };

    fn get_context(predecessor_account_id: AccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .signer_account_id(predecessor_account_id.clone())
            .predecessor_account_id(predecessor_account_id);
        builder
    }

    #[test]
    fn test_ft_on_transfer_happy_path() {
        let context = get_context(accounts(0));
        let one_near = NearToken::from_near(1);
        testing_env!(context.build());
        let mut contract = Contract::new(accounts(0), vec![accounts(1)]);
        let lockup_create = FtMessage::LockupCreate(LockupCreate {
            account_id: "x.near".parse().unwrap(),
            schedule: Schedule(vec![
                Checkpoint {
                    timestamp: 0,
                    balance: ZERO_NEAR,
                },
                Checkpoint {
                    timestamp: 1,
                    balance: one_near,
                },
            ]),
            vesting_schedule: None,
        });
        let value = contract.ft_on_transfer(
            accounts(1),
            one_near.as_yoctonear().into(),
            serde_json::to_string(&lockup_create).unwrap(),
        );
        assert!(
            matches!(value, PromiseOrValue::Value(v) if v.0 == 0),
            "failed expectation!"
        );
    }

    #[test]
    #[should_panic(expected = "Invalid token ID")]
    fn test_ft_on_transfer_invalid_token() {
        let context = get_context(accounts(0));
        testing_env!(context.build());
        let mut contract = Contract::new(accounts(1), vec![]);
        contract.ft_on_transfer(accounts(2), U128(1), "".to_string());
    }

    #[test]
    #[should_panic(expected = "Not in deposit allowlist")]
    fn test_ft_on_transfer_not_on_allowlist() {
        let context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new(accounts(1), vec![]);
        contract.ft_on_transfer(accounts(2), U128(1), "".to_string());
    }
}
