#![allow(dead_code)]
pub use ft_lockup::{
    lockup::{Lockup, LockupCreate, LockupIndex},
    schedule::{Checkpoint, Schedule},
    termination::{TerminationConfig, VestingConditions},
    view::LockupView,
    Contract as FtLockupContract,
};
use near_contract_standards::fungible_token::metadata::{FungibleTokenMetadata, FT_METADATA_SPEC};
use near_sdk::{
    json_types::{Base58CryptoHash, U128},
    serde_json::{self, json},
    AccountId, Gas, Timestamp,
};
use near_workspaces::{
    network::Sandbox,
    operations::CallTransaction,
    result::{ExecutionResult, Value, ViewResultDetails},
    types::NearToken,
    Account, Contract, Worker,
};

const ONE_YOCTO: NearToken = NearToken::from_yoctonear(1);
pub const ONE_DAY_SEC: u128 = 24 * 60 * 60;
pub const ONE_YEAR_SEC: u128 = 365 * ONE_DAY_SEC;
pub const GENESIS_TIMESTAMP_SEC: u128 = 1_600_000_000;

pub const NEAR: &str = "near";
pub const TOKEN_ID: &str = "token.near";
pub const FT_LOCKUP_ID: &str = "ft-lockup.near";
pub const OWNER_ID: &str = "owner.near";
pub const DRAFT_OPERATOR_ID: &str = "draft_operator.near";

// https://docs.near.org/concepts/storage/storage-staking#how-much-does-it-cost
pub const STORAGE_PRICE_PER_BYTE: u128 = 10_000_000_000_000_000_000;

pub const ZERO_NEAR: NearToken = NearToken::from_near(0);
pub const T_GAS: Gas = Gas::from_gas(10u64.pow(12));
pub const DEFAULT_GAS: Gas = Gas::from_gas(15 * T_GAS.as_gas());
pub const MAX_GAS: Gas = Gas::from_gas(300 * T_GAS.as_gas());
pub const FT_TRANSFER_CALL_GAS: Gas = Gas::from_gas(60 * T_GAS.as_gas());
pub const CLAIM_GAS: Gas = Gas::from_gas(100 * T_GAS.as_gas());
pub const TERMINATE_GAS: Gas = Gas::from_gas(100 * T_GAS.as_gas());

// TODO - use arbitrary decimals.
pub const TOKEN_DECIMALS: u8 = 24;
pub const TOKEN_TOTAL_SUPPLY: NearToken = NearToken::from_near(1_000_000);
struct Setup {
    #[allow(unused)]
    worker: Worker<Sandbox>,
    pub root: Account,
    pub near: Account,
    pub owner: Account,
    pub token_owner: Account,
    pub contract: Contract,
    pub token: Contract,
}

pub struct Accounts {
    pub alice: Account,
    pub bob: Account,
    pub charlie: Account,
    pub dude: Account,
    pub eve: Account,
}

pub fn lockup_vesting_schedule(amount: NearToken) -> (Schedule, Schedule) {
    let lockup_schedule = Schedule(vec![
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC,
            balance: ZERO_NEAR,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2,
            balance: ZERO_NEAR,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4,
            balance: amount.saturating_mul(3).saturating_div(4),
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4 + 1,
            balance: amount,
        },
    ]);
    let vesting_schedule = Schedule(vec![
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC,
            balance: ZERO_NEAR,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC - 1,
            balance: ZERO_NEAR,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
            balance: amount.saturating_div(4),
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4,
            balance: amount,
        },
    ]);
    (lockup_schedule, vesting_schedule)
}

pub fn lockup_vesting_schedule_2(amount: NearToken) -> (Schedule, Schedule) {
    let lockup_schedule = Schedule(vec![
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2,
            balance: ZERO_NEAR,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4,
            balance: amount.saturating_mul(3).saturating_div(4),
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4 + 1,
            balance: amount,
        },
    ]);
    let vesting_schedule = Schedule(vec![
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC - 1,
            balance: ZERO_NEAR,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
            balance: amount.saturating_div(4),
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4,
            balance: amount,
        },
    ]);
    (lockup_schedule, vesting_schedule)
}

pub async fn storage_deposit(
    user: &Account,
    contract_id: &AccountId,
    account_id: &AccountId,
    attached_deposit: NearToken,
) {
    user.call(contract_id, "storage_deposit")
        .args_json(json!({ "account_id": account_id }))
        .deposit(attached_deposit)
        .transact()
        .await
        .unwrap()
        .unwrap();
}

pub async fn storage_force_unregister(user: &Account, contract_id: &AccountId) {
    user.call(contract_id, "storage_unregister")
        .args_json(json!({ "force": true }))
        .deposit(ONE_YOCTO)
        .transact()
        .await
        .unwrap()
        .unwrap();
}

pub async fn ft_storage_deposit(user: &Account, token_id: &AccountId, account_id: &AccountId) {
    storage_deposit(
        user,
        token_id,
        account_id,
        NearToken::from_yoctonear(125 * STORAGE_PRICE_PER_BYTE),
    )
    .await;
}

pub fn to_nano(timestamp: u32) -> Timestamp {
    Timestamp::from(timestamp) * 10u64.pow(9)
}

async fn exec_tx(ct: CallTransaction) -> ExecutionResult<Value> {
    ct.transact().await.unwrap().unwrap()
}

impl Setup {
    pub async fn init(deposit_whitelist: Option<Vec<AccountId>>) -> Self {
        let worker = near_workspaces::sandbox().await.unwrap();
        let token_owner = worker.dev_create_account().await.unwrap();

        let (token, contract, root, near, owner) = tokio::join!(
            async {
                let wasm = std::fs::read("./res/fungible_token.wasm").unwrap();
                let token = worker.dev_deploy(&wasm).await.unwrap();

                token
                    .call("new")
                    .args_json(json!({
                        "owner_id": token_owner.id(),
                        "total_supply": TOKEN_TOTAL_SUPPLY,
                        "metadata": FungibleTokenMetadata {
                          spec: FT_METADATA_SPEC.to_string(),
                          name: "Token".to_string(),
                          symbol: "TOKEN".to_string(),
                          icon: None,
                          reference: None,
                          reference_hash: None,
                          decimals: TOKEN_DECIMALS,
                      }
                    }))
                    .transact()
                    .await
                    .unwrap()
                    .unwrap();
                token
            },
            async {
                let wasm = near_workspaces::compile_project("./").await.unwrap();
                worker.dev_deploy(&wasm).await.unwrap()
            },
            async { worker.dev_create_account().await.unwrap() },
            async { worker.dev_create_account().await.unwrap() },
            async { worker.dev_create_account().await.unwrap() },
        );
        // TODO - may need to set the signer here.
        contract
            .call("new")
            .args_json(json!({
                "token_id": token.id(),
                "deposit_allowlist": deposit_whitelist.unwrap_or_else(|| vec![owner.id().clone()]),
            }))
            .deposit(NearToken::from_yoctonear(10));

        ft_storage_deposit(&owner, token.id(), contract.id()).await;

        Setup {
            worker,
            token,
            token_owner,
            contract,
            root,
            near,
            owner,
        }
    }

    pub async fn ft_transfer(
        &self,
        sender: &Account,
        amount: NearToken,
        receiver: &Account,
    ) -> ExecutionResult<Value> {
        let ct = sender
            .call(self.token.id(), "ft_transfer")
            .args_json(json!({
                "receiver_id": receiver.id(),
                "amount": amount,
            }))
            .max_gas()
            .deposit(ONE_YOCTO);
        exec_tx(ct).await
    }

    pub async fn ft_transfer_call(
        &self,
        user: &Account,
        amount: NearToken,
        msg: &str,
    ) -> ExecutionResult<Value> {
        let ct = user
            .call(self.token.id(), "ft_transfer_call")
            .args_json(json!({
                "receiver_id": self.contract.id(),
                "amount": amount,
                "msg": msg,
            }))
            .gas(FT_TRANSFER_CALL_GAS)
            .deposit(ONE_YOCTO);
        exec_tx(ct).await
    }

    pub async fn add_lockup(
        &self,
        user: &Account,
        amount: NearToken,
        lockup_create: &LockupCreate,
    ) -> ExecutionResult<Value> {
        self.ft_transfer_call(user, amount, &serde_json::to_string(lockup_create).unwrap())
            .await
    }

    pub async fn claim_specific_lockups(
        &self,
        user: &Account,
        amounts: &[(LockupIndex, Option<NearToken>)],
    ) -> ExecutionResult<Value> {
        let ct = user
            .call(self.contract.id(), "claim")
            .args_json(json!({"amounts": Some(amounts.to_owned())}))
            .gas(CLAIM_GAS);
        exec_tx(ct).await
    }

    pub async fn terminate(
        &self,
        user: &Account,
        lockup_index: LockupIndex,
    ) -> ExecutionResult<Value> {
        let ct = user
            .call(self.contract.id(), "terminate")
            .args_json(json!({"lockup_index": lockup_index}))
            .gas(TERMINATE_GAS)
            .deposit(ONE_YOCTO);
        exec_tx(ct).await
    }

    pub async fn terminate_with_schedule(
        &self,
        user: &Account,
        lockup_index: LockupIndex,
        hashed_schedule: Schedule,
    ) -> ExecutionResult<Value> {
        let ct = user
            .call(self.contract.id(), "terminate")
            .args_json(
                json!({"lockup_index": lockup_index, "hashed_schedule": Some(hashed_schedule)}),
            )
            .gas(TERMINATE_GAS)
            .deposit(ONE_YOCTO);
        exec_tx(ct).await
    }

    pub async fn terminate_with_timestamp(
        &self,
        user: &Account,
        lockup_index: LockupIndex,
        termination_timestamp: U128,
    ) -> ExecutionResult<Value> {
        let ct = user
            .call(self.contract.id(), "terminate")
            .args_json(
                json!({"lockup_index": lockup_index, "termination_timestamp": Some(termination_timestamp)}),
            )
            .gas(TERMINATE_GAS)
            .deposit(ONE_YOCTO);
        exec_tx(ct).await
    }

    pub async fn remove_from_deposit_whitelist_single(
        &self,
        user: &Account,
        account_id: &AccountId,
    ) -> ExecutionResult<Value> {
        let ct = user
            .call(self.contract.id(), "remove_from_deposit_whitelist")
            .args_json(json!({ "account_id": account_id }))
            .deposit(ONE_YOCTO);
        exec_tx(ct).await
    }

    pub async fn add_to_deposit_whitelist_single(
        &self,
        user: &Account,
        account_id: &AccountId,
    ) -> ExecutionResult<Value> {
        let ct = user
            .call(self.contract.id(), "add_to_deposit_whitelist")
            .args_json(json!({ "account_id": account_id }))
            .deposit(ONE_YOCTO);
        exec_tx(ct).await
    }

    pub async fn remove_from_deposit_whitelist(
        &self,
        user: &Account,
        account_id: &AccountId,
    ) -> ExecutionResult<Value> {
        let ct = user
            .call(self.contract.id(), "remove_from_deposit_whitelist")
            .args_json(json!({ "account_id": vec![account_id] }))
            .deposit(ONE_YOCTO);
        exec_tx(ct).await
    }

    pub async fn add_to_deposit_whitelist(
        &self,
        user: &Account,
        account_id: &AccountId,
    ) -> ExecutionResult<Value> {
        let ct = user
            .call(self.contract.id(), "add_to_deposit_whitelist")
            .args_json(json!({ "account_id": vec![account_id] }))
            .deposit(ONE_YOCTO);
        exec_tx(ct).await
    }

    pub async fn get_num_lockups(&self) -> u32 {
        self.near
            .view(self.contract.id(), "get_num_lockups")
            .await
            .unwrap()
            .json::<u32>()
            .unwrap()
    }

    pub async fn get_lockups(&self, indices: &[LockupIndex]) -> Vec<(LockupIndex, LockupView)> {
        self.near
            .view(self.contract.id(), "get_lockups")
            .args_json(json!({"indices": indices.to_owned()}))
            .await
            .unwrap()
            .json::<Vec<(LockupIndex, LockupView)>>()
            .unwrap()
    }

    pub async fn get_lockups_paged(
        &self,
        from_index: Option<LockupIndex>,
        limit: Option<LockupIndex>,
    ) -> Vec<(LockupIndex, LockupView)> {
        self.near
            .view(self.contract.id(), "get_lockups_paged")
            .args_json(json!({"from_index": from_index, "limit": limit}))
            .await
            .unwrap()
            .json::<Vec<(LockupIndex, LockupView)>>()
            .unwrap()
    }

    pub async fn get_deposit_whitelist(&self) -> Vec<AccountId> {
        self.near
            .view(self.contract.id(), "get_deposit_whitelist")
            .await
            .unwrap()
            .json::<Vec<AccountId>>()
            .unwrap()
    }

    pub async fn hash_schedule(&self, schedule: &Schedule) -> Base58CryptoHash {
        self.near
            .view(self.contract.id(), "hash_schedule")
            .args_json(json!({"schedule": schedule}))
            .await
            .unwrap()
            .json::<Base58CryptoHash>()
            .unwrap()
    }

    pub async fn validate_schedule(
        &self,
        schedule: &Schedule,
        total_balance: NearToken,
        termination_schedule: Option<&Schedule>,
    ) -> ViewResultDetails {
        self.near
            .view(self.contract.id(), "validate_schedule")
            .args_json(json!({
              "schedule": schedule.clone(),
              "total_balance": total_balance,
              "termination_schedule": termination_schedule

            }))
            .await
            .unwrap()
    }

    pub async fn get_token_id(&self) -> AccountId {
        self.near
            .view(self.contract.id(), "get_token_id")
            .await
            .unwrap()
            .json::<AccountId>()
            .unwrap()
    }

    pub async fn get_version(&self) -> String {
        self.near
            .view(self.contract.id(), "get_version")
            .await
            .unwrap()
            .json::<String>()
            .unwrap()
    }

    pub async fn get_account_lockups(&self, user: &AccountId) -> Vec<(LockupIndex, LockupView)> {
        self.near
            .view(self.contract.id(), "get_account_lockups")
            .args_json(json!({
              "account_id": user,
            }))
            .await
            .unwrap()
            .json::<Vec<(LockupIndex, LockupView)>>()
            .unwrap()
    }

    pub async fn get_lockup(&self, lockup_index: LockupIndex) -> LockupView {
        self.near
            .view(self.contract.id(), "get_lockup")
            .args_json(json!({
              "index": lockup_index,
            }))
            .await
            .unwrap()
            .json::<LockupView>()
            .unwrap()
    }

    pub async fn ft_balance_of(&self, user: &Account) -> NearToken {
        self.near
            .view(self.token.id(), "ft_balance_of")
            .args_json(json!({
                "account_id": user.id(),
            }))
            .await
            .unwrap()
            .json::<NearToken>()
            .unwrap()
    }

    // pub async fn set_time_sec(&self, timestamp_sec: TimestampSec) {
    //     self.near.borrow_runtime_mut().cur_block.block_timestamp = to_nano(timestamp_sec);
    // }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[tokio::test]
//     async fn test_setup() {
//         let x = setup().await;
//         println!("{:?}", x.ft_lockup);
//     }
// }
