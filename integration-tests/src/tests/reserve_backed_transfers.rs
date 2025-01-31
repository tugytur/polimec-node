// Polimec Blockchain – https://www.polimec.org/
// Copyright (C) Polimec 2022. All rights reserved.

// The Polimec Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The Polimec Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::*;
use frame_support::{
	traits::{
		fungible::{Inspect as FungibleInspect, Unbalanced},
		fungibles::{Inspect, Mutate},
		PalletInfoAccess,
	},
	weights::WeightToFee,
};
use sp_runtime::DispatchError;
use xcm_emulator::Parachain;

const RESERVE_TRANSFER_AMOUNT: u128 = 10_0_000_000_000; // 10 DOT
const MAX_REF_TIME: u64 = 5_000_000_000;
const MAX_PROOF_SIZE: u64 = 200_000;

fn create_asset_on_asset_hub(asset_id: u32) {
	if asset_id == 10 {
		return;
	}
	let admin_account = AssetNet::account_id_of(FERDIE);
	AssetNet::execute_with(|| {
		assert_ok!(AssetHubAssets::force_create(
			AssetHubOrigin::root(),
			asset_id.into(),
			sp_runtime::MultiAddress::Id(admin_account.clone()),
			true,
			0_0_010_000_000u128
		));
	});
}

fn mint_asset_on_asset_hub_to(asset_id: u32, recipient: &AssetHubAccountId, amount: u128) {
	AssetNet::execute_with(|| {
		match asset_id {
			10 => {
				assert_ok!(AssetHubBalances::write_balance(recipient, amount));
			},
			_ => {
				assert_ok!(AssetHubAssets::mint_into(asset_id, recipient, amount));
			},
		}
		AssetHubSystem::reset_events();
	});
}

fn get_polimec_balances(asset_id: u32, user_account: AccountId) -> (u128, u128, u128, u128) {
	PolimecNet::execute_with(|| {
		(
			PolitestForeignAssets::balance(asset_id, user_account.clone()),
			PolimecBalances::balance(&user_account.clone()),
			PolitestForeignAssets::total_issuance(asset_id),
			PolimecBalances::total_issuance(),
		)
	})
}

fn get_asset_hub_balances(asset_id: u32, user_account: AccountId, polimec_account: AccountId) -> (u128, u128, u128) {
	AssetNet::execute_with(|| {
		match asset_id {
			// Asset id 10 equals Dot
			10 => (
				AssetHubBalances::balance(&user_account),
				AssetHubBalances::balance(&polimec_account),
				AssetHubBalances::total_issuance(),
			),
			_ => (
				AssetHubAssets::balance(asset_id, user_account.clone()),
				AssetHubAssets::balance(asset_id, polimec_account.clone()),
				AssetHubAssets::total_issuance(asset_id),
			),
		}
	})
}

/// Test the reserve based transfer from asset_hub to Polimec. Depending of the asset_id we
/// transfer either USDT, USDC and DOT.
fn test_reserve_to_polimec(asset_id: u32) {
	create_asset_on_asset_hub(asset_id);
	let asset_hub_asset_id: MultiLocation = match asset_id {
		10 => Parent.into(),
		_ => (PalletInstance(AssetHubAssets::index() as u8), GeneralIndex(asset_id as u128)).into(),
	};

	let alice_account = PolimecNet::account_id_of(ALICE);
	let polimec_sibling_account =
		AssetNet::sovereign_account_id_of((Parent, Parachain(PolimecNet::para_id().into())).into());
	let max_weight = Weight::from_parts(MAX_REF_TIME, MAX_PROOF_SIZE);

	mint_asset_on_asset_hub_to(asset_id, &alice_account, 100_0_000_000_000);

	let (
		polimec_prev_alice_asset_balance,
		polimec_prev_alice_plmc_balance,
		polimec_prev_asset_issuance,
		polimec_prev_plmc_issuance,
	) = get_polimec_balances(asset_id, alice_account.clone());

	// check AssetHub's pre transfer balances and issuance
	let (asset_hub_prev_alice_asset_balance, asset_hub_prev_polimec_asset_balance, asset_hub_prev_asset_issuance) =
		get_asset_hub_balances(asset_id, alice_account.clone(), polimec_sibling_account.clone());

	AssetNet::execute_with(|| {
		let asset_transfer: MultiAsset = (asset_hub_asset_id, RESERVE_TRANSFER_AMOUNT).into();
		let origin = AssetHubOrigin::signed(alice_account.clone());
		let dest: VersionedMultiLocation = ParentThen(X1(Parachain(PolimecNet::para_id().into()))).into();

		let beneficiary: VersionedMultiLocation =
			AccountId32 { network: None, id: alice_account.clone().into() }.into();
		let assets: VersionedMultiAssets = asset_transfer.into();
		let fee_asset_item = 0;
		let weight_limit = Unlimited;

		let call = AssetHubXcmPallet::limited_reserve_transfer_assets(
			origin,
			bx!(dest),
			bx!(beneficiary),
			bx!(assets),
			fee_asset_item,
			weight_limit,
		);
		assert_ok!(call);
	});

	// check the transfer was not blocked by our our xcm configured
	PolimecNet::execute_with(|| {
		assert_expected_events!(
			PolimecNet,
			vec![
				PolimecEvent::MessageQueue(pallet_message_queue::Event::Processed {success: true, ..}) => {},
			]
		);
	});

	let (
		polimec_post_alice_asset_balance,
		polimec_post_alice_plmc_balance,
		polimec_post_asset_issuance,
		polimec_post_plmc_issuance,
	) = get_polimec_balances(asset_id, alice_account.clone());

	let (asset_hub_post_alice_asset_balance, asset_hub_post_polimec_asset_balance, asset_hub_post_asset_issuance) =
		get_asset_hub_balances(asset_id, alice_account.clone(), polimec_sibling_account.clone());

	let polimec_delta_alice_asset_balance = polimec_post_alice_asset_balance.abs_diff(polimec_prev_alice_asset_balance);
	let polimec_delta_alice_plmc_balance = polimec_post_alice_plmc_balance.abs_diff(polimec_prev_alice_plmc_balance);
	let polimec_delta_asset_issuance = polimec_post_asset_issuance.abs_diff(polimec_prev_asset_issuance);
	let polimec_delta_plmc_issuance = polimec_post_plmc_issuance.abs_diff(polimec_prev_plmc_issuance);
	let asset_hub_delta_alice_asset_balance =
		asset_hub_post_alice_asset_balance.abs_diff(asset_hub_prev_alice_asset_balance);
	let asset_hub_delta_polimec_asset_balance =
		asset_hub_post_polimec_asset_balance.abs_diff(asset_hub_prev_polimec_asset_balance);
	let asset_hub_delta_asset_issuance = asset_hub_post_asset_issuance.abs_diff(asset_hub_prev_asset_issuance);

	assert!(
	    polimec_delta_alice_asset_balance >= RESERVE_TRANSFER_AMOUNT - politest_runtime::WeightToFee::weight_to_fee(&max_weight) &&
	    polimec_delta_alice_asset_balance <= RESERVE_TRANSFER_AMOUNT,
	    "Polimec alice_account.clone() Asset balance should have increased by at least the transfer amount minus the XCM execution fee"
	);

	assert!(
		polimec_delta_asset_issuance >=
			RESERVE_TRANSFER_AMOUNT - politest_runtime::WeightToFee::weight_to_fee(&max_weight) &&
			polimec_delta_asset_issuance <= RESERVE_TRANSFER_AMOUNT,
		"Polimec Asset issuance should have increased by at least the transfer amount minus the XCM execution fee"
	);

	// We overapproximate the fee for delivering the assets to polimec. The actual fee is
	// probably lower.
	let fee = system_parachains_constants::polkadot::fee::WeightToFee::weight_to_fee(&max_weight);
	assert!(
		asset_hub_delta_alice_asset_balance <= RESERVE_TRANSFER_AMOUNT + fee &&
			asset_hub_delta_alice_asset_balance >= RESERVE_TRANSFER_AMOUNT,
		"AssetHub alice_account.clone() Asset balance should have decreased by the transfer amount"
	);

	assert!(
		asset_hub_delta_polimec_asset_balance == RESERVE_TRANSFER_AMOUNT,
		"The USDT balance of Polimec's sovereign account on AssetHub should receive the transfer amount"
	);

	assert!(
		asset_hub_delta_asset_issuance == 0u128,
		"AssetHub's USDT issuance should not change, since it acts as a reserve for that asset"
	);

	assert_eq!(
		polimec_delta_alice_plmc_balance, 0,
		"Polimec alice_account.clone() PLMC balance should not have changed"
	);

	assert_eq!(polimec_delta_plmc_issuance, 0, "Polimec PLMC issuance should not have changed");
}

fn test_polimec_to_reserve(asset_id: u32) {
	create_asset_on_asset_hub(asset_id);
	let asset_hub_asset_id: MultiLocation = match asset_id {
		10 => Parent.into(),
		_ => ParentThen(X3(
			Parachain(AssetNet::para_id().into()),
			PalletInstance(AssetHubAssets::index() as u8),
			GeneralIndex(asset_id as u128),
		))
		.into(),
	};

	let alice_account = PolimecNet::account_id_of(ALICE);
	let polimec_sibling_account =
		AssetNet::sovereign_account_id_of((Parent, Parachain(PolimecNet::para_id().into())).into());
	let max_weight = Weight::from_parts(MAX_REF_TIME, MAX_PROOF_SIZE);

	mint_asset_on_asset_hub_to(asset_id, &polimec_sibling_account, RESERVE_TRANSFER_AMOUNT + 1_0_000_000_000);

	PolimecNet::execute_with(|| {
		assert_ok!(PolimecForeignAssets::mint_into(
			asset_id,
			&alice_account,
			RESERVE_TRANSFER_AMOUNT + 1_0_000_000_000
		));
	});

	let (
		polimec_prev_alice_asset_balance,
		polimec_prev_alice_plmc_balance,
		polimec_prev_asset_issuance,
		polimec_prev_plmc_issuance,
	) = get_polimec_balances(asset_id, alice_account.clone());

	// check AssetHub's pre transfer balances and issuance
	let (asset_hub_prev_alice_asset_balance, asset_hub_prev_polimec_asset_balance, asset_hub_prev_asset_issuance) =
		get_asset_hub_balances(asset_id, alice_account.clone(), polimec_sibling_account.clone());

	PolimecNet::execute_with(|| {
		let asset_transfer: MultiAsset = (asset_hub_asset_id, RESERVE_TRANSFER_AMOUNT + 1_0_000_000_000).into();
		let origin = PolimecOrigin::signed(alice_account.clone());
		let dest: VersionedMultiLocation = ParentThen(X1(Parachain(AssetNet::para_id().into()))).into();

		let beneficiary: VersionedMultiLocation =
			AccountId32 { network: None, id: alice_account.clone().into() }.into();
		let assets: VersionedMultiAssets = asset_transfer.into();
		let fee_asset_item = 0;
		let weight_limit = Unlimited;

		let call = PolimecXcmPallet::limited_reserve_transfer_assets(
			origin,
			bx!(dest),
			bx!(beneficiary),
			bx!(assets),
			fee_asset_item,
			weight_limit,
		);
		assert_ok!(call);
	});

	// check that the xcm was not blocked
	AssetNet::execute_with(|| {
		assert_expected_events!(
			AssetNet,
			vec![
				AssetHubEvent::MessageQueue(pallet_message_queue::Event::Processed {success: true, ..}) => {},
			]
		);
	});

	let (
		polimec_post_alice_asset_balance,
		polimec_post_alice_plmc_balance,
		polimec_post_asset_issuance,
		polimec_post_plmc_issuance,
	) = get_polimec_balances(asset_id, alice_account.clone());

	let (asset_hub_post_alice_asset_balance, asset_hub_post_polimec_asset_balance, asset_hub_post_asset_issuance) =
		get_asset_hub_balances(asset_id, alice_account.clone(), polimec_sibling_account.clone());

	let polimec_delta_alice_asset_balance = polimec_post_alice_asset_balance.abs_diff(polimec_prev_alice_asset_balance);
	let polimec_delta_alice_plmc_balance = polimec_post_alice_plmc_balance.abs_diff(polimec_prev_alice_plmc_balance);
	let polimec_delta_asset_issuance = polimec_post_asset_issuance.abs_diff(polimec_prev_asset_issuance);
	let polimec_delta_plmc_issuance = polimec_post_plmc_issuance.abs_diff(polimec_prev_plmc_issuance);
	let asset_hub_delta_alice_asset_balance =
		asset_hub_post_alice_asset_balance.abs_diff(asset_hub_prev_alice_asset_balance);
	let asset_hub_delta_polimec_asset_balance =
		asset_hub_post_polimec_asset_balance.abs_diff(asset_hub_prev_polimec_asset_balance);
	let asset_hub_delta_asset_issuance = asset_hub_post_asset_issuance.abs_diff(asset_hub_prev_asset_issuance);

	assert_eq!(
		polimec_delta_alice_asset_balance,
		RESERVE_TRANSFER_AMOUNT + 1_0_000_000_000,
		"Polimec's alice_account Asset balance should decrease by the transfer amount"
	);

	assert_eq!(
		polimec_delta_asset_issuance,
		RESERVE_TRANSFER_AMOUNT + 1_0_000_000_000,
		"Polimec's Asset issuance should decrease by transfer amount due to burn"
	);

	assert_eq!(polimec_delta_plmc_issuance, 0, "Polimec's PLMC issuance should not change, since all xcm token transfer are done in Asset, and no fees are burnt since no extrinsics are dispatched");
	assert_eq!(polimec_delta_alice_plmc_balance, 0, "Polimec's Alice PLMC should not change");

	assert!(
	    asset_hub_delta_alice_asset_balance >=
	        RESERVE_TRANSFER_AMOUNT &&
	        asset_hub_delta_alice_asset_balance <= RESERVE_TRANSFER_AMOUNT + 1_0_000_000_000,
	    "AssetHub's alice_account Asset balance should increase by at least the transfer amount minus the max allowed fees"
	);

	assert!(
		asset_hub_delta_polimec_asset_balance >= RESERVE_TRANSFER_AMOUNT &&
			asset_hub_delta_polimec_asset_balance <= RESERVE_TRANSFER_AMOUNT + 1_0_000_000_000,
		"Polimecs sovereign account on asset hub should have transferred Asset amount to Alice"
	);

	assert!(
	    asset_hub_delta_asset_issuance <= system_parachains_constants::polkadot::fee::WeightToFee::weight_to_fee(&max_weight),
	    "AssetHub's Asset issuance should not change, since it acts as a reserve for that asset (except for fees which are burnt)"
	);
}

/// Test reserve based transfer of USDT from AssetHub to Polimec.
#[test]
fn reserve_usdt_to_polimec() {
	let asset_id = 1984;
	test_reserve_to_polimec(asset_id);
}

/// Test reserve based transfer of USDC from AssetHub to Polimec.
#[test]
fn reserve_usdc_to_polimec() {
	let asset_id = 1337;
	test_reserve_to_polimec(asset_id);
}

/// Test reserve based transfer of DOT from AssetHub to Polimec.
#[test]
fn reserve_dot_to_polimec() {
	let asset_id = 10;
	test_reserve_to_polimec(asset_id);
}

/// Test that reserve based transfer of random asset from AssetHub to Polimec fails.
#[test]
#[should_panic]
fn reserve_random_asset_to_polimec() {
	let asset_id = 69;
	test_reserve_to_polimec(asset_id);
}

/// Test transfer of reserve-based DOT from Polimec back to AssetHub.
#[test]
fn polimec_usdt_to_reserve() {
	let asset_id = 1984;
	test_polimec_to_reserve(asset_id);
}

/// Test transfer of reserve-based DOT from Polimec back to AssetHub.
#[test]
fn polimec_usdc_to_reserve() {
	let asset_id = 1337;
	test_polimec_to_reserve(asset_id);
}

/// Test transfer of reserve-based DOT from Polimec back to AssetHub.
#[test]
fn polimec_dot_to_reserve() {
	let asset_id = 10;
	test_polimec_to_reserve(asset_id);
}

#[test]
fn test_user_cannot_create_foreign_asset_on_polimec() {
	PolimecNet::execute_with(|| {
		let admin = AssetNet::account_id_of(ALICE);
		assert_noop!(
			PolimecForeignAssets::create(
				PolimecOrigin::signed(admin.clone()),
				69.into(),
				sp_runtime::MultiAddress::Id(admin),
				0_0_010_000_000u128,
			),
			DispatchError::BadOrigin
		);
	});
}
