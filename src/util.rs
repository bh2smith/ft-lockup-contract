use near_sdk::{env, json_types::U128, NearToken, Timestamp};

pub(crate) const ZERO_NEAR: NearToken = NearToken::from_near(0);
pub(crate) fn nano_to_sec(timestamp: Timestamp) -> u128 {
    (timestamp / 10u64.pow(9)) as u128
}

pub(crate) fn current_timestamp_sec() -> U128 {
    U128(nano_to_sec(env::block_timestamp()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nano_to_sec() {
        assert_eq!(nano_to_sec(1_719_234_571_328_277_000), 1_719_234_571);
    }

    #[test]
    fn test_current_timestamp_sec() {
        // env is working on a fresh blockchain starting from time 0
        assert_eq!(current_timestamp_sec().0, 0);
    }
}
