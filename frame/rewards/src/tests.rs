// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Betelgeuse.
//
// Copyright (c) 2020 Wei Tang.
// Copyright (c) 2020 Shawn Tabrizi.
//
// Betelgeuse is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Betelgeuse is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Betelgeuse. If not, see <http://www.gnu.org/licenses/>.

//! Tests for Rewards Pallet

use crate::*;
use crate::mock::*;
use sp_runtime::testing::{Digest, DigestItem};
use frame_system::InitKind;
use frame_support::{assert_ok, assert_noop};
use frame_support::error::BadOrigin;
use frame_support::traits::{OnInitialize, OnFinalize};
use pallet_balances::Error as BalancesError;

// Get the last event from System
fn last_event() -> mock::Event {
	System::events().pop().expect("Event expected").event
}

/// Run until a particular block.
fn run_to_block(n: u64, author: u64) {
	while System::block_number() < n {
		Rewards::on_finalize(System::block_number());
		Balances::on_finalize(System::block_number());

		let current_block = System::block_number() + 1;
		let parent_hash = System::parent_hash();
		let pre_digest = DigestItem::PreRuntime(sp_consensus_pow::POW_ENGINE_ID, author.encode());
		System::initialize(
			&current_block,
			&parent_hash,
			&Default::default(),
			&Digest { logs: vec![pre_digest] },
			InitKind::Full
		);
		System::set_block_number(current_block);

		Balances::on_initialize(System::block_number());
		Rewards::on_initialize(System::block_number());
	}
}

#[test]
fn genesis_config_works() {
	new_test_ext(1).execute_with(|| {
		assert_eq!(Author::<Test>::get(), Some(1));
		assert_eq!(Reward::<Test>::get(), 60);
		assert_eq!(Balances::free_balance(1), 0);
		assert_eq!(Balances::free_balance(2), 0);
		assert_eq!(System::block_number(), 1);
	});
}

#[test]
fn set_reward_works() {
	new_test_ext(1).execute_with(|| {
		// Fails with bad origin
		assert_noop!(Rewards::set_reward(Origin::signed(1), 42), BadOrigin);
		assert_noop!(Rewards::set_reward(Origin::none(), 42), BadOrigin);
		// Successful
		assert_ok!(Rewards::set_reward(Origin::root(), 42));
		assert_eq!(Reward::<Test>::get(), 42);
		assert_eq!(last_event(), RawEvent::RewardChanged(42).into());
		// Fails when too low
		assert_noop!(Rewards::set_reward(Origin::root(), 0), Error::<Test>::RewardTooLow);
	});
}

#[test]
fn set_author_works() {
	new_test_ext(1).execute_with(|| {
		// Fails with bad origin
		assert_noop!(Rewards::note_author_prefs(Origin::signed(1), Perbill::zero()), BadOrigin);
		// Block author can successfully set themselves
		assert_ok!(Rewards::note_author_prefs(Origin::none(), Perbill::zero()));
		// Cannot set author twice
		assert_noop!(
			Rewards::note_author_prefs(Origin::none(), Perbill::zero()),
			Error::<Test>::AuthorPrefsAlreadySet,
		);
		assert_eq!(Author::<Test>::get(), Some(1));
	});
}

#[test]
fn reward_payment_works() {
	new_test_ext(1).execute_with(|| {
		// Block author sets themselves as author
		assert_ok!(Rewards::note_author_prefs(Origin::none(), Perbill::zero()));
		// Next block
		run_to_block(2, 2);
		// User gets reward
		assert_eq!(Balances::free_balance(1), 54);

		// Set new reward
		assert_ok!(Rewards::set_reward(Origin::root(), 42));
		assert_ok!(Rewards::set_taxation(Origin::root(), Perbill::zero()));
		assert_ok!(Rewards::note_author_prefs(Origin::none(), Perbill::zero()));
		run_to_block(3, 1);
		assert_eq!(Balances::free_balance(2), 42);
	});
}

#[test]
fn reward_locks_work() {
	new_test_ext(1).execute_with(|| {
		// Make numbers better :)
		assert_ok!(Rewards::set_taxation(Origin::root(), Perbill::zero()));
		assert_ok!(Rewards::set_reward(Origin::root(), 101));

		// Block author sets themselves as author
		assert_ok!(Rewards::note_author_prefs(Origin::none(), Perbill::zero()));
		// Next block
		run_to_block(2, 1);
		// User gets reward
		assert_eq!(Balances::free_balance(1), 101);
		// 100 is locked, 1 is unlocked for paying txs
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 1));

		// Cannot transfer because of locks
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);

		// Confirm locks (10 of them, each of value 10)
		let mut expected_locks = (1..=10).map(|x| (x * 10 + 1, 10)).collect::<BTreeMap<_, _>>();
		assert_eq!(Rewards::reward_locks(1), expected_locks);

		// 10 blocks later (10 days)
		System::set_block_number(11);
		// User update locks
		assert_ok!(Rewards::unlock(Origin::signed(1), 1));
		// Locks updated
		expected_locks.remove(&11);
		assert_eq!(Rewards::reward_locks(1), expected_locks);
		// Transfer works
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 10));
		// Cannot transfer more
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);

		// User mints more blocks
		assert_ok!(Rewards::note_author_prefs(Origin::none(), Perbill::zero()));
		run_to_block(12, 1);
		assert_ok!(Rewards::note_author_prefs(Origin::none(), Perbill::zero()));
		run_to_block(13, 1);

		// Locks as expected
		// Left over from block 1 + from block 11
		let mut expected_locks = (2..=10).map(|x| (x * 10 + 1, 10 + 10)).collect::<BTreeMap<_, _>>();
		// Last one from block 11
		expected_locks.insert(111, 10);
		// From block 12
		expected_locks.append(&mut (2..=11).map(|x| (x * 10 + 2, 10)).collect::<BTreeMap<_, _>>());
		assert_eq!(Rewards::reward_locks(1), expected_locks);

		// User gains 2 free for txs
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 2));
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);

		// 20 more is unlocked on block 21
		System::set_block_number(21);
		assert_ok!(Rewards::unlock(Origin::signed(1), 1));
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 20));
		// 10 more unlocked on block 22
		System::set_block_number(22);
		assert_ok!(Rewards::unlock(Origin::signed(1), 1));
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 10));

		// Cannot transfer more
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);
	});
}

fn reward_point(start: u64, reward: u128, taxation: Perbill) -> CurvePoint<u64, u128> {
	CurvePoint { start, reward, taxation }
}

fn test_curve() -> Vec<CurvePoint<u64, u128>> {
	vec![
		reward_point(50, 20, Perbill::from_percent(50)),
		reward_point(40, 25, Perbill::from_percent(20)),
		reward_point(20, 50, Perbill::from_percent(10)),
		reward_point(10, 100, Perbill::from_percent(10)),
	]
}

#[test]
fn curve_works() {
	new_test_ext(1).execute_with(|| {
		// Set reward curve
		assert_ok!(Rewards::set_curve(Origin::root(), test_curve()));
		assert_eq!(last_event(), mock::Event::pallet_rewards(crate::Event::<Test>::CurveSet));
		// Check current reward
		assert_eq!(Rewards::reward(), 60);
		run_to_block(9, 1);
		assert_eq!(Rewards::reward(), 60);
		run_to_block(10, 1);
		// Update successful
		assert_eq!(Rewards::reward(), 100);
		// Success reported
		assert_eq!(last_event(), mock::Event::pallet_rewards(crate::Event::<Test>::RewardChanged(100)));
		run_to_block(20, 1);
		assert_eq!(Rewards::reward(), 50);
		// No change, not part of the curve
		run_to_block(30, 1);
		assert_eq!(Rewards::reward(), 50);
		run_to_block(40, 1);
		assert_eq!(Rewards::reward(), 25);
		run_to_block(50, 1);
		assert_eq!(Rewards::reward(), 20);
		// Curve is finished and is empty
		assert_eq!(Curve::<Test>::get(), vec![]);
		// Everything works fine past the curve definition
		run_to_block(100, 1);
		assert_eq!(Rewards::reward(), 20);
	});
}

#[test]
fn set_curve_works() {
	new_test_ext(1).execute_with(|| {
		// Bad Origin
		assert_noop!(Rewards::set_curve(Origin::signed(1), test_curve()), BadOrigin);
		// Duplicate Point
		let duplicate_curve = vec![
			reward_point(20, 50, Perbill::from_percent(0)),
			reward_point(20, 30, Perbill::from_percent(0)),
		];
		assert_noop!(
			Rewards::set_curve(Origin::root(), duplicate_curve),
			Error::<Test>::NotSorted,
		);
		// Unsorted
		let unsorted_curve = vec![
			reward_point(10, 30, Perbill::from_percent(0)),
			reward_point(20, 50, Perbill::from_percent(0)),
		];
		assert_noop!(
			Rewards::set_curve(Origin::root(), unsorted_curve),
			Error::<Test>::NotSorted,
		);
		// Single Point OK
		let single_point = vec![reward_point(100, 100, Perbill::from_percent(0))];
		assert_ok!(Rewards::set_curve(Origin::root(), single_point));
		// Empty Curve OK
		assert_ok!(Rewards::set_curve(Origin::root(), vec![]));
	});
}

#[test]
fn failed_update_reported() {
	new_test_ext(1).execute_with(|| {
		// Shouldn't be able to set reward to 0
		let bad_curve = vec![
			reward_point(30, 50, Perbill::from_percent(0)),
			reward_point(20, 0, Perbill::from_percent(0)),
			reward_point(10, 100, Perbill::from_percent(0)),
		];
		// Set reward curve
		assert_noop!(Rewards::set_curve(Origin::root(), bad_curve), Error::<Test>::RewardTooLow);
	});
}
