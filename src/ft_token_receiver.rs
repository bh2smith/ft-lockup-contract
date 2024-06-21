use crate::{lockup::LockupCreate, Contract, ContractExt};
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::{
    env, json_types::U128, near, AccountId, PromiseOrValue,
};

#[near(serializers = [json])]
pub enum FtMessage {
    LockupCreate(LockupCreate),
}

#[near]
impl FungibleTokenReceiver for Contract {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        _amount: U128,
        _msg: String,
    ) -> PromiseOrValue<U128> {
        assert_eq!(
            env::predecessor_account_id(), self.token_account_id,
            "Invalid token ID"
        );
        self.assert_deposit_whitelist(&sender_id);
        // let amount = NearToken::from_yoctonear(amount.0);
        // TODO Understand why this:
        // let ft_message: FtMessage = serde_json::from_str(&msg).unwrap();
        // match ft_message {
        //     FtMessage::LockupCreate(lockup_create) => {
        //         let lockup = lockup_create.into_lockup(&sender_id);
        //         lockup.assert_new_valid(amount);
        //         let index = self.internal_add_lockup(&lockup);
        //         log!(
        //             "Created new lockup for {} with index {}",
        //             lockup.account_id,
        //             index
        //         );
        //         FtLockupCreateLockup::from((index, lockup)).emit()
        //     }
        // }

        PromiseOrValue::Value(0.into())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::{
        test_utils::VMContextBuilder,
        testing_env
    };
    use near_sdk::PromiseOrValue::Promise;
    use near_sdk::test_utils::accounts;

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
        testing_env!(context.build());
        let mut contract = Contract::new(accounts(0), vec![accounts(1)]);
        let value = contract.ft_on_transfer(accounts(1), U128(1), "Poop".to_string());
        match value {
            Promise(_) => {
                panic!("uh uh")
            }
            PromiseOrValue::Value(v) => {
                assert_eq!(v.0, 0)
            }
        }
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
    #[should_panic(expected = "Not in deposit whitelist")]
    fn test_ft_on_transfer_not_on_whitelist() {
        let context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new(accounts(1), vec![]);
        contract.ft_on_transfer(accounts(2), U128(1), "".to_string());
    }
}
