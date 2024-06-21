# Fungible Token Lockup Contract

## Features

- A reusable lockup contract for a select fungible token.
- Lockup schedule can be set as a list of checkpoints with time and balance.
- Supports multiple lockups per account ID.
- Ability to create a lockup that can be terminated
  - A single lockup can be only terminated by a specific account ID.
  - Supports custom vesting schedule that should be ahead of the lockup schedule
  - The vesting schedule can be hidden behind a hash, so it only needs to be revealed in case of termination.
- Automatic rollbacks if a FT transfer fails.
- Claiming all account's lockups in a single transaction.
- Ability to add new lockups.
- Allowlist for the accounts that can create new lockups.
