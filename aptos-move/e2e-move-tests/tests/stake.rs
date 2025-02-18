// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

use aptos_types::account_address::AccountAddress;
use e2e_move_tests::{
    assert_success, enable_golden, get_stake_pool, get_validator_set, initialize_staking,
    join_validator_set, leave_validator_set, rotate_consensus_key, setup_staking, unlock_stake,
    withdraw_stake, MoveHarness,
};

#[test]
fn test_staking_end_to_end() {
    let mut harness = MoveHarness::new();
    enable_golden!(harness);
    let owner = harness.new_account_at(AccountAddress::from_hex_literal("0x123").unwrap());
    let operator = harness.new_account_at(AccountAddress::from_hex_literal("0x234").unwrap());
    let owner_address = *owner.address();
    let operator_address = *operator.address();

    // Initialize and add stake.
    let stake_amount = 100_000_000;
    assert_success!(initialize_staking(
        &mut harness,
        &owner,
        stake_amount,
        operator_address,
        owner_address
    ));
    let stake_pool = get_stake_pool(&harness, &owner_address);
    assert_eq!(stake_pool.active, stake_amount);
    assert_eq!(stake_pool.operator_address, operator_address);
    assert_eq!(stake_pool.delegated_voter, owner_address);

    // Join the validator set.
    assert_success!(rotate_consensus_key(&mut harness, &operator, owner_address));
    assert_success!(join_validator_set(&mut harness, &operator, owner_address));
    harness.new_epoch();

    // Validator should now be locked up.
    let stake_pool = get_stake_pool(&harness, &owner_address);
    assert_eq!(
        stake_pool.locked_until_secs,
        harness.executor.get_block_time_seconds() + 7200
    );

    // Unlock 1/4 stake.
    let amount_to_withdraw = stake_amount / 4;
    let remaining_stake = stake_amount - amount_to_withdraw;
    assert_success!(unlock_stake(&mut harness, &owner, amount_to_withdraw));
    let stake_pool = get_stake_pool(&harness, &owner_address);
    assert_eq!(stake_pool.active, remaining_stake);
    assert_eq!(stake_pool.pending_inactive, amount_to_withdraw);

    // Wait until stake is unlocked.
    harness.fast_forward(7200);
    harness.new_epoch();
    let stake_pool = get_stake_pool(&harness, &owner_address);
    assert_eq!(stake_pool.pending_inactive, 0);
    assert_eq!(stake_pool.inactive, amount_to_withdraw);

    // Withdraw and verify that coins are returned.
    assert_success!(withdraw_stake(&mut harness, &owner, stake_amount / 2));
    let stake_pool = get_stake_pool(&harness, &owner_address);
    assert_eq!(stake_pool.inactive, 0);

    // Verify that lockup has been renewed for remaining stake.
    assert_eq!(stake_pool.active, remaining_stake);
    assert_eq!(
        stake_pool.locked_until_secs,
        harness.executor.get_block_time_seconds() + 7200
    );

    // Validator takes the rest of the stake out.
    assert_success!(unlock_stake(&mut harness, &owner, remaining_stake));
    harness.fast_forward(7200);
    harness.new_epoch();
    assert_success!(withdraw_stake(&mut harness, &owner, remaining_stake));
    let stake_pool = get_stake_pool(&harness, &owner_address);
    assert_eq!(stake_pool.active, 0);
    assert_eq!(stake_pool.inactive, 0);
}

#[test]
fn test_staking_rewards() {
    let mut harness = MoveHarness::new();
    enable_golden!(harness);
    let validator_1 = harness.new_account_at(AccountAddress::from_hex_literal("0x123").unwrap());
    let validator_2 = harness.new_account_at(AccountAddress::from_hex_literal("0x234").unwrap());
    let validator_1_address = *validator_1.address();
    let validator_2_address = *validator_2.address();

    // Initialize the validators.
    let rewards_per_epoch = 1141;
    let mut stake_amount_1 = 100_000_000;
    setup_staking(&mut harness, &validator_1, stake_amount_1);
    let mut stake_amount_2 = 100_000_000;
    setup_staking(&mut harness, &validator_2, stake_amount_2);
    harness.new_epoch();

    // Both validators propose a block in the current epoch. Both should receive rewards.
    harness.new_block_with_metadata(Some(0), vec![]);
    harness.new_block_with_metadata(Some(1), vec![]);
    harness.new_epoch();
    stake_amount_1 += rewards_per_epoch;
    stake_amount_2 += rewards_per_epoch;
    assert_eq!(
        get_stake_pool(&harness, &validator_1_address).active,
        stake_amount_1
    );
    assert_eq!(
        get_stake_pool(&harness, &validator_2_address).active,
        stake_amount_2
    );

    // Each validator proposes in their own epoch. They receive the rewards at the end of each epoch
    harness.new_block_with_metadata(Some(0), vec![]);
    harness.new_epoch();
    stake_amount_1 += rewards_per_epoch;
    assert_eq!(
        get_stake_pool(&harness, &validator_1_address).active,
        stake_amount_1
    );
    assert_eq!(
        get_stake_pool(&harness, &validator_2_address).active,
        stake_amount_2
    );
    harness.new_block_with_metadata(Some(1), vec![]);
    harness.new_epoch();
    assert_eq!(
        get_stake_pool(&harness, &validator_1_address).active,
        stake_amount_1
    );
    stake_amount_2 += rewards_per_epoch;
    assert_eq!(
        get_stake_pool(&harness, &validator_2_address).active,
        stake_amount_2
    );

    // Validator 1 misses one proposal and thus receives no rewards while validator 2 didn't miss
    // any so they receive full rewards.
    harness.new_block_with_metadata(Some(1), vec![0]);
    harness.new_epoch();
    assert_eq!(
        get_stake_pool(&harness, &validator_1_address).active,
        stake_amount_1
    );
    stake_amount_2 += rewards_per_epoch;
    assert_eq!(
        get_stake_pool(&harness, &validator_2_address).active,
        stake_amount_2
    );

    // Validator 1 misses one proposal but has one successful so they receive half of the rewards.
    harness.new_block_with_metadata(Some(0), vec![0]);
    harness.new_epoch();
    stake_amount_1 += rewards_per_epoch / 2;
    assert_eq!(
        get_stake_pool(&harness, &validator_1_address).active,
        stake_amount_1
    );
}

#[test]
fn test_staking_rewards_pending_inactive() {
    let mut harness = MoveHarness::new();
    enable_golden!(harness);
    let validator = harness.new_account_at(AccountAddress::from_hex_literal("0x123").unwrap());
    let validator_address = *validator.address();

    // Initialize the validator.
    let stake_amount = 100_000_000;
    setup_staking(&mut harness, &validator, stake_amount);
    harness.new_epoch();

    // Validator requests to leave.
    leave_validator_set(&mut harness, &validator, validator_address);
    let validator_set = get_validator_set(&harness);
    assert_eq!(
        validator_set.pending_inactive[0].account_address,
        validator_address
    );

    // Validator proposes a block in the current epoch and should receive rewards despite
    // being pending_inactive.
    harness.new_block_with_metadata(Some(0), vec![]);
    harness.new_epoch();
    assert_eq!(
        get_stake_pool(&harness, &validator_address).active,
        stake_amount + 1141
    );
}
