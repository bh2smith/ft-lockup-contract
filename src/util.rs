use crate::*;

pub(crate) const ZERO_NEAR: NearToken = NearToken::from_near(0);
pub(crate) fn nano_to_sec(timestamp: Timestamp) -> TimestampSec {
    (timestamp / 10u64.pow(9)) as _
}

pub(crate) fn current_timestamp_sec() -> TimestampSec {
    nano_to_sec(env::block_timestamp())
}
