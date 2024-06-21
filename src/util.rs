use near_sdk::{env, json_types::U128, NearToken, Timestamp};

pub(crate) const ZERO_NEAR: NearToken = NearToken::from_near(0);
pub(crate) fn nano_to_sec(timestamp: Timestamp) -> u128 {
    (timestamp / 10u64.pow(9)) as u128
}

// TODO - DO NOT USE BLOCK TIME! USE BLOCK NUMBER!
pub(crate) fn current_timestamp_sec() -> U128 {
    U128(nano_to_sec(env::block_timestamp()))
}
