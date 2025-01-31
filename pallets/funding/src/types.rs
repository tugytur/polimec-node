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

// If you feel like getting in touch with us, you can do so at info@polimec.org

//! Types for Funding pallet.

use crate::{traits::BondingRequirementCalculation, BalanceOf};
pub use config_types::*;
use frame_support::{pallet_prelude::*, traits::tokens::Balance as BalanceT};
use frame_system::pallet_prelude::BlockNumberFor;
pub use inner_types::*;
use polimec_common::USD_DECIMALS;
use polkadot_parachain_primitives::primitives::Id as ParaId;
use serde::{Deserialize, Serialize};
use sp_arithmetic::{FixedPointNumber, FixedPointOperand, Percent};
use sp_runtime::traits::{CheckedDiv, CheckedMul, One, Saturating, Zero};
use sp_std::{cmp::Eq, prelude::*};
pub use storage_types::*;

pub mod config_types {
	use parachains_common::DAYS;
	use sp_arithmetic::{traits::Saturating, FixedU128};
	use sp_runtime::traits::{Convert, One};

	use crate::{traits::VestingDurationCalculation, Config};

	use super::*;

	#[derive(
		Clone,
		Encode,
		Decode,
		Eq,
		PartialEq,
		RuntimeDebug,
		TypeInfo,
		MaxEncodedLen,
		Copy,
		Ord,
		PartialOrd,
		Serialize,
		Deserialize,
	)]
	pub struct Multiplier(u8);

	impl Multiplier {
		/// Creates a new `Multiplier` if the value is between 1 and 25, otherwise returns an error.
		pub const fn new(x: u8) -> Result<Self, ()> {
			// The minimum and maximum values are chosen to be 1 and 25 respectively, as defined in the Knowledge Hub.
			const MIN_VALID: u8 = 1;
			const MAX_VALID: u8 = 25;

			if x >= MIN_VALID && x <= MAX_VALID {
				Ok(Self(x))
			} else {
				Err(())
			}
		}

		pub const fn force_new(x: u8) -> Self {
			Self(x)
		}
	}

	impl BondingRequirementCalculation for Multiplier {
		fn calculate_bonding_requirement<T: Config>(&self, ticket_size: BalanceOf<T>) -> Result<BalanceOf<T>, ()> {
			let balance_multiplier = BalanceOf::<T>::from(self.0);
			ticket_size.checked_div(&balance_multiplier).ok_or(())
		}
	}

	impl VestingDurationCalculation for Multiplier {
		fn calculate_vesting_duration<T: Config>(&self) -> BlockNumberFor<T> {
			// gradient "m" of the linear curve function y = m*x + b where x is the multiplier and y is the number of weeks
			const GRADIENT: FixedU128 = FixedU128::from_rational(2167u128, 1000u128);
			// negative constant (because we cannot have negative values, so we take the negative and do "-b" instead of "+b") "b" of the linear curve function y = m*x + b
			const NEG_CONSTANT: FixedU128 = FixedU128::from_rational(2167u128, 1000u128);

			let multiplier_as_fixed = FixedU128::saturating_from_integer(self.0);
			let weeks = GRADIENT.saturating_mul(multiplier_as_fixed).saturating_sub(NEG_CONSTANT);

			T::DaysToBlocks::convert(weeks * FixedU128::from_u32(7u32)).max(One::one())
		}
	}

	impl Default for Multiplier {
		fn default() -> Self {
			Self(1u8)
		}
	}

	impl TryFrom<u8> for Multiplier {
		type Error = ();

		fn try_from(x: u8) -> Result<Self, ()> {
			Self::new(x)
		}
	}

	impl Into<u8> for Multiplier {
		fn into(self) -> u8 {
			self.0
		}
	}

	/// Enum used to identify PLMC holds.
	/// It implements Serialize and Deserialize to hold a fungible in the Genesis Configuration.
	#[derive(
		Encode,
		Decode,
		Copy,
		Clone,
		PartialEq,
		Eq,
		RuntimeDebug,
		MaxEncodedLen,
		TypeInfo,
		Ord,
		PartialOrd,
		Serialize,
		Deserialize,
	)]

	pub struct DaysToBlocks;
	impl Convert<FixedU128, u64> for DaysToBlocks {
		fn convert(a: FixedU128) -> u64 {
			let one_day_in_blocks = DAYS;
			a.saturating_mul_int(one_day_in_blocks as u64)
		}
	}
	impl Convert<FixedU128, u32> for DaysToBlocks {
		fn convert(a: FixedU128) -> u32 {
			let one_day_in_blocks = DAYS;
			a.saturating_mul_int(one_day_in_blocks)
		}
	}

	pub type MaxParticipationsForMaxMultiplier = ConstU32<25>;
	pub const fn retail_max_multiplier_for_participations(participations: u8) -> u8 {
		match participations {
			0..=2 => 1,
			3..=4 => 2,
			5..=9 => 4,
			10..=24 => 7,
			25..=u8::MAX => 10,
		}
	}
	pub const PROFESSIONAL_MAX_MULTIPLIER: u8 = 10u8;
	pub const INSTITUTIONAL_MAX_MULTIPLIER: u8 = 25u8;
}

pub mod storage_types {
	use super::*;

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
	pub struct ProjectMetadata<BoundedString, Balance: PartialOrd + Copy, Price: FixedPointNumber, AccountId, Cid> {
		/// Token Metadata
		pub token_information: CurrencyMetadata<BoundedString>,
		/// Mainnet Token Max Supply
		pub mainnet_token_max_supply: Balance,
		/// Total allocation of Contribution Tokens available for the Funding Round.
		pub total_allocation_size: Balance,
		/// Percentage of the total allocation of Contribution Tokens available for the Auction Round
		pub auction_round_allocation_percentage: Percent,
		/// The minimum price per token in USD, decimal-aware. See [`calculate_decimals_aware_price()`](crate::traits::ProvideAssetPrice::calculate_decimals_aware_price) for more information.
		pub minimum_price: Price,
		/// Maximum and minimum ticket sizes for auction round
		pub bidding_ticket_sizes: BiddingTicketSizes<Price, Balance>,
		/// Maximum and minimum ticket sizes for community/remainder rounds
		pub contributing_ticket_sizes: ContributingTicketSizes<Price, Balance>,
		/// Participation currencies (e.g stablecoin, DOT, KSM)
		/// e.g. https://github.com/paritytech/substrate/blob/427fd09bcb193c1e79dec85b1e207c718b686c35/frame/uniques/src/types.rs#L110
		/// For now is easier to handle the case where only just one Currency is accepted
		pub participation_currencies:
			BoundedVec<AcceptedFundingAsset, ConstU32<{ AcceptedFundingAsset::VARIANT_COUNT as u32 }>>,
		pub funding_destination_account: AccountId,
		/// Additional metadata
		pub policy_ipfs_cid: Option<Cid>,
	}

	impl<
			BoundedString,
			Balance: From<u64> + PartialOrd + Copy + FixedPointOperand + One + CheckedMul,
			Price: FixedPointNumber,
			AccountId,
			Cid,
		> ProjectMetadata<BoundedString, Balance, Price, AccountId, Cid>
	{
		/// Validate issuer metadata for the following checks:
		/// - Minimum price is not zero
		/// - Minimum bidding ticket sizes are higher than 5k USD
		/// - Specified participation currencies are unique
		pub fn is_valid(&self) -> Result<(), MetadataError> {
			if self.minimum_price == Price::zero() {
				return Err(MetadataError::PriceTooLow);
			}
			let usd_unit = sp_arithmetic::traits::checked_pow(Balance::from(10u64), USD_DECIMALS as usize)
				.ok_or(MetadataError::BadTokenomics)?;

			let min_bidder_bound_usd: Balance =
				usd_unit.checked_mul(&5000u64.into()).ok_or(MetadataError::BadTokenomics)?;
			self.bidding_ticket_sizes.is_valid(vec![
				InvestorTypeUSDBounds::Professional((min_bidder_bound_usd, None).into()),
				InvestorTypeUSDBounds::Institutional((min_bidder_bound_usd, None).into()),
			])?;

			let min_contributor_bound_usd: Balance =
				usd_unit.checked_mul(&1u64.into()).ok_or(MetadataError::BadTokenomics)?;
			self.contributing_ticket_sizes.is_valid(vec![
				InvestorTypeUSDBounds::Institutional((min_contributor_bound_usd, None).into()),
				InvestorTypeUSDBounds::Professional((min_contributor_bound_usd, None).into()),
				InvestorTypeUSDBounds::Retail((min_contributor_bound_usd, None).into()),
			])?;

			if self.total_allocation_size == 0u64.into() ||
				self.total_allocation_size > self.mainnet_token_max_supply ||
				self.total_allocation_size <
					Balance::from(10u64).saturating_pow(self.token_information.decimals as usize)
			{
				return Err(MetadataError::AllocationSizeError);
			}

			if self.auction_round_allocation_percentage <= Percent::from_percent(0) {
				return Err(MetadataError::AuctionRoundPercentageError);
			}

			let mut deduped = self.participation_currencies.clone().to_vec();
			deduped.sort();
			deduped.dedup();
			if deduped.len() != self.participation_currencies.len() {
				return Err(MetadataError::ParticipationCurrenciesError);
			}

			let target_funding = self.minimum_price.saturating_mul_int(self.total_allocation_size);
			if target_funding < (1000u64 * 10u64.saturating_pow(USD_DECIMALS.into())).into() {
				return Err(MetadataError::FundingTargetTooLow);
			}
			if target_funding > (1_000_000_000u64 * 10u64.saturating_pow(USD_DECIMALS.into())).into() {
				return Err(MetadataError::FundingTargetTooHigh);
			}

			if self.token_information.decimals < 6 || self.token_information.decimals > 18 {
				return Err(MetadataError::BadDecimals);
			}

			let abs_diff: u32 = self.token_information.decimals.abs_diff(USD_DECIMALS).into();
			let abs_diff_unit = 10u128.checked_pow(abs_diff).ok_or(MetadataError::BadDecimals)?;
			let abs_diff_fixed = Price::checked_from_rational(abs_diff_unit, 1).ok_or(MetadataError::BadDecimals)?;
			let original_price = if USD_DECIMALS > self.token_information.decimals {
				self.minimum_price.checked_div(&abs_diff_fixed)
			} else {
				self.minimum_price.checked_mul(&abs_diff_fixed)
			}
			.ok_or(MetadataError::BadDecimals)?;

			let min_price = Price::checked_from_rational(1, 100_000).ok_or(MetadataError::BadTokenomics)?;
			let max_price = Price::checked_from_rational(1000, 1).ok_or(MetadataError::BadTokenomics)?;
			if original_price < min_price || original_price > max_price {
				return Err(MetadataError::BadTokenomics);
			}

			Ok(())
		}
	}

	pub struct Bound<Balance> {
		pub lower: Balance,
		pub upper: Option<Balance>,
	}

	impl<Balance> From<(Balance, Option<Balance>)> for Bound<Balance> {
		fn from(value: (Balance, Option<Balance>)) -> Self {
			Self { lower: value.0, upper: value.1 }
		}
	}

	pub enum InvestorTypeUSDBounds<Balance> {
		Retail(Bound<Balance>),
		Professional(Bound<Balance>),
		Institutional(Bound<Balance>),
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct ProjectDetails<
		AccountId,
		Did,
		BlockNumber,
		Price: FixedPointNumber,
		Balance: BalanceT,
		EvaluationRoundInfo,
	> {
		pub issuer_account: AccountId,
		pub issuer_did: Did,
		/// Whether the project is frozen, so no `metadata` changes are allowed.
		pub is_frozen: bool,
		/// The price in USD per token decided after the Auction Round
		pub weighted_average_price: Option<Price>,
		/// The current status of the project
		pub status: ProjectStatus,
		/// When the different project phases start and end
		pub phase_transition_points: PhaseTransitionPoints<BlockNumber>,
		/// Fundraising target amount in USD (6 decimals)
		pub fundraising_target_usd: Balance,
		/// The amount of Contribution Tokens that have not yet been sold
		pub remaining_contribution_tokens: Balance,
		/// Funding reached amount in USD (6 decimals)
		pub funding_amount_reached_usd: Balance,
		/// Information about the total amount bonded, and the outcome in regards to reward/slash/nothing
		pub evaluation_round_info: EvaluationRoundInfo,
		/// If the auction was oversubscribed, how much USD was raised across all winning bids
		pub usd_bid_on_oversubscription: Option<Balance>,
		/// When the Funding Round ends
		pub funding_end_block: Option<BlockNumber>,
		/// ParaId of project
		pub parachain_id: Option<ParaId>,
		/// Migration readiness check
		pub migration_readiness_check: Option<MigrationReadinessCheck>,
		/// HRMP Channel status
		pub hrmp_channel_status: HRMPChannelStatus,
	}
	/// Tells on_initialize what to do with the project
	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum UpdateType {
		EvaluationEnd,
		AuctionOpeningStart,
		AuctionClosingStart,
		AuctionClosingEnd,
		CommunityFundingStart,
		RemainderFundingStart,
		FundingEnd,
		ProjectDecision(FundingOutcomeDecision),
		StartSettlement,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Ord, PartialOrd)]
	pub struct EvaluationInfo<Id, Did, ProjectId, AccountId, Balance, BlockNumber> {
		pub id: Id,
		pub did: Did,
		pub project_id: ProjectId,
		pub evaluator: AccountId,
		pub original_plmc_bond: Balance,
		// An evaluation bond can be converted to participation bond
		pub current_plmc_bond: Balance,
		pub early_usd_amount: Balance,
		pub late_usd_amount: Balance,
		pub when: BlockNumber,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct BidInfo<ProjectId, Did, Balance: BalanceT, Price: FixedPointNumber, AccountId, BlockNumber, Multiplier> {
		pub id: u32,
		pub project_id: ProjectId,
		pub bidder: AccountId,
		pub did: Did,
		pub status: BidStatus<Balance>,
		#[codec(compact)]
		pub original_ct_amount: Balance,
		pub original_ct_usd_price: Price,
		pub final_ct_amount: Balance,
		pub final_ct_usd_price: Price,
		pub funding_asset: AcceptedFundingAsset,
		pub funding_asset_amount_locked: Balance,
		pub multiplier: Multiplier,
		pub plmc_bond: Balance,
		pub when: BlockNumber,
	}

	impl<
			ProjectId: Eq,
			Did: Eq,
			Balance: BalanceT + FixedPointOperand + Ord,
			Price: FixedPointNumber,
			AccountId: Eq,
			BlockNumber: Eq + Ord,
			Multiplier: Eq,
		> Ord for BidInfo<ProjectId, Did, Balance, Price, AccountId, BlockNumber, Multiplier>
	{
		fn cmp(&self, other: &Self) -> sp_std::cmp::Ordering {
			match self.original_ct_usd_price.cmp(&other.original_ct_usd_price) {
				sp_std::cmp::Ordering::Equal => Ord::cmp(&other.id, &self.id),
				other => other,
			}
		}
	}

	impl<
			ProjectId: Eq,
			Did: Eq,
			Balance: BalanceT + FixedPointOperand,
			Price: FixedPointNumber,
			AccountId: Eq,
			BlockNumber: Eq + Ord,
			Multiplier: Eq,
		> PartialOrd for BidInfo<ProjectId, Did, Balance, Price, AccountId, BlockNumber, Multiplier>
	{
		fn partial_cmp(&self, other: &Self) -> Option<sp_std::cmp::Ordering> {
			Some(self.cmp(other))
		}
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct ContributionInfo<Id, Did, ProjectId, AccountId, Balance, Multiplier> {
		pub id: Id,
		pub did: Did,
		pub project_id: ProjectId,
		pub contributor: AccountId,
		pub ct_amount: Balance,
		pub usd_contribution_amount: Balance,
		pub multiplier: Multiplier,
		pub funding_asset: AcceptedFundingAsset,
		pub funding_asset_amount: Balance,
		pub plmc_bond: Balance,
	}

	/// Represents a bucket that holds a specific amount of tokens at a given price.
	/// Each bucket has a unique ID, an amount of tokens left, a current price, an initial price,
	/// and constants to define price and amount increments for the next buckets.
	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct Bucket<Balance, Price> {
		/// The amount of tokens left in this bucket.
		pub amount_left: Balance,
		/// The current price of tokens in this bucket.
		pub current_price: Price,
		/// The initial price of tokens in the bucket.
		pub initial_price: Price,
		/// Defines the price increment for each subsequent bucket.
		pub delta_price: Price,
		/// Defines the amount increment for each subsequent bucket.
		pub delta_amount: Balance,
	}

	impl<Balance: Copy + Saturating + One + Zero, Price: FixedPointNumber> Bucket<Balance, Price> {
		/// Creates a new bucket with the given parameters.
		pub const fn new(
			amount_left: Balance,
			initial_price: Price,
			delta_price: Price,
			delta_amount: Balance,
		) -> Self {
			Self { amount_left, current_price: initial_price, initial_price, delta_price, delta_amount }
		}

		/// Update the bucket
		pub fn update(&mut self, removed_amount: Balance) {
			self.amount_left.saturating_reduce(removed_amount);
			if self.amount_left.is_zero() {
				self.next();
			}
		}

		/// Updates the bucket to represent the next one in the sequence. This involves:
		/// - resetting the amount left,
		/// - recalculating the price based on the current price and the price increments defined by the `delta_price`.
		fn next(&mut self) {
			self.amount_left = self.delta_amount;
			self.current_price = self.current_price.saturating_add(self.delta_price);
		}
	}
}

pub mod inner_types {
	use super::*;
	use variant_count::VariantCount;

	pub enum MetadataError {
		/// The minimum price per token is too low.
		PriceTooLow,
		/// The ticket sizes are not valid.
		TicketSizeError,
		/// The participation currencies are not unique.
		ParticipationCurrenciesError,
		/// The allocation size is invalid. Either zero or higher than the max supply.
		AllocationSizeError,
		/// The auction round percentage cannot be zero.
		AuctionRoundPercentageError,
		/// The funding target has to be higher than 1000 USD.
		FundingTargetTooLow,
		/// The funding target has to be lower than 1bn USD.
		FundingTargetTooHigh,
		/// The project's metadata hash is not provided while starting the evaluation round.
		CidNotProvided,
		/// The ct decimals specified for the CT is outside the 4 to 20 range.
		BadDecimals,
		// The combination of decimals and price of this project is not representable within our 6 decimals USD system,
		// and integer space of 128 bits.
		BadTokenomics,
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
	pub struct CurrencyMetadata<BoundedString> {
		/// The user-friendly name of this asset. Limited in length by `StringLimit`.
		pub name: BoundedString,
		/// The ticker symbol for this asset. Limited in length by `StringLimit`.
		pub symbol: BoundedString,
		/// The number of decimals this asset uses to represent one unit.
		pub decimals: u8,
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
	pub struct TicketSize<Balance: PartialOrd + Copy> {
		pub usd_minimum_per_participation: Balance,
		pub usd_maximum_per_did: Option<Balance>,
	}
	impl<Balance: PartialOrd + Copy> TicketSize<Balance> {
		pub fn new(usd_minimum_per_participation: Balance, usd_maximum_per_did: Option<Balance>) -> Self {
			Self { usd_minimum_per_participation, usd_maximum_per_did }
		}

		pub fn usd_ticket_above_minimum_per_participation(&self, usd_amount: Balance) -> bool {
			usd_amount >= self.usd_minimum_per_participation
		}

		pub fn usd_ticket_below_maximum_per_did(&self, usd_amount: Balance) -> bool {
			match self.usd_maximum_per_did {
				Some(max) => usd_amount <= max,
				None => true,
			}
		}

		pub fn check_valid(&self, bound: Bound<Balance>) -> bool {
			if let (min, Some(max)) = (self.usd_minimum_per_participation, self.usd_maximum_per_did) {
				if min > max {
					return false
				}
			}

			if self.usd_minimum_per_participation < bound.lower {
				return false;
			}

			if let Some(upper_bound) = bound.upper {
				let Some(max_usd) = self.usd_maximum_per_did else { return false };
				if max_usd > upper_bound {
					return false;
				}
			}
			true
		}
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
	pub struct BiddingTicketSizes<Price: FixedPointNumber, Balance: PartialOrd + Copy> {
		pub professional: TicketSize<Balance>,
		pub institutional: TicketSize<Balance>,
		pub phantom: PhantomData<(Price, Balance)>,
	}
	impl<Price: FixedPointNumber, Balance: PartialOrd + Copy> BiddingTicketSizes<Price, Balance> {
		pub fn is_valid(&self, usd_bounds: Vec<InvestorTypeUSDBounds<Balance>>) -> Result<(), MetadataError> {
			for bound in usd_bounds {
				match bound {
					InvestorTypeUSDBounds::Professional(bound) =>
						if !self.professional.check_valid(bound) {
							return Err(MetadataError::TicketSizeError);
						},
					InvestorTypeUSDBounds::Institutional(bound) =>
						if !self.institutional.check_valid(bound) {
							return Err(MetadataError::TicketSizeError);
						},
					_ => {},
				}
			}
			Ok(())
		}
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
	pub struct ContributingTicketSizes<Price: FixedPointNumber, Balance: PartialOrd + Copy> {
		pub retail: TicketSize<Balance>,
		pub professional: TicketSize<Balance>,
		pub institutional: TicketSize<Balance>,
		pub phantom: PhantomData<(Price, Balance)>,
	}
	impl<Price: FixedPointNumber, Balance: PartialOrd + Copy> ContributingTicketSizes<Price, Balance> {
		pub fn is_valid(&self, usd_bounds: Vec<InvestorTypeUSDBounds<Balance>>) -> Result<(), MetadataError> {
			for bound in usd_bounds {
				match bound {
					InvestorTypeUSDBounds::Professional(bound) =>
						if !self.professional.check_valid(bound) {
							return Err(MetadataError::TicketSizeError);
						},
					InvestorTypeUSDBounds::Institutional(bound) =>
						if !self.institutional.check_valid(bound) {
							return Err(MetadataError::TicketSizeError);
						},
					InvestorTypeUSDBounds::Retail(bound) =>
						if !self.retail.check_valid(bound) {
							return Err(MetadataError::TicketSizeError);
						},
				}
			}
			Ok(())
		}
	}

	#[derive(
		VariantCount, Clone, Copy, Encode, Decode, Eq, PartialEq, PartialOrd, Ord, RuntimeDebug, TypeInfo, MaxEncodedLen,
	)]
	#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
	pub enum AcceptedFundingAsset {
		#[codec(index = 0)]
		USDT,
		#[codec(index = 1)]
		USDC,
		#[codec(index = 2)]
		DOT,
	}
	impl AcceptedFundingAsset {
		pub const fn to_assethub_id(&self) -> u32 {
			match self {
				AcceptedFundingAsset::USDT => 1984,
				AcceptedFundingAsset::DOT => 10,
				AcceptedFundingAsset::USDC => 1337,
			}
		}
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
	pub enum ProjectStatus {
		#[default]
		Application,
		EvaluationRound,
		AuctionInitializePeriod,
		AuctionOpening,
		AuctionClosing,
		CalculatingWAP,
		CommunityRound,
		RemainderRound,
		FundingFailed,
		AwaitingProjectDecision,
		FundingSuccessful,
		ReadyToStartMigration,
		MigrationCompleted,
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct PhaseTransitionPoints<BlockNumber> {
		pub application: BlockNumberPair<BlockNumber>,
		pub evaluation: BlockNumberPair<BlockNumber>,
		pub auction_initialize_period: BlockNumberPair<BlockNumber>,
		pub auction_opening: BlockNumberPair<BlockNumber>,
		pub random_closing_ending: Option<BlockNumber>,
		pub auction_closing: BlockNumberPair<BlockNumber>,
		pub community: BlockNumberPair<BlockNumber>,
		pub remainder: BlockNumberPair<BlockNumber>,
	}

	impl<BlockNumber: Copy> PhaseTransitionPoints<BlockNumber> {
		pub const fn new(now: BlockNumber) -> Self {
			Self {
				application: BlockNumberPair::new(Some(now), None),
				evaluation: BlockNumberPair::new(None, None),
				auction_initialize_period: BlockNumberPair::new(None, None),
				auction_opening: BlockNumberPair::new(None, None),
				random_closing_ending: None,
				auction_closing: BlockNumberPair::new(None, None),
				community: BlockNumberPair::new(None, None),
				remainder: BlockNumberPair::new(None, None),
			}
		}
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct BlockNumberPair<BlockNumber> {
		pub start: Option<BlockNumber>,
		pub end: Option<BlockNumber>,
	}

	impl<BlockNumber: Copy> BlockNumberPair<BlockNumber> {
		pub const fn new(start: Option<BlockNumber>, end: Option<BlockNumber>) -> Self {
			Self { start, end }
		}

		pub const fn start(&self) -> Option<BlockNumber> {
			self.start
		}

		pub const fn end(&self) -> Option<BlockNumber> {
			self.end
		}

		pub fn update(&mut self, start: Option<BlockNumber>, end: Option<BlockNumber>) {
			let new_state = match (start, end) {
				(Some(start), None) => (Some(start), self.end),
				(None, Some(end)) => (self.start, Some(end)),
				(Some(start), Some(end)) => (Some(start), Some(end)),
				(None, None) => (self.start, self.end),
			};
			(self.start, self.end) = (new_state.0, new_state.1);
		}

		pub fn force_update(&mut self, start: Option<BlockNumber>, end: Option<BlockNumber>) -> Self {
			Self { start, end }
		}
	}

	#[derive(Default, Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum BidStatus<Balance: BalanceT> {
		/// The bid is not yet accepted or rejected
		#[default]
		YetUnknown,
		/// The bid is accepted
		Accepted,
		/// The bid is rejected, and the reason is provided
		Rejected(RejectionReason),
		/// The bid is partially accepted. The amount accepted and reason for rejection are provided
		PartiallyAccepted(Balance, RejectionReason),
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum RejectionReason {
		/// The bid was submitted after the closing period ended
		AfterClosingEnd,
		/// The bid was accepted but too many tokens were requested. A partial amount was accepted
		NoTokensLeft,
		/// Error in calculating ticket_size for partially funded request
		BadMath,
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct VestingInfo<BlockNumber, Balance> {
		pub total_amount: Balance,
		pub amount_per_block: Balance,
		pub duration: BlockNumber,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum ProjectPhases {
		Evaluation,
		AuctionInitializePeriod,
		AuctionOpening,
		AuctionClosing,
		CalculatingWAP,
		CommunityFunding,
		RemainderFunding,
		DecisionPeriod,
		FundingFinalization(ProjectOutcome),
		Settlement,
		Migration,
	}

	/// An enum representing all possible outcomes for a project.
	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum ProjectOutcome {
		/// The evaluation funding target was not reached.
		EvaluationFailed,
		/// 90%+ of the funding target was reached, so the project is successful.
		FundingSuccessful,
		/// 33%- of the funding target was reached, so the project failed.
		FundingFailed,
		/// The project issuer accepted the funding outcome between 33% and 90% of the target.
		FundingAccepted,
		/// The project issuer rejected the funding outcome between 33% and 90% of the target.
		FundingRejected,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct EvaluationRoundInfo<Balance> {
		pub total_bonded_usd: Balance,
		pub total_bonded_plmc: Balance,
		pub evaluators_outcome: EvaluatorsOutcome<Balance>,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum EvaluatorsOutcome<Balance> {
		Unchanged,
		Rewarded(RewardInfo<Balance>),
		Slashed,
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, PartialOrd, Ord, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum RewardOrSlash<Balance> {
		Reward(Balance),
		Slash(Balance),
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct RewardInfo<Balance> {
		// Total "Early Evaluators" rewards amount in Contribution Tokens
		pub early_evaluator_reward_pot: Balance,
		// Total "Normal Evaluators" rewards amount in Contribution Tokens
		pub normal_evaluator_reward_pot: Balance,
		pub early_evaluator_total_bonded_usd: Balance,
		pub normal_evaluator_total_bonded_usd: Balance,
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum FundingOutcomeDecision {
		AcceptFunding,
		RejectFunding,
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct MigrationReadinessCheck {
		pub holding_check: (xcm::v3::QueryId, CheckOutcome),
		pub pallet_check: (xcm::v3::QueryId, CheckOutcome),
	}

	impl MigrationReadinessCheck {
		pub fn is_ready(&self) -> bool {
			self.holding_check.1 == CheckOutcome::Passed && self.pallet_check.1 == CheckOutcome::Passed
		}
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum CheckOutcome {
		AwaitingResponse,
		Passed,
		Failed,
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct HRMPChannelStatus {
		pub project_to_polimec: ChannelStatus,
		pub polimec_to_project: ChannelStatus,
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum ChannelStatus {
		/// hrmp channel is closed.
		Closed,
		/// hrmp channel is open.
		Open,
		/// request for a hrmp channel was sent to the relay. Waiting for response.
		AwaitingAcceptance,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct ProjectMigrationOrigins<ProjectId, MigrationOrigins> {
		pub project_id: ProjectId,
		pub migration_origins: MigrationOrigins,
	}
}
