use frame_support::pallet_prelude::*;
use sp_runtime::traits::Zero;

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Project<
	AccountId,
	BoundedString,
	Balance: MaxEncodedLen + Zero + sp_std::cmp::PartialEq + sp_std::cmp::PartialOrd,
> {
	/// The issuer of the  certificate
	pub issuer_certificate: Issuer,
	/// Name of the issuer
	pub issuer_name: BoundedString,
	/// Token information
	pub token_information: CurrencyMetadata<BoundedString>,
	/// Total allocation of contribution tokens available for the funding round
	pub total_allocation_size: Balance,
	/// Minimum price per contribution token
	pub minimum_price: Balance,
	/// Fundraising target amount in USD equivalent
	pub fundraising_target: Balance,
	/// Maximum and/or minimum ticket size
	pub ticket_size: TicketSize<Balance>,
	/// Maximum and/or minimum number of participants for the Auction and Community Round
	pub participants_size: ParticipantsSize,
	/// Funding round thresholds for retail, professional and institutional participants
	pub funding_thresholds: Thresholds,
	/// Conversion rate of contribution token to mainnet token
	pub conversion_rate: u32,
	/// Participation currencies (e.g stablecoin, DOT, KSM)
	/// TODO: Use something like BoundedVec<Option<Currencies>, StringLimit>
	/// e.g. https://github.com/paritytech/substrate/blob/427fd09bcb193c1e79dec85b1e207c718b686c35/frame/uniques/src/types.rs#L110
	/// For now is easier to handle the case where only just one Currency is accepted
	pub participation_currencies: Currencies,
	/// Issuer destination accounts for accepted participation currencies (for receiving
	/// contributions)
	pub destinations_account: AccountId,
	/// Additional metadata
	/// TODO: Maybe we can move this to the ProjectInfo struct
	pub metadata: ProjectMetadata<BoundedString>,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct ProjectInfo<
	// ProjectIdentifier,
	BlockNumber,
	Balance: MaxEncodedLen + Zero + sp_std::cmp::PartialEq + sp_std::cmp::PartialOrd,
> {
	// TODO: Maybe we can save the ProjectIdentifier here
	// pub project_id: ProjectIdentifier;
	/// Whether the project is frozen, so no `metadata` changes are allowed.
	pub is_frozen: bool,
	/// The price decided after the Auction Round
	pub final_price: Option<Balance>,
	/// When the project is created
	pub created_at: BlockNumber,
	/// The current status of the project
	pub project_status: ProjectStatus,
	/// When the Auction Round ends, None before the random selection, Some(value) after the random
	/// selection
	pub auction_round_end: Option<BlockNumber>,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct ProjectMetadata<BoundedString> {
	/// A link to the whitepaper
	pub whitepaper: BoundedString,
	/// A link to a team description
	pub team_description: BoundedString,
	/// A link to the tokenomics description
	pub tokenomics: BoundedString,
	/// Total supply of mainnet tokens
	// TODO: Maybe this has to become something similar to `pub total_supply: Balance`
	pub total_supply: u128,
	/// A link to the roadmap
	pub roadmap: BoundedString,
	/// A link to a description on how the funds will be used
	pub usage_of_founds: BoundedString,
}

#[derive(Debug)]
pub enum ValidityError {
	PriceTooLow,
	TicketSizeError,
	ParticipantsSizeError,
}

impl<
		AccountId,
		BoundedString,
		Balance: MaxEncodedLen + Zero + sp_std::cmp::PartialEq + sp_std::cmp::PartialOrd,
	> Project<AccountId, BoundedString, Balance>
{
	// TODO: Perform a REAL validity check
	pub fn validity_check(&self) -> Result<(), ValidityError> {
		if self.minimum_price == Balance::zero() {
			return Err(ValidityError::PriceTooLow)
		}
		self.ticket_size.is_valid()?;
		self.participants_size.is_valid()?;
		Ok(())
	}
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct TicketSize<
	Balance: MaxEncodedLen + Zero + sp_std::cmp::PartialEq + sp_std::cmp::PartialOrd,
> {
	pub minimum: Option<Balance>,
	pub maximum: Option<Balance>,
}

impl<Balance: MaxEncodedLen + Zero + sp_std::cmp::PartialEq + sp_std::cmp::PartialOrd>
	TicketSize<Balance>
{
	fn is_valid(&self) -> Result<(), ValidityError> {
		if self.minimum.is_some() && self.maximum.is_some() {
			if self.minimum < self.maximum {
				return Ok(())
			} else {
				return Err(ValidityError::TicketSizeError)
			}
		}
		if self.minimum.is_some() || self.maximum.is_some() {
			return Ok(())
		}

		Err(ValidityError::TicketSizeError)
	}
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct ParticipantsSize {
	pub minimum: Option<u32>,
	pub maximum: Option<u32>,
}

impl ParticipantsSize {
	fn is_valid(&self) -> Result<(), ValidityError> {
		if self.minimum.is_some() && self.maximum.is_some() {
			if self.minimum < self.maximum {
				return Ok(())
			} else {
				return Err(ValidityError::ParticipantsSizeError)
			}
		}
		if self.minimum.is_some() || self.maximum.is_some() {
			return Ok(())
		}

		Err(ValidityError::ParticipantsSizeError)
	}
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Thresholds {
	#[codec(compact)]
	retail: u64,
	#[codec(compact)]
	professional: u64,
	#[codec(compact)]
	institutional: u64,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct CurrencyMetadata<BoundedString> {
	/// The user friendly name of this asset. Limited in length by `StringLimit`.
	pub name: BoundedString,
	/// The ticker symbol for this asset. Limited in length by `StringLimit`.
	pub symbol: BoundedString,
	/// The number of decimals this asset uses to represent one unit.
	pub decimals: u8,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct EvaluationMetadata<BlockNumber, Balance: MaxEncodedLen> {
	/// When (expressed in block numbers) the evaluation phase ends
	pub evaluation_period_ends: BlockNumber,
	/// The amount of PLMC bonded in the project during the evaluation phase
	#[codec(compact)]
	pub amount_bonded: Balance,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AuctionMetadata<BlockNumber> {
	/// When (expressed in block numbers) the Auction Round started
	pub starting_block: BlockNumber,
	/// When (expressed in block numbers) the English Auction phase ends
	pub english_ending_block: BlockNumber,
	/// When (expressed in block numbers) the Candle Auction phase ends
	pub candle_ending_block: BlockNumber,
	/// When (expressed in block numbers) the Community Round ends
	pub community_ending_block: BlockNumber,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct BidInfo<Balance: MaxEncodedLen, BlockNumber> {
	///
	#[codec(compact)]
	pub amount: Balance,
	///
	#[codec(compact)]
	pub market_cap: Balance,
	///
	pub when: BlockNumber,
}

// Enums
// TODO: Use SCALE fixed indexes
// TODO: Check if it's correct
#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum Issuer {
	#[default]
	Kilt,
	Other,
}

// TODO: Use SCALE fixed indexes
#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum Currencies {
	DOT,
	KSM,
	#[default]
	USDC,
	USDT,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum ProjectStatus {
	#[default]
	Application,
	EvaluationRound,
	EvaluationEnded,
	AuctionRound(AuctionPhase),
	CommunityRound,
	ReadyToLaunch,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum AuctionPhase {
	#[default]
	English,
	Candle,
}