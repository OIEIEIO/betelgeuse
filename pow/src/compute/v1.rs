// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Betelgeuse.
//
// Copyright (c) 2019-2020 Wei Tang.
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

use codec::{Encode, Decode};
use sp_core::H256;
use betelgeuse_primitives::Difficulty;
use super::Calculation;

#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct SealV1 {
	pub difficulty: Difficulty,
	pub nonce: H256,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ComputeV1 {
	pub key_hash: H256,
	pub pre_hash: H256,
	pub difficulty: Difficulty,
	pub nonce: H256,
}

impl ComputeV1 {
	pub fn input(&self) -> Calculation {
		let calculation = Calculation {
			difficulty: self.difficulty,
			pre_hash: self.pre_hash,
			nonce: self.nonce,
		};

		calculation
	}

	pub fn seal_and_work(&self, mode: super::ComputeMode) -> (SealV1, H256) {
		let input = self.input();

		let work = super::compute::<Calculation>(
			&self.key_hash,
			&input,
			mode,
		);

		(SealV1 {
			nonce: self.nonce,
			difficulty: self.difficulty,
		}, work)
	}

	pub fn seal(&self) -> SealV1 {
		SealV1 {
			nonce: self.nonce,
			difficulty: self.difficulty,
		}
	}
}
