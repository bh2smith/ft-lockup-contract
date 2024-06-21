use crate::{
    event::{emit, FtLockup, FtLockupCreateLockup},
    lockup::LockupCreate,
    Contract, ContractExt,
};
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::{
    env, json_types::U128, log, near, serde_json, AccountId, NearToken, PromiseOrValue,
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
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        assert_eq!(
            env::predecessor_account_id(),
            self.token_account_id,
            "Invalid token ID"
        );
        self.assert_deposit_whitelist(&sender_id);
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
                let event: FtLockupCreateLockup = (index, lockup).into();
                emit(FtLockup::CreateLockup(vec![event]));
            }
        }

        PromiseOrValue::Value(0.into())
    }
}
