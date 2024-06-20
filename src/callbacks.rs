use crate::event::{emit, FtLockup, FtLockupClaimLockup, FtLockupCreateLockup};
use crate::lockup::{Lockup, LockupClaim};
use crate::util::{current_timestamp_sec, ZERO_NEAR};
use crate::Contract;
use crate::ContractExt;
use near_sdk::{ext_contract, is_promise_success, log, near_bindgen, AccountId, NearToken};
#[ext_contract(callbacks)]
pub trait SelfCallbacks {
    fn after_ft_transfer(
        &mut self,
        account_id: AccountId,
        lockup_claims: Vec<LockupClaim>,
    ) -> NearToken;

    fn after_lockup_termination(&mut self, account_id: AccountId, amount: NearToken) -> NearToken;
}

#[near_bindgen]
impl SelfCallbacks for Contract {
    #[private]
    fn after_ft_transfer(
        &mut self,
        account_id: AccountId,
        lockup_claims: Vec<LockupClaim>,
    ) -> NearToken {
        let promise_success = is_promise_success();
        let mut total_balance = ZERO_NEAR;
        if promise_success {
            let mut remove_indices = vec![];
            let mut events: Vec<FtLockupClaimLockup> = vec![];
            for LockupClaim {
                index,
                is_final,
                claim_amount,
            } in lockup_claims
            {
                if is_final {
                    remove_indices.push(index);
                }
                total_balance = total_balance.saturating_add(claim_amount);
                let event = FtLockupClaimLockup {
                    id: index,
                    amount: claim_amount,
                };
                events.push(event);
            }
            if !remove_indices.is_empty() {
                let mut indices = self.account_lockups.get(&account_id).unwrap_or_default();
                for index in remove_indices {
                    indices.remove(&index);
                }
                self.internal_save_account_lockups(&account_id, indices);
            }
            emit(FtLockup::ClaimLockup(events));
        } else {
            log!("Token transfer has failed. Refunding.");
            let mut modified = false;
            let mut indices = self.account_lockups.get(&account_id).unwrap_or_default();
            for LockupClaim {
                index,
                claim_amount,
                ..
            } in lockup_claims
            {
                if indices.insert(index) {
                    modified = true;
                }
                let mut lockup = self.lockups.get(index as _).unwrap();
                lockup.claimed_balance = lockup.claimed_balance.saturating_sub(claim_amount);
                self.lockups.replace(index as _, &lockup);
            }

            if modified {
                self.internal_save_account_lockups(&account_id, indices);
            }
        }
        total_balance
    }

    #[private]
    fn after_lockup_termination(&mut self, account_id: AccountId, amount: NearToken) -> NearToken {
        let promise_success = is_promise_success();
        if !promise_success {
            log!("Lockup termination transfer has failed.");
            // There is no internal balance, so instead we create a new lockup.
            let lockup = Lockup::new_unlocked_since(account_id, amount, current_timestamp_sec());
            let lockup_index = self.internal_add_lockup(&lockup);
            let event: FtLockupCreateLockup = (lockup_index, lockup).into();
            emit(FtLockup::CreateLockup(vec![event]));
            ZERO_NEAR
        } else {
            amount
        }
    }
}
