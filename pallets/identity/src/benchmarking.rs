// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Identity pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use crate::Pallet as Identity;
use codec::Encode;
use frame_benchmarking::{account, v2::*, whitelisted_caller, BenchmarkError};
use frame_support::{
	assert_ok, ensure,
	traits::{EnsureOrigin, Get, OnFinalize, OnInitialize},
};
use frame_system::RawOrigin;
use sp_io::crypto::{sr25519_generate, sr25519_sign};
use sp_runtime::{
	traits::{IdentifyAccount, One},
	MultiSignature, MultiSigner,
};

const SEED: u32 = 0;

fn assert_has_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_has_event(generic_event.into());
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn run_to_block<T: Config>(n: frame_system::pallet_prelude::BlockNumberFor<T>) {
	while frame_system::Pallet::<T>::block_number() < n {
		crate::Pallet::<T>::on_finalize(frame_system::Pallet::<T>::block_number());
		frame_system::Pallet::<T>::on_finalize(frame_system::Pallet::<T>::block_number());
		frame_system::Pallet::<T>::set_block_number(
			frame_system::Pallet::<T>::block_number() + One::one(),
		);
		frame_system::Pallet::<T>::on_initialize(frame_system::Pallet::<T>::block_number());
		crate::Pallet::<T>::on_initialize(frame_system::Pallet::<T>::block_number());
	}
}
// Adds `r` registrars to the Identity Pallet. These registrars will have set
// fees and fields.
fn add_registrars<T: Config>(r: u32) -> Result<(), &'static str> {
	for i in 0..r {
		let registrar: T::AccountId = account("registrar", i, SEED);
		let registrar_lookup = T::Lookup::unlookup(registrar.clone());
		let registrar_origin = T::RegistrarOrigin::try_successful_origin()
			.expect("RegistrarOrigin has no successful origin required for the benchmark");
		Identity::<T>::add_registrar(registrar_origin, registrar_lookup)?;
		let fields = T::IdentityInformation::all_fields();
		Identity::<T>::set_fields(RawOrigin::Signed(registrar.clone()).into(), fields)?;
	}

	assert_eq!(Registrars::<T>::get().len(), r as usize);
	Ok(())
}

// Create `s` sub-accounts for the identity of `who` and return them.
// Each will have 32 bytes of raw data added to it.
fn create_sub_accounts<T: Config>(
	who: &T::AccountId,
	s: u32,
) -> Result<Vec<(T::AccountId, Data)>, &'static str> {
	let mut subs = Vec::new();
	let who_origin = RawOrigin::Signed(who.clone());
	let data = Data::Raw(vec![0; 32].try_into().unwrap());

	for i in 0..s {
		let sub_account = account("sub", i, SEED);
		subs.push((sub_account, data.clone()));
	}

	// Set identity so `set_subs` does not fail.
	if IdentityOf::<T>::get(who).is_none() {
		let info = T::IdentityInformation::create_identity_info();
		Identity::<T>::set_identity(who_origin.into(), Box::new(info))?;
	}

	Ok(subs)
}

// Adds `s` sub-accounts to the identity of `who`. Each will have 32 bytes of raw data added to it.
// This additionally returns the vector of sub-accounts so it can be modified if needed.
fn add_sub_accounts<T: Config>(
	who: &T::AccountId,
	s: u32,
) -> Result<Vec<(T::AccountId, Data)>, &'static str> {
	let who_origin = RawOrigin::Signed(who.clone());
	let subs = create_sub_accounts::<T>(who, s)?;

	Identity::<T>::set_subs(who_origin.into(), subs.clone())?;

	Ok(subs)
}

fn bench_suffix() -> Vec<u8> {
	b"bench".to_vec()
}

fn bench_username() -> Vec<u8> {
	// len = 24
	b"veryfastbenchmarkmachine".to_vec()
}

fn bounded_username<T: Config>(username: Vec<u8>, suffix: Vec<u8>) -> Username<T> {
	let mut full_username = Vec::with_capacity(username.len() + suffix.len() + 1);
	full_username.extend(username);
	full_username.extend(b"@");
	full_username.extend(suffix);
	Username::<T>::try_from(full_username).expect("test usernames should fit within bounds")
}

#[benchmarks(
	where
		<T as frame_system::Config>::AccountId: From<sp_runtime::AccountId32>,
		T::OffchainSignature: From<MultiSignature>,
)]
mod benchmarks {
	use super::*;
	#[benchmark]
	fn add_registrar(r: Linear<1, { T::MaxRegistrars::get() - 1 }>) -> Result<(), BenchmarkError> {
		add_registrars::<T>(r)?;
		ensure!(Registrars::<T>::get().len() as u32 == r, "Registrars not set up correctly.");
		let origin =
			T::RegistrarOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let account = T::Lookup::unlookup(account("registrar", r + 1, SEED));

		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, account);

		ensure!(Registrars::<T>::get().len() as u32 == r + 1, "Registrars not added.");
		Ok(())
	}

	#[benchmark]
	fn remove_registrar(
		r: Linear<1, { T::MaxRegistrars::get() - 1 }>,
	) -> Result<(), BenchmarkError> {
		add_registrars::<T>(r)?;
		ensure!(Registrars::<T>::get().len() as u32 == r, "Registrars not set up correctly.");
		let origin =
			T::RegistrarOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let account = T::Lookup::unlookup(account("registrar", r - 1, SEED));
		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, account);

		ensure!(Registrars::<T>::get().len() as u32 == r - 1, "Registrar not removed.");

		Ok(())
	}

	#[benchmark]
	fn set_identity(r: Linear<1, { T::MaxRegistrars::get() }>) -> Result<(), BenchmarkError> {
		add_registrars::<T>(r)?;

		let caller: T::AccountId = whitelisted_caller();
		let caller_lookup = T::Lookup::unlookup(caller.clone());
		let caller_origin: <T as frame_system::Config>::RuntimeOrigin =
			RawOrigin::Signed(caller.clone()).into();

		// Add an initial identity
		let initial_info = T::IdentityInformation::create_identity_info();
		Identity::<T>::set_identity(caller_origin.clone(), Box::new(initial_info.clone()))?;

		// User requests judgement from all the registrars, and they approve
		for i in 0..r {
			let registrar: T::AccountId = account("registrar", i, SEED);

			Identity::<T>::request_judgement(caller_origin.clone(), registrar.clone())?;
			Identity::<T>::provide_judgement(
				RawOrigin::Signed(registrar).into(),
				caller_lookup.clone(),
				Judgement::Reasonable,
				T::Hashing::hash_of(&initial_info),
			)?;
		}

		#[extrinsic_call]
		_(
			RawOrigin::Signed(caller.clone()),
			Box::new(T::IdentityInformation::create_identity_info()),
		);

		assert_last_event::<T>(Event::<T>::IdentitySet { who: caller }.into());
		Ok(())
	}

	// We need to split `set_subs` into two benchmarks to accurately isolate the potential
	// writes caused by new or old sub accounts. The actual weight should simply be
	// the sum of these two weights.
	#[benchmark]
	fn set_subs_new(s: Linear<0, { T::MaxSubAccounts::get() }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		// Create a new subs vec with sub accounts
		let subs = create_sub_accounts::<T>(&caller, s)?;
		ensure!(SubsOf::<T>::get(&caller).len() == 0, "Caller already has subs");

		#[extrinsic_call]
		set_subs(RawOrigin::Signed(caller.clone()), subs);

		ensure!(SubsOf::<T>::get(&caller).len() as u32 == s, "Subs not added");
		Ok(())
	}

	#[benchmark]
	fn set_subs_old(p: Linear<0, { T::MaxSubAccounts::get() }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		// Give them p many previous sub accounts.
		let _ = add_sub_accounts::<T>(&caller, p)?;

		// Remove all subs.
		let subs = create_sub_accounts::<T>(&caller, 0)?;
		ensure!(SubsOf::<T>::get(&caller).len() as u32 == p, "Caller does have subs",);

		#[extrinsic_call]
		set_subs(RawOrigin::Signed(caller.clone()), subs);

		ensure!(SubsOf::<T>::get(&caller).len() == 0, "Subs not removed");
		Ok(())
	}

	#[benchmark]
	fn clear_identity(
		r: Linear<1, { T::MaxRegistrars::get() }>,
		s: Linear<0, { T::MaxSubAccounts::get() }>,
	) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let caller_origin =
			<T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(caller.clone()));
		let caller_lookup = <T::Lookup as StaticLookup>::unlookup(caller.clone());

		// Register the registrars
		add_registrars::<T>(r)?;
		// Add sub accounts
		let _ = add_sub_accounts::<T>(&caller, s)?;

		// Create their main identity with x additional fields
		let info = T::IdentityInformation::create_identity_info();
		Identity::<T>::set_identity(caller_origin.clone(), Box::new(info.clone()))?;

		// User requests judgement from all the registrars, and they approve
		for i in 0..r {
			let registrar: T::AccountId = account("registrar", i, SEED);

			Identity::<T>::request_judgement(caller_origin.clone(), registrar.clone())?;
			Identity::<T>::provide_judgement(
				RawOrigin::Signed(registrar).into(),
				caller_lookup.clone(),
				Judgement::Reasonable,
				T::Hashing::hash_of(&info),
			)?;
		}

		ensure!(IdentityOf::<T>::contains_key(&caller), "Identity does not exist.");

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()));

		ensure!(!IdentityOf::<T>::contains_key(&caller), "Identity not cleared.");
		Ok(())
	}

	#[benchmark]
	fn request_judgement(r: Linear<1, { T::MaxRegistrars::get() }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		// Register the registrars
		add_registrars::<T>(r)?;

		// Create their main identity with x additional fields
		let info = T::IdentityInformation::create_identity_info();
		let caller_origin =
			<T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(caller.clone()));
		Identity::<T>::set_identity(caller_origin.clone(), Box::new(info))?;

		let registrar: T::AccountId = account("registrar", 0, SEED);
		// let registrar = T::Lookup::unlookup(registrar.clone());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), registrar.clone());

		assert_last_event::<T>(Event::<T>::JudgementRequested { who: caller, registrar }.into());

		Ok(())
	}

	#[benchmark]
	fn cancel_request(r: Linear<1, { T::MaxRegistrars::get() }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		// Register the registrars
		add_registrars::<T>(r)?;

		// Create their main identity with x additional fields
		let info = T::IdentityInformation::create_identity_info();
		let caller_origin =
			<T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(caller.clone()));
		Identity::<T>::set_identity(caller_origin.clone(), Box::new(info))?;

		let registrar: T::AccountId = account("registrar", 0, SEED);

		Identity::<T>::request_judgement(caller_origin, registrar.clone())?;

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), registrar.clone());

		assert_last_event::<T>(Event::<T>::JudgementUnrequested { who: caller, registrar }.into());

		Ok(())
	}
	#[benchmark]
	fn set_account_id(r: Linear<1, { T::MaxRegistrars::get() - 1 }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let caller_lookup = T::Lookup::unlookup(caller.clone());

		add_registrars::<T>(r)?;

		let registrar_origin = T::RegistrarOrigin::try_successful_origin()
			.expect("RegistrarOrigin has no successful origin required for the benchmark");
		Identity::<T>::add_registrar(registrar_origin, caller_lookup)?;

		let registrars = Registrars::<T>::get();
		ensure!(registrars[r as usize].as_ref().unwrap().account == caller, "id not set.");

		let new_account = T::Lookup::unlookup(account("new", 0, SEED));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), new_account);

		let updated_registrars = Registrars::<T>::get();
		ensure!(
			updated_registrars[r as usize].as_ref().unwrap().account == account("new", 0, SEED),
			"id not changed."
		);

		Ok(())
	}

	#[benchmark]
	fn set_fields(r: Linear<1, { T::MaxRegistrars::get() - 1 }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let caller_lookup = T::Lookup::unlookup(caller.clone());

		add_registrars::<T>(r)?;

		let registrar_origin = T::RegistrarOrigin::try_successful_origin()
			.expect("RegistrarOrigin has no successful origin required for the benchmark");
		Identity::<T>::add_registrar(registrar_origin, caller_lookup)?;

		let registrars = Registrars::<T>::get();
		ensure!(
			registrars[r as usize].as_ref().unwrap().fields == Default::default(),
			"fields already set."
		);
		let fields = T::IdentityInformation::all_fields();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), fields);

		let updated_registrars = Registrars::<T>::get();
		ensure!(
			updated_registrars[r as usize].as_ref().unwrap().fields != Default::default(),
			"fields not set."
		);

		Ok(())
	}

	#[benchmark]
	fn provide_judgement(
		r: Linear<1, { T::MaxRegistrars::get() - 1 }>,
	) -> Result<(), BenchmarkError> {
		// The user
		let user: T::AccountId = account("user", r, SEED);
		let user_origin =
			<T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(user.clone()));
		let user_lookup = <T::Lookup as StaticLookup>::unlookup(user.clone());

		let caller: T::AccountId = whitelisted_caller();
		let caller_lookup = T::Lookup::unlookup(caller.clone());

		add_registrars::<T>(r)?;

		let info = T::IdentityInformation::create_identity_info();
		let info_hash = T::Hashing::hash_of(&info);
		Identity::<T>::set_identity(user_origin.clone(), Box::new(info))?;

		let registrar_origin = T::RegistrarOrigin::try_successful_origin()
			.expect("RegistrarOrigin has no successful origin required for the benchmark");
		Identity::<T>::add_registrar(registrar_origin, caller_lookup)?;
		Identity::<T>::request_judgement(user_origin, caller.clone())?;

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), user_lookup, Judgement::Reasonable, info_hash);

		assert_last_event::<T>(
			Event::<T>::JudgementGiven { target: user, registrar: caller }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn kill_identity(
		r: Linear<1, { T::MaxRegistrars::get() }>,
		s: Linear<0, { T::MaxSubAccounts::get() }>,
	) -> Result<(), BenchmarkError> {
		add_registrars::<T>(r)?;

		let target: T::AccountId = account("target", 0, SEED);
		let target_origin: <T as frame_system::Config>::RuntimeOrigin =
			RawOrigin::Signed(target.clone()).into();
		let target_lookup = T::Lookup::unlookup(target.clone());

		let info = T::IdentityInformation::create_identity_info();
		Identity::<T>::set_identity(target_origin.clone(), Box::new(info.clone()))?;
		let _ = add_sub_accounts::<T>(&target, s)?;

		// User requests judgement from all the registrars, and they approve
		for i in 0..r {
			let registrar: T::AccountId = account("registrar", i, SEED);

			Identity::<T>::request_judgement(target_origin.clone(), registrar.clone())?;
			Identity::<T>::provide_judgement(
				RawOrigin::Signed(registrar).into(),
				target_lookup.clone(),
				Judgement::Reasonable,
				T::Hashing::hash_of(&info),
			)?;
		}

		ensure!(IdentityOf::<T>::contains_key(&target), "Identity not set");

		let origin =
			T::RegistrarOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, target_lookup);

		ensure!(!IdentityOf::<T>::contains_key(&target), "Identity not removed");

		Ok(())
	}

	#[benchmark]
	fn add_sub(s: Linear<0, { T::MaxSubAccounts::get() - 1 }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let _ = add_sub_accounts::<T>(&caller, s)?;
		let sub = account("new_sub", 0, SEED);
		let data = Data::Raw(vec![0; 32].try_into().unwrap());

		ensure!(SubsOf::<T>::get(&caller).len() as u32 == s, "Subs not set.");

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), T::Lookup::unlookup(sub), data);

		ensure!(SubsOf::<T>::get(&caller).len() as u32 == s + 1, "Subs not added.");

		Ok(())
	}

	#[benchmark]
	fn rename_sub(s: Linear<1, { T::MaxSubAccounts::get() }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let (sub, _) = add_sub_accounts::<T>(&caller, s)?.remove(0);
		let data = Data::Raw(vec![1; 32].try_into().unwrap());

		ensure!(SuperOf::<T>::get(&sub).unwrap().1 != data, "data already set");

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), T::Lookup::unlookup(sub.clone()), data.clone());

		ensure!(SuperOf::<T>::get(&sub).unwrap().1 == data, "data not set");

		Ok(())
	}

	#[benchmark]
	fn remove_sub(s: Linear<1, { T::MaxSubAccounts::get() }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let (sub, _) = add_sub_accounts::<T>(&caller, s)?.remove(0);
		ensure!(SuperOf::<T>::contains_key(&sub), "Sub doesn't exists");

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), T::Lookup::unlookup(sub.clone()));

		ensure!(!SuperOf::<T>::contains_key(&sub), "Sub not removed");

		Ok(())
	}

	#[benchmark]
	fn quit_sub(s: Linear<0, { T::MaxSubAccounts::get() - 1 }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let sup = account("super", 0, SEED);
		let _ = add_sub_accounts::<T>(&sup, s)?;
		let sup_origin = RawOrigin::Signed(sup).into();
		Identity::<T>::add_sub(
			sup_origin,
			T::Lookup::unlookup(caller.clone()),
			Data::Raw(vec![0; 32].try_into().unwrap()),
		)?;
		ensure!(SuperOf::<T>::contains_key(&caller), "Sub doesn't exists");

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()));

		ensure!(!SuperOf::<T>::contains_key(&caller), "Sub not removed");

		Ok(())
	}

	#[benchmark]
	fn add_username_authority() -> Result<(), BenchmarkError> {
		let origin =
			T::UsernameAuthorityOrigin::try_successful_origin().expect("can generate origin");

		let authority: T::AccountId = account("authority", 0, SEED);
		let authority_lookup = T::Lookup::unlookup(authority.clone());
		let suffix = bench_suffix();
		let allocation = 10;

		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, authority_lookup, suffix, allocation);

		assert_last_event::<T>(Event::<T>::AuthorityAdded { authority }.into());
		Ok(())
	}

	#[benchmark]
	fn remove_username_authority() -> Result<(), BenchmarkError> {
		let origin =
			T::UsernameAuthorityOrigin::try_successful_origin().expect("can generate origin");

		let authority: T::AccountId = account("authority", 0, SEED);
		let authority_lookup = T::Lookup::unlookup(authority.clone());
		let suffix = bench_suffix();
		let allocation = 10;

		assert_ok!(Identity::<T>::add_username_authority(
			origin.clone(),
			authority_lookup.clone(),
			suffix,
			allocation
		));

		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, authority_lookup);

		assert_last_event::<T>(Event::<T>::AuthorityRemoved { authority }.into());
		Ok(())
	}

	#[benchmark]
	fn set_username_for() -> Result<(), BenchmarkError> {
		// Set up a username authority.
		let auth_origin =
			T::UsernameAuthorityOrigin::try_successful_origin().expect("can generate origin");
		let authority: T::AccountId = account("authority", 0, SEED);
		let authority_lookup = T::Lookup::unlookup(authority.clone());
		let suffix = bench_suffix();
		let allocation = 10;

		Identity::<T>::add_username_authority(
			auth_origin,
			authority_lookup,
			suffix.clone(),
			allocation,
		)?;

		let username = bench_username();
		let bounded_username = bounded_username::<T>(username.clone(), suffix.clone());
		let encoded_username = Encode::encode(&bounded_username.to_vec());

		let public = sr25519_generate(0.into(), None);
		let who_account: T::AccountId = MultiSigner::Sr25519(public).into_account().into();
		let who_lookup = T::Lookup::unlookup(who_account.clone());

		let signature =
			MultiSignature::Sr25519(sr25519_sign(0.into(), &public, &encoded_username).unwrap());

		// Verify signature here to avoid surprise errors at runtime
		assert!(signature.verify(&encoded_username[..], &public.into()));

		#[extrinsic_call]
		_(RawOrigin::Signed(authority.clone()), who_lookup, username, Some(signature.into()));

		assert_has_event::<T>(
			Event::<T>::UsernameSet {
				who: who_account.clone(),
				username: bounded_username.clone(),
			}
			.into(),
		);
		assert_has_event::<T>(
			Event::<T>::PrimaryUsernameSet { who: who_account, username: bounded_username }.into(),
		);
		Ok(())
	}

	#[benchmark]
	fn accept_username() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let username = bounded_username::<T>(bench_username(), bench_suffix());

		Identity::<T>::queue_acceptance(&caller, username.clone());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), username.clone());

		assert_last_event::<T>(Event::<T>::UsernameSet { who: caller, username }.into());
		Ok(())
	}

	#[benchmark]
	fn remove_expired_approval() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let username = bounded_username::<T>(bench_username(), bench_suffix());
		Identity::<T>::queue_acceptance(&caller, username.clone());

		let expected_exiration =
			frame_system::Pallet::<T>::block_number() + T::PendingUsernameExpiration::get();

		run_to_block::<T>(expected_exiration + One::one());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), username);

		assert_last_event::<T>(Event::<T>::PreapprovalExpired { whose: caller }.into());
		Ok(())
	}

	#[benchmark]
	fn set_primary_username() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let first_username = bounded_username::<T>(bench_username(), bench_suffix());
		let second_username = bounded_username::<T>(b"slowbenchmark".to_vec(), bench_suffix());

		// First one will be set as primary. Second will not be.
		Identity::<T>::insert_username(&caller, first_username);
		Identity::<T>::insert_username(&caller, second_username.clone());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), second_username.clone());

		assert_last_event::<T>(
			Event::<T>::PrimaryUsernameSet { who: caller, username: second_username }.into(),
		);
		Ok(())
	}

	#[benchmark]
	fn remove_dangling_username() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let first_username = bounded_username::<T>(bench_username(), bench_suffix());
		let second_username = bounded_username::<T>(b"slowbenchmark".to_vec(), bench_suffix());

		// First one will be set as primary. Second will not be.
		Identity::<T>::insert_username(&caller, first_username);
		Identity::<T>::insert_username(&caller, second_username.clone());

		// User calls `clear_identity`, leaving their second username as "dangling"
		Identity::<T>::clear_identity(RawOrigin::Signed(caller.clone()).into())?;

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), second_username.clone());

		assert_last_event::<T>(
			Event::<T>::DanglingUsernameRemoved { who: caller, username: second_username }.into(),
		);
		Ok(())
	}

	impl_benchmark_test_suite!(Identity, crate::tests::new_test_ext(), crate::tests::Test);
}
