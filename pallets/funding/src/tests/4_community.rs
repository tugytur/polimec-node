use super::*;

#[cfg(test)]
mod round_flow {
	use super::*;

	#[cfg(test)]
	mod success {
		use super::*;
		use frame_support::traits::fungibles::metadata::Inspect;
		use sp_runtime::bounded_vec;
		use std::collections::HashSet;

		#[test]
		fn community_round_completed() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let _ = inst.create_remainder_contributing_project(
				default_project_metadata(ISSUER_1),
				ISSUER_1,
				default_evaluations(),
				default_bids(),
				default_community_buys(),
			);
		}

		#[test]
		fn multiple_contribution_projects_completed() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let project1 = default_project_metadata(ISSUER_1);
			let project2 = default_project_metadata(ISSUER_2);
			let project3 = default_project_metadata(ISSUER_3);
			let project4 = default_project_metadata(ISSUER_4);
			let evaluations = default_evaluations();
			let bids = default_bids();
			let community_buys = default_community_buys();

			inst.create_remainder_contributing_project(
				project1,
				ISSUER_1,
				evaluations.clone(),
				bids.clone(),
				community_buys.clone(),
			);
			inst.create_remainder_contributing_project(
				project2,
				ISSUER_2,
				evaluations.clone(),
				bids.clone(),
				community_buys.clone(),
			);
			inst.create_remainder_contributing_project(
				project3,
				ISSUER_3,
				evaluations.clone(),
				bids.clone(),
				community_buys.clone(),
			);
			inst.create_remainder_contributing_project(project4, ISSUER_4, evaluations, bids, community_buys);
		}

		#[test]
		fn community_round_ends_on_all_ct_sold_exact() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let bids = vec![
				BidParams::new_with_defaults(BIDDER_1, 40_000 * CT_UNIT),
				BidParams::new_with_defaults(BIDDER_2, 10_000 * CT_UNIT),
			];
			let project_id = inst.create_community_contributing_project(
				default_project_metadata(ISSUER_1),
				ISSUER_1,
				default_evaluations(),
				bids,
			);
			const BOB: AccountId = 808;

			let remaining_ct = inst.get_project_details(project_id).remaining_contribution_tokens;
			let ct_price = inst.get_project_details(project_id).weighted_average_price.expect("CT Price should exist");

			let contributions = vec![ContributionParams::new(BOB, remaining_ct, 1u8, AcceptedFundingAsset::USDT)];
			let plmc_fundings = inst.calculate_contributed_plmc_spent(contributions.clone(), ct_price);
			let plmc_existential_deposits = plmc_fundings.accounts().existential_deposits();
			let foreign_asset_fundings =
				inst.calculate_contributed_funding_asset_spent(contributions.clone(), ct_price);

			inst.mint_plmc_to(plmc_fundings.clone());
			inst.mint_plmc_to(plmc_existential_deposits.clone());
			inst.mint_foreign_asset_to(foreign_asset_fundings.clone());

			// Buy remaining CTs
			inst.contribute_for_users(project_id, contributions)
				.expect("The Buyer should be able to buy the exact amount of remaining CTs");
			inst.advance_time(2u64).unwrap();
			// Check remaining CTs is 0
			assert_eq!(
				inst.get_project_details(project_id).remaining_contribution_tokens,
				0,
				"There are still remaining CTs"
			);

			// Check project is in FundingEnded state
			assert_eq!(inst.get_project_details(project_id).status, ProjectStatus::FundingSuccessful);

			inst.do_free_plmc_assertions(plmc_existential_deposits);
			inst.do_free_foreign_asset_assertions(vec![UserToForeignAssets::<TestRuntime>::new(
				BOB,
				0_u128,
				AcceptedFundingAsset::USDT.to_assethub_id(),
			)]);
			inst.do_reserved_plmc_assertions(
				vec![plmc_fundings[0].clone()],
				HoldReason::Participation(project_id).into(),
			);
			inst.do_contribution_transferred_foreign_asset_assertions(foreign_asset_fundings, project_id);
		}

		#[test]
		fn round_has_total_ct_allocation_minus_auction_sold() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let project_metadata = default_project_metadata(ISSUER_1);

			let project_id = inst.create_community_contributing_project(
				project_metadata.clone(),
				ISSUER_1,
				default_evaluations(),
				default_bids(),
			);
			let project_details = inst.get_project_details(project_id);
			let bid_ct_sold: BalanceOf<TestRuntime> = inst.execute(|| {
				Bids::<TestRuntime>::iter_prefix_values((project_id,))
					.fold(Zero::zero(), |acc, bid| acc + bid.final_ct_amount)
			});
			assert_eq!(
				project_details.remaining_contribution_tokens,
				project_metadata.total_allocation_size - bid_ct_sold
			);

			let contributions = vec![(BUYER_1, project_details.remaining_contribution_tokens).into()];

			let plmc_contribution_funding = inst.calculate_contributed_plmc_spent(
				contributions.clone(),
				project_details.weighted_average_price.unwrap(),
			);
			let plmc_existential_deposits = plmc_contribution_funding.accounts().existential_deposits();
			inst.mint_plmc_to(plmc_contribution_funding.clone());
			inst.mint_plmc_to(plmc_existential_deposits.clone());

			let foreign_asset_contribution_funding = inst.calculate_contributed_funding_asset_spent(
				contributions.clone(),
				project_details.weighted_average_price.unwrap(),
			);
			inst.mint_foreign_asset_to(foreign_asset_contribution_funding.clone());

			inst.contribute_for_users(project_id, contributions).unwrap();

			assert_eq!(inst.get_project_details(project_id).remaining_contribution_tokens, 0);
		}

		#[test]
		fn different_decimals_ct_works_as_expected() {
			// Setup some base values to compare different decimals
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let ed = inst.get_ed();
			let default_project_metadata = default_project_metadata(ISSUER_1);
			let original_decimal_aware_price = default_project_metadata.minimum_price;
			let original_price = <TestRuntime as Config>::PriceProvider::convert_back_to_normal_price(
				original_decimal_aware_price,
				USD_DECIMALS,
				default_project_metadata.token_information.decimals,
			)
			.unwrap();
			let usable_plmc_price = inst.execute(|| {
				<TestRuntime as Config>::PriceProvider::get_decimals_aware_price(
					PLMC_FOREIGN_ID,
					USD_DECIMALS,
					PLMC_DECIMALS,
				)
				.unwrap()
			});
			let usdt_price = inst.execute(|| {
				<TestRuntime as Config>::PriceProvider::get_decimals_aware_price(
					AcceptedFundingAsset::USDT.to_assethub_id(),
					USD_DECIMALS,
					ForeignAssets::decimals(AcceptedFundingAsset::USDT.to_assethub_id()),
				)
				.unwrap()
			});
			let usdc_price = inst.execute(|| {
				<TestRuntime as Config>::PriceProvider::get_decimals_aware_price(
					AcceptedFundingAsset::USDC.to_assethub_id(),
					USD_DECIMALS,
					ForeignAssets::decimals(AcceptedFundingAsset::USDC.to_assethub_id()),
				)
				.unwrap()
			});
			let dot_price = inst.execute(|| {
				<TestRuntime as Config>::PriceProvider::get_decimals_aware_price(
					AcceptedFundingAsset::DOT.to_assethub_id(),
					USD_DECIMALS,
					ForeignAssets::decimals(AcceptedFundingAsset::DOT.to_assethub_id()),
				)
				.unwrap()
			});

			let mut funding_assets_cycle =
				vec![AcceptedFundingAsset::USDT, AcceptedFundingAsset::USDC, AcceptedFundingAsset::DOT]
					.into_iter()
					.cycle();

			let mut total_fundings_ct = Vec::new();
			let mut total_fundings_usd = Vec::new();
			let mut total_fundings_plmc = Vec::new();

			let mut decimal_test = |decimals: u8| {
				let funding_asset = funding_assets_cycle.next().unwrap();
				let funding_asset_usd_price = match funding_asset {
					AcceptedFundingAsset::USDT => usdt_price,
					AcceptedFundingAsset::USDC => usdc_price,
					AcceptedFundingAsset::DOT => dot_price,
				};

				let mut project_metadata = default_project_metadata.clone();
				project_metadata.token_information.decimals = decimals;
				project_metadata.minimum_price =
					<TestRuntime as Config>::PriceProvider::calculate_decimals_aware_price(
						original_price,
						USD_DECIMALS,
						decimals,
					)
					.unwrap();

				project_metadata.total_allocation_size = 1_000_000 * 10u128.pow(decimals as u32);
				project_metadata.mainnet_token_max_supply = project_metadata.total_allocation_size;
				project_metadata.participation_currencies = bounded_vec!(funding_asset);

				let issuer: AccountIdOf<TestRuntime> = (10_000 + inst.get_new_nonce()).try_into().unwrap();
				let evaluations = inst.generate_successful_evaluations(
					project_metadata.clone(),
					default_evaluators(),
					default_weights(),
				);
				let project_id =
					inst.create_community_contributing_project(project_metadata.clone(), issuer, evaluations, vec![]);

				let total_funding_ct = project_metadata.total_allocation_size;
				let total_funding_usd = project_metadata.minimum_price.saturating_mul_int(total_funding_ct);
				let total_funding_plmc = usable_plmc_price.reciprocal().unwrap().saturating_mul_int(total_funding_usd);
				let total_funding_funding_asset =
					funding_asset_usd_price.reciprocal().unwrap().saturating_mul_int(total_funding_usd);

				total_fundings_ct.push(total_funding_ct);
				total_fundings_usd.push(total_funding_usd);
				total_fundings_plmc.push(total_funding_plmc);

				// Every project should want to raise 10MM USD
				assert_eq!(total_funding_usd, 10_000_000 * USD_UNIT);

				// Every project should produce the same PLMC bond when having the full funding at multiplier 1.
				assert_close_enough!(total_funding_plmc, 1_190_476 * PLMC, Perquintill::from_float(0.999));

				// Every project should have a different amount of CTs to raise, depending on their decimals
				assert_eq!(total_funding_ct, 1_000_000 * 10u128.pow(decimals as u32));

				// Buying all the remaining tokens. This is a fixed USD value, but the extrinsic amount depends on CT decimals.
				inst.mint_plmc_to(vec![UserToPLMCBalance::new(BUYER_1, total_funding_plmc + ed)]);
				inst.mint_foreign_asset_to(vec![UserToForeignAssets::new(
					BUYER_1,
					total_funding_funding_asset,
					funding_asset.to_assethub_id(),
				)]);

				assert_ok!(inst.execute(|| PolimecFunding::community_contribute(
					RuntimeOrigin::signed(BUYER_1),
					get_mock_jwt_with_cid(
						BUYER_1,
						InvestorType::Retail,
						generate_did_from_account(BUYER_1),
						project_metadata.clone().policy_ipfs_cid.unwrap()
					),
					project_id,
					total_funding_ct,
					1u8.try_into().unwrap(),
					funding_asset,
				)));

				// the remaining tokens should be zero
				assert_eq!(inst.get_project_details(project_id).remaining_contribution_tokens, 0);

				// We can successfully finish the project
				inst.finish_funding(project_id).unwrap();
			};

			for decimals in 6..=18 {
				decimal_test(decimals);
			}

			// Since we use the same original price and allocation size and adjust for decimals,
			// the USD and PLMC amounts should be the same
			assert!(total_fundings_usd.iter().all(|x| *x == total_fundings_usd[0]));
			assert!(total_fundings_plmc.iter().all(|x| *x == total_fundings_plmc[0]));

			// CT amounts however should be different from each other
			let mut hash_set_1 = HashSet::new();
			for amount in total_fundings_ct {
				assert!(!hash_set_1.contains(&amount));
				hash_set_1.insert(amount);
			}
		}
	}
}

#[cfg(test)]
mod community_contribute_extrinsic {
	use super::*;

	#[cfg(test)]
	mod success {
		use super::*;
		use frame_support::dispatch::DispatchResultWithPostInfo;

		#[test]
		fn evaluation_bond_counts_towards_contribution() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let project_metadata = default_project_metadata(ISSUER_1);

			const BOB: AccountId = 42069;
			const CARL: AccountId = 420691;
			let mut evaluations = default_evaluations();
			let bob_evaluation: UserToUSDBalance<TestRuntime> = (BOB, 1337 * USD_UNIT).into();
			let carl_evaluation: UserToUSDBalance<TestRuntime> = (CARL, 1337 * USD_UNIT).into();
			evaluations.push(bob_evaluation.clone());
			evaluations.push(carl_evaluation.clone());

			let project_id = inst.create_community_contributing_project(
				project_metadata.clone(),
				ISSUER_1,
				evaluations,
				default_bids(),
			);
			let ct_price = inst.get_project_details(project_id).weighted_average_price.unwrap();
			let plmc_price = <TestRuntime as Config>::PriceProvider::get_decimals_aware_price(
				PLMC_FOREIGN_ID,
				USD_DECIMALS,
				PLMC_DECIMALS,
			)
			.unwrap();

			let evaluation_plmc_bond =
				inst.execute(|| Balances::balance_on_hold(&HoldReason::Evaluation(project_id).into(), &BOB));
			let slashable_plmc = <TestRuntime as Config>::EvaluatorSlash::get() * evaluation_plmc_bond;
			let usable_plmc = evaluation_plmc_bond - slashable_plmc;

			let usable_usd = plmc_price.checked_mul_int(usable_plmc).unwrap();
			let slashable_usd = plmc_price.checked_mul_int(slashable_plmc).unwrap();

			let usable_ct = ct_price.reciprocal().unwrap().saturating_mul_int(usable_usd);
			let slashable_ct = ct_price.reciprocal().unwrap().saturating_mul_int(slashable_usd);

			// Can't contribute with only the evaluation bond
			inst.execute(|| {
				assert_noop!(
					Pallet::<TestRuntime>::community_contribute(
						RuntimeOrigin::signed(BOB),
						get_mock_jwt_with_cid(
							BOB,
							InvestorType::Retail,
							generate_did_from_account(BOB),
							project_metadata.clone().policy_ipfs_cid.unwrap()
						),
						project_id,
						usable_ct + slashable_ct,
						1u8.try_into().unwrap(),
						AcceptedFundingAsset::USDT,
					),
					Error::<TestRuntime>::ParticipantNotEnoughFunds
				);
			});

			// Can partially use the usable evaluation bond (half in this case)
			let contribution_usdt =
				inst.calculate_contributed_funding_asset_spent(vec![(BOB, usable_ct / 2).into()], ct_price);
			inst.mint_foreign_asset_to(contribution_usdt.clone());
			inst.execute(|| {
				assert_ok!(Pallet::<TestRuntime>::community_contribute(
					RuntimeOrigin::signed(BOB),
					get_mock_jwt_with_cid(
						BOB,
						InvestorType::Retail,
						generate_did_from_account(BOB),
						project_metadata.clone().policy_ipfs_cid.unwrap()
					),
					project_id,
					usable_ct / 2,
					1u8.try_into().unwrap(),
					AcceptedFundingAsset::USDT,
				));
			});

			// Can use the full evaluation bond
			let contribution_usdt =
				inst.calculate_contributed_funding_asset_spent(vec![(CARL, usable_ct).into()], ct_price);
			inst.mint_foreign_asset_to(contribution_usdt.clone());
			inst.execute(|| {
				assert_ok!(Pallet::<TestRuntime>::community_contribute(
					RuntimeOrigin::signed(CARL),
					get_mock_jwt_with_cid(
						CARL,
						InvestorType::Retail,
						generate_did_from_account(CARL),
						project_metadata.clone().policy_ipfs_cid.unwrap()
					),
					project_id,
					usable_ct,
					1u8.try_into().unwrap(),
					AcceptedFundingAsset::USDT,
				));
			});
		}

		#[test]
		fn evaluation_bond_used_on_failed_bid_can_be_reused_on_contribution() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let bob = 42069;
			let project_metadata = default_project_metadata(ISSUER_1);
			// An evaluator that did a bid but it was not accepted at the end of the auction, can use that PLMC for contributing
			let mut evaluations = default_evaluations();
			let bob_evaluation = (bob, 1337 * USD_UNIT).into();
			evaluations.push(bob_evaluation);

			let bids = default_bids();
			let bob_bid: BidParams<TestRuntime> = (bob, 1337 * CT_UNIT).into();
			let all_bids = bids.iter().chain(vec![bob_bid.clone()].iter()).cloned().collect_vec();

			let project_id = inst.create_auctioning_project(default_project_metadata(ISSUER_2), ISSUER_2, evaluations);

			let evaluation_plmc_bond =
				inst.execute(|| Balances::balance_on_hold(&HoldReason::Evaluation(project_id).into(), &bob));
			let slashable_plmc_bond = <TestRuntime as Config>::EvaluatorSlash::get() * evaluation_plmc_bond;
			let usable_plmc_bond = evaluation_plmc_bond - slashable_plmc_bond;

			let bids_plmc = inst.calculate_auction_plmc_charged_from_all_bids_made_or_with_bucket(
				&all_bids,
				project_metadata.clone(),
				None,
			);
			let bids_existential_deposits = bids_plmc.accounts().existential_deposits();
			inst.mint_plmc_to(bids_plmc.clone());
			inst.mint_plmc_to(bids_existential_deposits.clone());

			let bids_foreign = inst.calculate_auction_funding_asset_charged_from_all_bids_made_or_with_bucket(
				&all_bids,
				project_metadata.clone(),
				None,
			);
			inst.mint_foreign_asset_to(bids_foreign.clone());

			inst.bid_for_users(project_id, bids).unwrap();

			let auction_end = <TestRuntime as Config>::AuctionOpeningDuration::get() +
				<TestRuntime as Config>::AuctionClosingDuration::get();
			inst.advance_time(auction_end - 1).unwrap();
			assert_eq!(inst.get_project_details(project_id).status, ProjectStatus::AuctionClosing);
			inst.bid_for_users(project_id, vec![bob_bid]).unwrap();

			inst.start_community_funding(project_id).unwrap();
			assert_eq!(inst.get_project_details(project_id).status, ProjectStatus::CommunityRound);

			let plmc_price = <TestRuntime as Config>::PriceProvider::get_decimals_aware_price(
				PLMC_FOREIGN_ID,
				USD_DECIMALS,
				PLMC_DECIMALS,
			)
			.unwrap();
			let wap = inst.get_project_details(project_id).weighted_average_price.unwrap();

			let usable_usd = plmc_price.saturating_mul_int(usable_plmc_bond);
			let usable_ct = wap.reciprocal().unwrap().saturating_mul_int(usable_usd);

			let bob_contribution = (bob, 1337 * CT_UNIT).into();
			let contribution_usdt = inst.calculate_contributed_funding_asset_spent(vec![bob_contribution], wap);
			inst.mint_foreign_asset_to(contribution_usdt.clone());
			inst.execute(|| {
				assert_ok!(Pallet::<TestRuntime>::community_contribute(
					RuntimeOrigin::signed(bob),
					get_mock_jwt_with_cid(
						bob,
						InvestorType::Retail,
						generate_did_from_account(bob),
						project_metadata.clone().policy_ipfs_cid.unwrap()
					),
					project_id,
					usable_ct,
					1u8.try_into().unwrap(),
					AcceptedFundingAsset::USDT,
				));
			});
		}

		#[test]
		fn contribute_with_multiple_currencies() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let mut project_metadata_all = default_project_metadata(ISSUER_1);
			project_metadata_all.participation_currencies =
				vec![AcceptedFundingAsset::USDT, AcceptedFundingAsset::USDC, AcceptedFundingAsset::DOT]
					.try_into()
					.unwrap();

			let project_id = inst.create_community_contributing_project(
				project_metadata_all.clone(),
				ISSUER_1,
				default_evaluations(),
				default_bids(),
			);
			let usdt_contribution = ContributionParams::new(BUYER_1, 10_000 * CT_UNIT, 1u8, AcceptedFundingAsset::USDT);
			let usdc_contribution = ContributionParams::new(BUYER_2, 10_000 * CT_UNIT, 1u8, AcceptedFundingAsset::USDC);
			let dot_contribution = ContributionParams::new(BUYER_3, 10_000 * CT_UNIT, 1u8, AcceptedFundingAsset::DOT);

			let wap = inst.get_project_details(project_id).weighted_average_price.unwrap();

			let plmc_fundings = inst.calculate_contributed_plmc_spent(
				vec![usdt_contribution.clone(), usdc_contribution.clone(), dot_contribution.clone()],
				wap,
			);
			let plmc_existential_deposits = plmc_fundings.accounts().existential_deposits();

			let plmc_all_mints =
				inst.generic_map_operation(vec![plmc_fundings, plmc_existential_deposits], MergeOperation::Add);
			inst.mint_plmc_to(plmc_all_mints.clone());

			let asset_hub_fundings = inst.calculate_contributed_funding_asset_spent(
				vec![usdt_contribution.clone(), usdc_contribution.clone(), dot_contribution.clone()],
				wap,
			);
			inst.mint_foreign_asset_to(asset_hub_fundings.clone());

			assert_ok!(inst.contribute_for_users(
				project_id,
				vec![usdt_contribution.clone(), usdc_contribution.clone(), dot_contribution.clone()]
			));
		}

		fn test_contribution_setup(
			inst: &mut MockInstantiator,
			project_id: ProjectId,
			contributor: AccountIdOf<TestRuntime>,
			investor_type: InvestorType,
			u8_multiplier: u8,
		) -> DispatchResultWithPostInfo {
			let project_policy = inst.get_project_metadata(project_id).policy_ipfs_cid.unwrap();
			let jwt = get_mock_jwt_with_cid(
				contributor.clone(),
				investor_type,
				generate_did_from_account(contributor),
				project_policy,
			);
			let amount = 1000 * CT_UNIT;
			let multiplier = Multiplier::force_new(u8_multiplier);
			let wap = inst.get_project_details(project_id).weighted_average_price.unwrap();

			if u8_multiplier > 0 {
				let contribution = ContributionParams::<TestRuntime> {
					contributor: contributor.clone(),
					amount,
					multiplier,
					asset: AcceptedFundingAsset::USDT,
				};

				let necessary_plmc = inst.calculate_contributed_plmc_spent(vec![contribution.clone()], wap);
				let plmc_existential_amounts = necessary_plmc.accounts().existential_deposits();
				let necessary_usdt = inst.calculate_contributed_funding_asset_spent(vec![contribution.clone()], wap);

				inst.mint_plmc_to(necessary_plmc.clone());
				inst.mint_plmc_to(plmc_existential_amounts.clone());
				inst.mint_foreign_asset_to(necessary_usdt.clone());
			}
			inst.execute(|| {
				Pallet::<TestRuntime>::community_contribute(
					RuntimeOrigin::signed(contributor),
					jwt,
					project_id,
					amount,
					multiplier,
					AcceptedFundingAsset::USDT,
				)
			})
		}

		#[test]
		fn non_retail_multiplier_limits() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let mut project_metadata = default_project_metadata(ISSUER_1);
			project_metadata.mainnet_token_max_supply = 80_000_000 * CT_UNIT;
			project_metadata.total_allocation_size = 10_000_000 * CT_UNIT;
			project_metadata.bidding_ticket_sizes = BiddingTicketSizes {
				professional: TicketSize::new(5000 * USD_UNIT, None),
				institutional: TicketSize::new(5000 * USD_UNIT, None),
				phantom: Default::default(),
			};
			project_metadata.contributing_ticket_sizes = ContributingTicketSizes {
				retail: TicketSize::new(USD_UNIT, None),
				professional: TicketSize::new(USD_UNIT, None),
				institutional: TicketSize::new(USD_UNIT, None),
				phantom: Default::default(),
			};
			let evaluations =
				inst.generate_successful_evaluations(project_metadata.clone(), default_evaluators(), default_weights());
			let bids = inst.generate_bids_from_total_ct_percent(
				project_metadata.clone(),
				50,
				default_weights(),
				default_bidders(),
				default_multipliers(),
			);
			let project_id =
				inst.create_community_contributing_project(project_metadata.clone(), ISSUER_1, evaluations, bids);

			// Professional contributions: 0x multiplier should fail
			assert_err!(
				test_contribution_setup(&mut inst, project_id, BUYER_1, InvestorType::Professional, 0),
				Error::<TestRuntime>::ForbiddenMultiplier
			);
			// Professional contributions: 1 - 10x multiplier should work
			for multiplier in 1..=10u8 {
				assert_ok!(test_contribution_setup(
					&mut inst,
					project_id,
					BUYER_1,
					InvestorType::Professional,
					multiplier
				));
			}
			// Professional contributions: >=11x multiplier should fail
			for multiplier in 11..=50u8 {
				assert_err!(
					test_contribution_setup(&mut inst, project_id, BUYER_1, InvestorType::Professional, multiplier),
					Error::<TestRuntime>::ForbiddenMultiplier
				);
			}

			// Institutional contributions: 0x multiplier should fail
			assert_err!(
				test_contribution_setup(&mut inst, project_id, BUYER_2, InvestorType::Institutional, 0),
				Error::<TestRuntime>::ForbiddenMultiplier
			);
			// Institutional contributions: 1 - 25x multiplier should work
			for multiplier in 1..=25u8 {
				assert_ok!(test_contribution_setup(
					&mut inst,
					project_id,
					BUYER_2,
					InvestorType::Institutional,
					multiplier
				));
			}
			// Institutional contributions: >=26x multiplier should fail
			for multiplier in 26..=50u8 {
				assert_err!(
					test_contribution_setup(&mut inst, project_id, BUYER_2, InvestorType::Institutional, multiplier),
					Error::<TestRuntime>::ForbiddenMultiplier
				);
			}
		}

		#[test]
		fn retail_multiplier_limits() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let mut issuer: AccountId = 6969420;

			let mut create_project = |inst: &mut MockInstantiator| {
				issuer += 1;
				inst.create_community_contributing_project(
					default_project_metadata(issuer),
					issuer,
					default_evaluations(),
					default_bids(),
				)
			};

			let max_allowed_multipliers_map = vec![(2, 1), (4, 2), (9, 4), (24, 7), (25, 10)];

			let mut previous_projects_created = 0;
			for (projects_participated_amount, max_allowed_multiplier) in max_allowed_multipliers_map {
				(previous_projects_created..projects_participated_amount - 1).for_each(|_| {
					let project_id = create_project(&mut inst);
					assert_ok!(test_contribution_setup(&mut inst, project_id, BUYER_1, InvestorType::Retail, 1));
				});

				let project_id = create_project(&mut inst);
				previous_projects_created = projects_participated_amount;

				// 0x multiplier should fail
				assert_err!(
					test_contribution_setup(&mut inst, project_id, BUYER_1, InvestorType::Retail, 0),
					Error::<TestRuntime>::ForbiddenMultiplier
				);

				// Multipliers that should work
				for multiplier in 1..=max_allowed_multiplier {
					assert_ok!(test_contribution_setup(
						&mut inst,
						project_id,
						BUYER_1,
						InvestorType::Retail,
						multiplier
					));
				}

				// Multipliers that should NOT work
				for multiplier in max_allowed_multiplier + 1..=50 {
					assert_err!(
						test_contribution_setup(&mut inst, project_id, BUYER_1, InvestorType::Retail, multiplier),
						Error::<TestRuntime>::ForbiddenMultiplier
					);
				}
			}
		}

		#[test]
		fn did_with_losing_bid_can_contribute() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let project_metadata = default_project_metadata(ISSUER_1);
			let mut evaluations = default_evaluations();
			evaluations.push((BIDDER_4, 1337 * USD_UNIT).into());

			let successful_bids = vec![
				BidParams::new(BIDDER_1, 400_000 * CT_UNIT, 1u8, AcceptedFundingAsset::USDT),
				BidParams::new(BIDDER_2, 100_000 * CT_UNIT, 1u8, AcceptedFundingAsset::USDT),
			];
			let failing_bids_after_random_end =
				vec![(BIDDER_3, 25_000 * CT_UNIT).into(), (BIDDER_4, 25_000 * CT_UNIT).into()];
			// This bids should fill the first bucket.
			let failing_bids_sold_out =
				vec![(BIDDER_5, 250_000 * CT_UNIT).into(), (BIDDER_6, 250_000 * CT_UNIT).into()];

			let all_bids = failing_bids_sold_out
				.iter()
				.chain(successful_bids.iter())
				.chain(failing_bids_after_random_end.iter())
				.cloned()
				.collect_vec();

			let project_id = inst.create_auctioning_project(project_metadata.clone(), ISSUER_1, default_evaluations());

			let plmc_fundings = inst.calculate_auction_plmc_charged_from_all_bids_made_or_with_bucket(
				&all_bids.clone(),
				project_metadata.clone(),
				None,
			);
			let plmc_existential_deposits = plmc_fundings.accounts().existential_deposits();
			inst.mint_plmc_to(plmc_fundings.clone());
			inst.mint_plmc_to(plmc_existential_deposits.clone());

			let foreign_funding = inst.calculate_auction_funding_asset_charged_from_all_bids_made_or_with_bucket(
				&all_bids.clone(),
				project_metadata.clone(),
				None,
			);
			inst.mint_foreign_asset_to(foreign_funding.clone());

			inst.bid_for_users(project_id, failing_bids_sold_out).unwrap();
			inst.bid_for_users(project_id, successful_bids).unwrap();
			inst.advance_time(
				<TestRuntime as Config>::AuctionOpeningDuration::get() +
					<TestRuntime as Config>::AuctionClosingDuration::get() -
					1,
			)
			.unwrap();
			inst.bid_for_users(project_id, failing_bids_after_random_end).unwrap();
			inst.start_community_funding(project_id).unwrap();

			// Some low amount of plmc and usdt to cover a purchase of 10CTs.
			let plmc_mints = vec![
				(BIDDER_3, 42069 * PLMC).into(),
				(BIDDER_4, 42069 * PLMC).into(),
				(BIDDER_5, 42069 * PLMC).into(),
				(BIDDER_6, 42069 * PLMC).into(),
				(BUYER_3, 42069 * PLMC).into(),
				(BUYER_4, 42069 * PLMC).into(),
				(BUYER_5, 42069 * PLMC).into(),
				(BUYER_6, 42069 * PLMC).into(),
			];
			inst.mint_plmc_to(plmc_mints);
			let usdt_mints = vec![
				(BIDDER_3, 42069 * CT_UNIT).into(),
				(BIDDER_4, 42069 * CT_UNIT).into(),
				(BIDDER_5, 42069 * CT_UNIT).into(),
				(BIDDER_6, 42069 * CT_UNIT).into(),
				(BUYER_3, 42069 * CT_UNIT).into(),
				(BUYER_4, 42069 * CT_UNIT).into(),
				(BUYER_5, 42069 * CT_UNIT).into(),
				(BUYER_6, 42069 * CT_UNIT).into(),
			];
			inst.mint_foreign_asset_to(usdt_mints);

			let mut bid_should_succeed = |account, investor_type, did_acc| {
				inst.execute(|| {
					assert_ok!(Pallet::<TestRuntime>::community_contribute(
						RuntimeOrigin::signed(account),
						get_mock_jwt_with_cid(
							account,
							investor_type,
							generate_did_from_account(did_acc),
							project_metadata.clone().policy_ipfs_cid.unwrap()
						),
						project_id,
						10 * CT_UNIT,
						1u8.try_into().unwrap(),
						AcceptedFundingAsset::USDT,
					));
				});
			};

			// Bidder 3 has a losing bid due to bidding after the random end. His did should be able to contribute regardless of what investor type
			// or account he uses to sign the transaction
			bid_should_succeed(BIDDER_3, InvestorType::Institutional, BIDDER_3);
			bid_should_succeed(BUYER_3, InvestorType::Institutional, BIDDER_3);
			bid_should_succeed(BIDDER_3, InvestorType::Professional, BIDDER_3);
			bid_should_succeed(BUYER_3, InvestorType::Professional, BIDDER_3);
			bid_should_succeed(BIDDER_3, InvestorType::Retail, BIDDER_3);
			bid_should_succeed(BUYER_3, InvestorType::Retail, BIDDER_3);

			// Bidder 4 has a losing bid due to bidding after the random end, and he was also an evaluator. Same conditions as before should apply.
			bid_should_succeed(BIDDER_4, InvestorType::Institutional, BIDDER_4);
			bid_should_succeed(BUYER_4, InvestorType::Institutional, BIDDER_4);
			bid_should_succeed(BIDDER_4, InvestorType::Professional, BIDDER_4);
			bid_should_succeed(BUYER_4, InvestorType::Professional, BIDDER_4);
			bid_should_succeed(BIDDER_4, InvestorType::Retail, BIDDER_4);
			bid_should_succeed(BUYER_4, InvestorType::Retail, BIDDER_4);

			// Bidder 5 has a losing bid due to CTs being sold out at his price point. Same conditions as before should apply.
			bid_should_succeed(BIDDER_5, InvestorType::Institutional, BIDDER_5);
			bid_should_succeed(BUYER_5, InvestorType::Institutional, BIDDER_5);
			bid_should_succeed(BIDDER_5, InvestorType::Professional, BIDDER_5);
			bid_should_succeed(BUYER_5, InvestorType::Professional, BIDDER_5);
			bid_should_succeed(BIDDER_5, InvestorType::Retail, BIDDER_5);
			bid_should_succeed(BUYER_5, InvestorType::Retail, BIDDER_5);

			// Bidder 6 has a losing bid due to CTs being sold out at his price point, and he was also an evaluator. Same conditions as before should apply.
			bid_should_succeed(BIDDER_6, InvestorType::Institutional, BIDDER_6);
			bid_should_succeed(BUYER_6, InvestorType::Institutional, BIDDER_6);
			bid_should_succeed(BIDDER_6, InvestorType::Professional, BIDDER_6);
			bid_should_succeed(BUYER_6, InvestorType::Professional, BIDDER_6);
			bid_should_succeed(BIDDER_6, InvestorType::Retail, BIDDER_6);
			bid_should_succeed(BUYER_6, InvestorType::Retail, BIDDER_6);
		}
	}

	#[cfg(test)]
	mod failure {
		use super::*;
		use frame_support::traits::{
			fungible::Mutate as MutateFungible,
			fungibles::Mutate as MutateFungibles,
			tokens::{Fortitude, Precision},
		};

		#[test]
		fn contribution_errors_if_user_limit_is_reached() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let project_id = inst.create_community_contributing_project(
				default_project_metadata(ISSUER_1),
				ISSUER_1,
				default_evaluations(),
				default_bids(),
			);
			const CONTRIBUTOR: AccountIdOf<TestRuntime> = 420;

			let project_details = inst.get_project_details(project_id);
			let token_price = project_details.weighted_average_price.unwrap();

			// Create a contribution vector that will reach the limit of contributions for a user-project
			let token_amount: BalanceOf<TestRuntime> = CT_UNIT;
			let range = 0..<TestRuntime as Config>::MaxContributionsPerUser::get();
			let contributions: Vec<ContributionParams<_>> = range
				.map(|_| ContributionParams::new(CONTRIBUTOR, token_amount, 1u8, AcceptedFundingAsset::USDT))
				.collect();

			let plmc_funding = inst.calculate_contributed_plmc_spent(contributions.clone(), token_price);
			let plmc_existential_deposits = plmc_funding.accounts().existential_deposits();

			let foreign_funding = inst.calculate_contributed_funding_asset_spent(contributions.clone(), token_price);

			inst.mint_plmc_to(plmc_funding.clone());
			inst.mint_plmc_to(plmc_existential_deposits.clone());

			inst.mint_foreign_asset_to(foreign_funding.clone());

			// Reach up to the limit of contributions for a user-project
			assert!(inst.contribute_for_users(project_id, contributions).is_ok());

			// Try to contribute again, but it should fail because the limit of contributions for a user-project was reached.
			let over_limit_contribution =
				ContributionParams::new(CONTRIBUTOR, token_amount, 1u8, AcceptedFundingAsset::USDT);
			assert!(inst.contribute_for_users(project_id, vec![over_limit_contribution]).is_err());

			// Check that the right amount of PLMC is bonded, and funding currency is transferred
			let contributor_post_buy_plmc_balance =
				inst.execute(|| <TestRuntime as Config>::NativeCurrency::balance(&CONTRIBUTOR));
			let contributor_post_buy_foreign_asset_balance = inst.execute(|| {
				<TestRuntime as Config>::FundingCurrency::balance(
					AcceptedFundingAsset::USDT.to_assethub_id(),
					CONTRIBUTOR,
				)
			});

			assert_eq!(contributor_post_buy_plmc_balance, inst.get_ed());
			assert_eq!(contributor_post_buy_foreign_asset_balance, 0);

			let plmc_bond_stored = inst.execute(|| {
				<TestRuntime as Config>::NativeCurrency::balance_on_hold(
					&HoldReason::Participation(project_id.into()).into(),
					&CONTRIBUTOR,
				)
			});
			let foreign_asset_contributions_stored = inst.execute(|| {
				Contributions::<TestRuntime>::iter_prefix_values((project_id, CONTRIBUTOR))
					.map(|c| c.funding_asset_amount)
					.sum::<BalanceOf<TestRuntime>>()
			});

			assert_eq!(plmc_bond_stored, inst.sum_balance_mappings(vec![plmc_funding.clone()]));
			assert_eq!(foreign_asset_contributions_stored, inst.sum_foreign_mappings(vec![foreign_funding.clone()]));
		}

		#[test]
		fn issuer_cannot_contribute_his_project() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let project_metadata = default_project_metadata(ISSUER_1);
			let project_id = inst.create_community_contributing_project(
				project_metadata.clone(),
				ISSUER_1,
				default_evaluations(),
				default_bids(),
			);
			assert_err!(
				inst.execute(|| crate::Pallet::<TestRuntime>::do_community_contribute(
					&(&ISSUER_1 + 1),
					project_id,
					500 * CT_UNIT,
					1u8.try_into().unwrap(),
					AcceptedFundingAsset::USDT,
					generate_did_from_account(ISSUER_1),
					InvestorType::Institutional,
					project_metadata.clone().policy_ipfs_cid.unwrap(),
				)),
				Error::<TestRuntime>::ParticipationToOwnProject
			);
		}

		#[test]
		fn did_with_winning_bid_cannot_contribute() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let project_metadata = default_project_metadata(ISSUER_1);
			let mut evaluations = default_evaluations();
			evaluations.push((BIDDER_2, 1337 * USD_UNIT).into());
			let bids = vec![
				BidParams::new(BIDDER_1, 400_000 * CT_UNIT, 1u8, AcceptedFundingAsset::USDT),
				BidParams::new(BIDDER_2, 50_000 * CT_UNIT, 1u8, AcceptedFundingAsset::USDT),
				// Partially accepted bid. Only the 50k of the second bid will be accepted.
				BidParams::new(BIDDER_3, 100_000 * CT_UNIT, 1u8, AcceptedFundingAsset::USDT),
			];

			let project_id = inst.create_community_contributing_project(
				project_metadata.clone(),
				ISSUER_1,
				default_evaluations(),
				bids,
			);

			let mut bid_should_fail = |account, investor_type, did_acc| {
				inst.execute(|| {
					assert_noop!(
						Pallet::<TestRuntime>::community_contribute(
							RuntimeOrigin::signed(account),
							get_mock_jwt_with_cid(
								account,
								investor_type,
								generate_did_from_account(did_acc),
								project_metadata.clone().policy_ipfs_cid.unwrap()
							),
							project_id,
							10 * CT_UNIT,
							1u8.try_into().unwrap(),
							AcceptedFundingAsset::USDT,
						),
						Error::<TestRuntime>::UserHasWinningBid
					);
				});
			};

			// Bidder 1 has a winning bid, his did should not be able to contribute regardless of what investor type
			// or account he uses to sign the transaction
			bid_should_fail(BIDDER_1, InvestorType::Institutional, BIDDER_1);
			bid_should_fail(BUYER_1, InvestorType::Institutional, BIDDER_1);
			bid_should_fail(BIDDER_1, InvestorType::Professional, BIDDER_1);
			bid_should_fail(BUYER_1, InvestorType::Professional, BIDDER_1);
			bid_should_fail(BIDDER_1, InvestorType::Retail, BIDDER_1);
			bid_should_fail(BUYER_1, InvestorType::Retail, BIDDER_1);

			// Bidder 2 has a winning bid, and he was also an evaluator. Same conditions as before should apply.
			bid_should_fail(BIDDER_2, InvestorType::Institutional, BIDDER_2);
			bid_should_fail(BUYER_2, InvestorType::Institutional, BIDDER_2);
			bid_should_fail(BIDDER_2, InvestorType::Professional, BIDDER_2);
			bid_should_fail(BUYER_2, InvestorType::Professional, BIDDER_2);
			bid_should_fail(BIDDER_2, InvestorType::Retail, BIDDER_2);
			bid_should_fail(BUYER_2, InvestorType::Retail, BIDDER_2);

			// Bidder 3 has a partial winning bid. Same conditions as before should apply.
			bid_should_fail(BIDDER_3, InvestorType::Institutional, BIDDER_3);
			bid_should_fail(BUYER_3, InvestorType::Institutional, BIDDER_3);
			bid_should_fail(BIDDER_3, InvestorType::Professional, BIDDER_3);
			bid_should_fail(BUYER_3, InvestorType::Professional, BIDDER_3);
			bid_should_fail(BIDDER_3, InvestorType::Retail, BIDDER_3);
			bid_should_fail(BUYER_3, InvestorType::Retail, BIDDER_3);
		}

		#[test]
		fn per_credential_type_ticket_size_minimums() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let mut project_metadata = default_project_metadata(ISSUER_1);
			project_metadata.contributing_ticket_sizes = ContributingTicketSizes {
				retail: TicketSize::new(10 * USD_UNIT, None),
				professional: TicketSize::new(100_000 * USD_UNIT, None),
				institutional: TicketSize::new(200_000 * USD_UNIT, None),
				phantom: Default::default(),
			};

			let project_id = inst.create_community_contributing_project(
				project_metadata.clone(),
				ISSUER_1,
				default_evaluations(),
				default_bids(),
			);

			inst.mint_plmc_to(vec![
				(BUYER_1, 50_000 * CT_UNIT).into(),
				(BUYER_2, 50_000 * CT_UNIT).into(),
				(BUYER_3, 50_000 * CT_UNIT).into(),
			]);

			inst.mint_foreign_asset_to(vec![
				(BUYER_1, 50_000 * USD_UNIT).into(),
				(BUYER_2, 50_000 * USD_UNIT).into(),
				(BUYER_3, 50_000 * USD_UNIT).into(),
			]);

			// contribution below 1 CT (10 USD) should fail for retail
			let jwt = get_mock_jwt_with_cid(
				BUYER_1,
				InvestorType::Retail,
				generate_did_from_account(BUYER_1),
				project_metadata.clone().policy_ipfs_cid.unwrap(),
			);
			inst.execute(|| {
				assert_noop!(
					Pallet::<TestRuntime>::community_contribute(
						RuntimeOrigin::signed(BUYER_1),
						jwt,
						project_id,
						CT_UNIT / 2,
						1u8.try_into().unwrap(),
						AcceptedFundingAsset::USDT,
					),
					Error::<TestRuntime>::TooLow
				);
			});
			// contribution below 10_000 CT (100k USD) should fail for professionals
			let jwt = get_mock_jwt_with_cid(
				BUYER_2,
				InvestorType::Professional,
				generate_did_from_account(BUYER_2),
				project_metadata.clone().policy_ipfs_cid.unwrap(),
			);
			inst.execute(|| {
				assert_noop!(
					Pallet::<TestRuntime>::community_contribute(
						RuntimeOrigin::signed(BUYER_2),
						jwt,
						project_id,
						9_999,
						1u8.try_into().unwrap(),
						AcceptedFundingAsset::USDT,
					),
					Error::<TestRuntime>::TooLow
				);
			});

			// contribution below 20_000 CT (200k USD) should fail for institutionals
			let jwt = get_mock_jwt_with_cid(
				BUYER_3,
				InvestorType::Professional,
				generate_did_from_account(BUYER_3),
				project_metadata.clone().policy_ipfs_cid.unwrap(),
			);
			inst.execute(|| {
				assert_noop!(
					Pallet::<TestRuntime>::community_contribute(
						RuntimeOrigin::signed(BUYER_3),
						jwt,
						project_id,
						19_999,
						1u8.try_into().unwrap(),
						AcceptedFundingAsset::USDT,
					),
					Error::<TestRuntime>::TooLow
				);
			});
		}

		#[test]
		fn per_credential_type_ticket_size_maximums() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let mut project_metadata = default_project_metadata(ISSUER_1);
			project_metadata.contributing_ticket_sizes = ContributingTicketSizes {
				retail: TicketSize::new(USD_UNIT, Some(100_000 * USD_UNIT)),
				professional: TicketSize::new(USD_UNIT, Some(20_000 * USD_UNIT)),
				institutional: TicketSize::new(USD_UNIT, Some(50_000 * USD_UNIT)),
				phantom: Default::default(),
			};

			let project_id = inst.create_community_contributing_project(
				project_metadata.clone(),
				ISSUER_1,
				default_evaluations(),
				default_bids(),
			);

			inst.mint_plmc_to(vec![
				(BUYER_1, 500_000 * CT_UNIT).into(),
				(BUYER_2, 500_000 * CT_UNIT).into(),
				(BUYER_3, 500_000 * CT_UNIT).into(),
				(BUYER_4, 500_000 * CT_UNIT).into(),
				(BUYER_5, 500_000 * CT_UNIT).into(),
				(BUYER_6, 500_000 * CT_UNIT).into(),
			]);

			inst.mint_foreign_asset_to(vec![
				(BUYER_1, 500_000 * USD_UNIT).into(),
				(BUYER_2, 500_000 * USD_UNIT).into(),
				(BUYER_3, 500_000 * USD_UNIT).into(),
				(BUYER_4, 500_000 * USD_UNIT).into(),
				(BUYER_5, 500_000 * USD_UNIT).into(),
				(BUYER_6, 500_000 * USD_UNIT).into(),
			]);

			let buyer_1_jwt = get_mock_jwt_with_cid(
				BUYER_1,
				InvestorType::Retail,
				generate_did_from_account(BUYER_1),
				project_metadata.clone().policy_ipfs_cid.unwrap(),
			);
			let buyer_2_jwt_same_did = get_mock_jwt_with_cid(
				BUYER_2,
				InvestorType::Retail,
				generate_did_from_account(BUYER_1),
				project_metadata.clone().policy_ipfs_cid.unwrap(),
			);
			// total contributions with same DID above 10k CT (100k USD) should fail for retail
			inst.execute(|| {
				assert_ok!(Pallet::<TestRuntime>::community_contribute(
					RuntimeOrigin::signed(BUYER_1),
					buyer_1_jwt,
					project_id,
					9000 * CT_UNIT,
					1u8.try_into().unwrap(),
					AcceptedFundingAsset::USDT,
				));
			});
			inst.execute(|| {
				assert_noop!(
					Pallet::<TestRuntime>::community_contribute(
						RuntimeOrigin::signed(BUYER_2),
						buyer_2_jwt_same_did.clone(),
						project_id,
						1001 * CT_UNIT,
						1u8.try_into().unwrap(),
						AcceptedFundingAsset::USDT,
					),
					Error::<TestRuntime>::TooHigh
				);
			});
			// bidding 2k total works
			inst.execute(|| {
				assert_ok!(Pallet::<TestRuntime>::community_contribute(
					RuntimeOrigin::signed(BUYER_2),
					buyer_2_jwt_same_did,
					project_id,
					1000 * CT_UNIT,
					1u8.try_into().unwrap(),
					AcceptedFundingAsset::USDT,
				));
			});

			let buyer_3_jwt = get_mock_jwt_with_cid(
				BUYER_3,
				InvestorType::Professional,
				generate_did_from_account(BUYER_3),
				project_metadata.clone().policy_ipfs_cid.unwrap(),
			);
			let buyer_4_jwt_same_did = get_mock_jwt_with_cid(
				BUYER_4,
				InvestorType::Professional,
				generate_did_from_account(BUYER_3),
				project_metadata.clone().policy_ipfs_cid.unwrap(),
			);
			// total contributions with same DID above 2k CT (20k USD) should fail for professionals
			inst.execute(|| {
				assert_ok!(Pallet::<TestRuntime>::community_contribute(
					RuntimeOrigin::signed(BUYER_3),
					buyer_3_jwt,
					project_id,
					1800 * CT_UNIT,
					1u8.try_into().unwrap(),
					AcceptedFundingAsset::USDT,
				));
			});
			inst.execute(|| {
				assert_noop!(
					Pallet::<TestRuntime>::community_contribute(
						RuntimeOrigin::signed(BUYER_4),
						buyer_4_jwt_same_did.clone(),
						project_id,
						201 * CT_UNIT,
						1u8.try_into().unwrap(),
						AcceptedFundingAsset::USDT,
					),
					Error::<TestRuntime>::TooHigh
				);
			});
			// bidding 2k total works
			inst.execute(|| {
				assert_ok!(Pallet::<TestRuntime>::community_contribute(
					RuntimeOrigin::signed(BUYER_4),
					buyer_4_jwt_same_did,
					project_id,
					200 * CT_UNIT,
					1u8.try_into().unwrap(),
					AcceptedFundingAsset::USDT,
				));
			});

			let buyer_5_jwt = get_mock_jwt_with_cid(
				BUYER_5,
				InvestorType::Institutional,
				generate_did_from_account(BUYER_5),
				project_metadata.clone().policy_ipfs_cid.unwrap(),
			);
			let buyer_6_jwt_same_did = get_mock_jwt_with_cid(
				BUYER_6,
				InvestorType::Institutional,
				generate_did_from_account(BUYER_5),
				project_metadata.clone().policy_ipfs_cid.unwrap(),
			);
			// total contributions with same DID above 5k CT (50 USD) should fail for institutionals
			inst.execute(|| {
				assert_ok!(Pallet::<TestRuntime>::community_contribute(
					RuntimeOrigin::signed(BUYER_5),
					buyer_5_jwt,
					project_id,
					4690 * CT_UNIT,
					1u8.try_into().unwrap(),
					AcceptedFundingAsset::USDT,
				));
			});
			inst.execute(|| {
				assert_noop!(
					Pallet::<TestRuntime>::community_contribute(
						RuntimeOrigin::signed(BUYER_6),
						buyer_6_jwt_same_did.clone(),
						project_id,
						311 * CT_UNIT,
						1u8.try_into().unwrap(),
						AcceptedFundingAsset::USDT,
					),
					Error::<TestRuntime>::TooHigh
				);
			});
			// bidding 5k total works
			inst.execute(|| {
				assert_ok!(Pallet::<TestRuntime>::community_contribute(
					RuntimeOrigin::signed(BUYER_6),
					buyer_6_jwt_same_did,
					project_id,
					310 * CT_UNIT,
					1u8.try_into().unwrap(),
					AcceptedFundingAsset::USDT,
				));
			});
		}

		#[test]
		fn insufficient_funds() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let project_metadata = default_project_metadata(ISSUER_1);
			let project_id = inst.create_community_contributing_project(
				project_metadata.clone(),
				ISSUER_1,
				default_evaluations(),
				default_bids(),
			);

			let jwt = get_mock_jwt_with_cid(
				BUYER_1,
				InvestorType::Retail,
				generate_did_from_account(BUYER_1),
				project_metadata.clone().policy_ipfs_cid.unwrap(),
			);
			let contribution = ContributionParams::new(BUYER_1, 1_000 * CT_UNIT, 1u8, AcceptedFundingAsset::USDT);
			let wap = inst.get_project_details(project_id).weighted_average_price.unwrap();

			// 1 unit less native asset than needed
			let plmc_funding = inst.calculate_contributed_plmc_spent(vec![contribution.clone()], wap);
			let plmc_existential_deposits = plmc_funding.accounts().existential_deposits();
			inst.mint_plmc_to(plmc_funding.clone());
			inst.mint_plmc_to(plmc_existential_deposits.clone());
			inst.execute(|| Balances::burn_from(&BUYER_1, 1, Precision::BestEffort, Fortitude::Force)).unwrap();

			let foreign_funding = inst.calculate_contributed_funding_asset_spent(vec![contribution.clone()], wap);
			inst.mint_foreign_asset_to(foreign_funding.clone());
			inst.execute(|| {
				assert_noop!(
					Pallet::<TestRuntime>::community_contribute(
						RuntimeOrigin::signed(BUYER_1),
						jwt.clone(),
						project_id,
						1_000 * CT_UNIT,
						1u8.try_into().unwrap(),
						AcceptedFundingAsset::USDT,
					),
					Error::<TestRuntime>::ParticipantNotEnoughFunds
				);
			});

			// 1 unit less funding asset than needed
			let plmc_funding = inst.calculate_contributed_plmc_spent(vec![contribution.clone()], wap);
			let plmc_existential_deposits = plmc_funding.accounts().existential_deposits();
			inst.mint_plmc_to(plmc_funding.clone());
			inst.mint_plmc_to(plmc_existential_deposits.clone());
			let foreign_funding = inst.calculate_contributed_funding_asset_spent(vec![contribution.clone()], wap);

			inst.execute(|| ForeignAssets::set_balance(AcceptedFundingAsset::USDT.to_assethub_id(), &BUYER_1, 0));
			inst.mint_foreign_asset_to(foreign_funding.clone());

			inst.execute(|| {
				ForeignAssets::burn_from(
					AcceptedFundingAsset::USDT.to_assethub_id(),
					&BUYER_1,
					100,
					Precision::BestEffort,
					Fortitude::Force,
				)
			})
			.unwrap();

			inst.execute(|| {
				assert_noop!(
					Pallet::<TestRuntime>::community_contribute(
						RuntimeOrigin::signed(BUYER_1),
						jwt,
						project_id,
						1_000 * CT_UNIT,
						1u8.try_into().unwrap(),
						AcceptedFundingAsset::USDT,
					),
					Error::<TestRuntime>::ParticipantNotEnoughFunds
				);
			});
		}

		#[test]
		fn called_outside_community_round() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let created_project = inst.create_new_project(default_project_metadata(ISSUER_1), ISSUER_1);
			let evaluating_project = inst.create_evaluating_project(default_project_metadata(ISSUER_2), ISSUER_2);
			let auctioning_project =
				inst.create_auctioning_project(default_project_metadata(ISSUER_3), ISSUER_3, default_evaluations());
			let remaining_project = inst.create_remainder_contributing_project(
				default_project_metadata(ISSUER_4),
				ISSUER_4,
				default_evaluations(),
				default_bids(),
				default_community_buys(),
			);
			let finished_project = inst.create_finished_project(
				default_project_metadata(ISSUER_5),
				ISSUER_5,
				default_evaluations(),
				default_bids(),
				default_community_buys(),
				default_remainder_buys(),
			);

			let projects =
				vec![created_project, evaluating_project, auctioning_project, remaining_project, finished_project];
			for project in projects {
				let project_policy = inst.get_project_metadata(project).policy_ipfs_cid.unwrap();
				inst.execute(|| {
					assert_noop!(
						PolimecFunding::community_contribute(
							RuntimeOrigin::signed(BUYER_1),
							get_mock_jwt_with_cid(
								BUYER_1,
								InvestorType::Retail,
								generate_did_from_account(BUYER_1),
								project_policy
							),
							project,
							1000 * CT_UNIT,
							1u8.try_into().unwrap(),
							AcceptedFundingAsset::USDT
						),
						Error::<TestRuntime>::IncorrectRound
					);
				});
			}
		}

		#[test]
		fn contribute_with_unaccepted_currencies() {
			let inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));

			let mut project_metadata_usdt = default_project_metadata(ISSUER_2);
			project_metadata_usdt.participation_currencies = vec![AcceptedFundingAsset::USDT].try_into().unwrap();

			let mut project_metadata_usdc = default_project_metadata(ISSUER_3);
			project_metadata_usdc.participation_currencies = vec![AcceptedFundingAsset::USDC].try_into().unwrap();

			let evaluations = default_evaluations();

			let usdt_bids = default_bids()
				.into_iter()
				.map(|mut b| {
					b.asset = AcceptedFundingAsset::USDT;
					b
				})
				.collect::<Vec<_>>();

			let usdc_bids = default_bids()
				.into_iter()
				.map(|mut b| {
					b.asset = AcceptedFundingAsset::USDC;
					b
				})
				.collect::<Vec<_>>();

			let projects = vec![
				TestProjectParams {
					expected_state: ProjectStatus::CommunityRound,
					metadata: project_metadata_usdt,
					issuer: ISSUER_2,
					evaluations: evaluations.clone(),
					bids: usdt_bids.clone(),
					community_contributions: vec![],
					remainder_contributions: vec![],
				},
				TestProjectParams {
					expected_state: ProjectStatus::CommunityRound,
					metadata: project_metadata_usdc,
					issuer: ISSUER_3,
					evaluations: evaluations.clone(),
					bids: usdc_bids.clone(),
					community_contributions: vec![],
					remainder_contributions: vec![],
				},
			];
			let (project_ids, mut inst) = create_multiple_projects_at(inst, projects);

			let project_id_usdt = project_ids[0];
			let project_id_usdc = project_ids[1];

			let usdt_contribution = ContributionParams::new(BUYER_1, 10_000 * CT_UNIT, 1u8, AcceptedFundingAsset::USDT);
			let usdc_contribution = ContributionParams::new(BUYER_2, 10_000 * CT_UNIT, 1u8, AcceptedFundingAsset::USDC);
			let dot_contribution = ContributionParams::new(BUYER_3, 10_000 * CT_UNIT, 1u8, AcceptedFundingAsset::DOT);

			assert_err!(
				inst.contribute_for_users(project_id_usdc, vec![usdt_contribution.clone()]),
				Error::<TestRuntime>::FundingAssetNotAccepted
			);
			assert_err!(
				inst.contribute_for_users(project_id_usdt, vec![usdc_contribution.clone()]),
				Error::<TestRuntime>::FundingAssetNotAccepted
			);
			assert_err!(
				inst.contribute_for_users(project_id_usdt, vec![dot_contribution.clone()]),
				Error::<TestRuntime>::FundingAssetNotAccepted
			);
		}

		#[test]
		fn cannot_use_evaluation_bond_on_another_project_contribution() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let project_metadata_1 = default_project_metadata(ISSUER_1);
			let project_metadata_2 = default_project_metadata(ISSUER_2);

			let mut evaluations_1 = default_evaluations();
			let evaluations_2 = default_evaluations();

			let evaluator_contributor = 69;
			let evaluation_amount = 420 * USD_UNIT;
			let evaluator_contribution =
				ContributionParams::new(evaluator_contributor, 600 * CT_UNIT, 1u8, AcceptedFundingAsset::USDT);
			evaluations_1.push((evaluator_contributor, evaluation_amount).into());

			let _project_id_1 = inst.create_community_contributing_project(
				project_metadata_1.clone(),
				ISSUER_1,
				evaluations_1,
				default_bids(),
			);
			let project_id_2 = inst.create_community_contributing_project(
				project_metadata_2.clone(),
				ISSUER_2,
				evaluations_2,
				default_bids(),
			);

			let wap = inst.get_project_details(project_id_2).weighted_average_price.unwrap();

			// Necessary Mints
			let already_bonded_plmc =
				inst.calculate_evaluation_plmc_spent(vec![(evaluator_contributor, evaluation_amount).into()])[0]
					.plmc_amount;
			let usable_evaluation_plmc =
				already_bonded_plmc - <TestRuntime as Config>::EvaluatorSlash::get() * already_bonded_plmc;
			let necessary_plmc_for_contribution =
				inst.calculate_contributed_plmc_spent(vec![evaluator_contribution.clone()], wap)[0].plmc_amount;
			let necessary_usdt_for_contribution =
				inst.calculate_contributed_funding_asset_spent(vec![evaluator_contribution.clone()], wap);
			inst.mint_plmc_to(vec![UserToPLMCBalance::new(
				evaluator_contributor,
				necessary_plmc_for_contribution - usable_evaluation_plmc,
			)]);
			inst.mint_foreign_asset_to(necessary_usdt_for_contribution);

			inst.execute(|| {
				assert_noop!(
					PolimecFunding::community_contribute(
						RuntimeOrigin::signed(evaluator_contributor),
						get_mock_jwt_with_cid(
							evaluator_contributor,
							InvestorType::Retail,
							generate_did_from_account(evaluator_contributor),
							project_metadata_2.clone().policy_ipfs_cid.unwrap()
						),
						project_id_2,
						evaluator_contribution.amount,
						evaluator_contribution.multiplier,
						evaluator_contribution.asset
					),
					Error::<TestRuntime>::ParticipantNotEnoughFunds
				);
			});
		}

		#[test]
		fn wrong_policy_on_jwt() {
			let mut inst = MockInstantiator::new(Some(RefCell::new(new_test_ext())));
			let project_metadata = default_project_metadata(ISSUER_1);
			let project_id = inst.create_community_contributing_project(
				project_metadata.clone(),
				ISSUER_1,
				default_evaluations(),
				default_bids(),
			);

			inst.execute(|| {
				assert_noop!(
					PolimecFunding::community_contribute(
						RuntimeOrigin::signed(BUYER_1),
						get_mock_jwt_with_cid(
							BUYER_1,
							InvestorType::Retail,
							generate_did_from_account(BUYER_1),
							"wrong_cid".as_bytes().to_vec().try_into().unwrap()
						),
						project_id,
						5000 * CT_UNIT,
						1u8.try_into().unwrap(),
						AcceptedFundingAsset::USDT
					),
					Error::<TestRuntime>::PolicyMismatch
				);
			});
		}
	}
}
