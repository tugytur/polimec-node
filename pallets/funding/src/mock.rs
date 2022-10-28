use core::cell::RefCell;

use crate as pallet_funding;
use frame_support::{
	pallet_prelude::ConstU32,
	parameter_types,
	traits::{ConstU16, Randomness},
	PalletId,
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type Balance = u128;
pub type BlockNumber = u64;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		FundingModule: pallet_funding,
	}
);

parameter_types! {
	pub const BlockHashCount: u32 = 250;
}

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub static ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Config for Test {
	type MaxLocks = frame_support::traits::ConstU32<1024>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
}

thread_local! {
	pub static LAST_RANDOM: RefCell<Option<(H256,u64)>>  = RefCell::new(None);
}
fn set_last_random(output: H256, known_since: u64) {
	LAST_RANDOM.with(|p| *p.borrow_mut() = Some((output, known_since)))
}
pub struct TestPastRandomness;
impl Randomness<H256, BlockNumber> for TestPastRandomness {
	fn random(_subject: &[u8]) -> (H256, u64) {
		LAST_RANDOM.with(|p| {
			if let Some((output, known_since)) = &*p.borrow() {
				(*output, *known_since)
			} else {
				let block_number: u64 = frame_system::Pallet::<Test>::block_number();
				(H256::zero(), block_number)
			}
		})
	}
}

parameter_types! {
	// TODO: Replace 28 with the real time
	pub const EvaluationDuration: BlockNumber = 28;
	// TODO: Replace 7 with the real time
	pub const EnglishAuctionDuration: BlockNumber = 10;
	// TODO: Use the correct Candle Duration
	pub const CandleAuctionDuration: BlockNumber = 5;
	// TODO:
	pub const CommunityRoundDuration: BlockNumber = 10;
	pub const FundingPalletId: PalletId = PalletId(*b"py/cfund");
}

impl pallet_funding::Config for Test {
	type Event = Event;
	type StringLimit = ConstU32<64>;
	type Currency = Balances;
	type CurrencyBalance = <Self as pallet_balances::Config>::Balance;
	type EvaluationDuration = EvaluationDuration;
	type EnglishAuctionDuration = EnglishAuctionDuration;
	type CandleAuctionDuration = CandleAuctionDuration;
	type PalletId = FundingPalletId;
	type ActiveProjectsLimit = ConstU32<100>;
	type CommunityRoundDuration = CommunityRoundDuration;
	type Randomness = TestPastRandomness;
}

// Build genesis storage according to the mock runtime.
// TODO: Add some mocks projects at Genesis to simplify the tests
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

	GenesisConfig {
		balances: BalancesConfig {
			balances: vec![(1, 512), (2, 512), (3, 512), (4, 512), (5, 512)],
		},
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	// In order to emit events the block number must be more than 0
	ext.execute_with(|| System::set_block_number(1));
	ext
}