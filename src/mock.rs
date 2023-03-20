use crate as functionland_fula;
use frame_support::{
    parameter_types,
    traits::{Everything, OnFinalize, OnInitialize},
    PalletId,
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};
use sugarfunge_primitives::Balance;

pub const MILLICENTS: Balance = 10_000_000_000_000;
pub const CENTS: Balance = 1_000 * MILLICENTS;
pub const DOLLARS: Balance = 100 * CENTS;

parameter_types! {
    pub const CreateAssetClassDeposit: Balance = 500 * MILLICENTS;
    pub const CreateCurrencyClassDeposit: Balance = 500 * MILLICENTS;
    pub const CreateBagDeposit: Balance = 1;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 500;
    pub const MaxClassMetadata: u32 = 1;
    pub const MaxAssetMetadata: u32 = 1;
}

impl pallet_balances::Config for Test {
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
}

impl sugarfunge_asset::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CreateAssetClassDeposit = CreateAssetClassDeposit;
    type Currency = Balances;
    type AssetId = u64;
    type ClassId = u64;
    type MaxClassMetadata = MaxClassMetadata;
    type MaxAssetMetadata = MaxAssetMetadata;
}

parameter_types! {
    pub const FulaModuleId: PalletId = PalletId(*b"fun/fula");
}

parameter_types! {
    pub const StringLimit: u32 = u8::MAX as u32;
    pub const MaxPoolParticipants: u32 = u8::MAX as u32;
}

impl fula_pool::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type StringLimit = StringLimit;
    type MaxPoolParticipants = MaxPoolParticipants;
}

parameter_types! {
    pub const MaxManifestMetadata: u32 = 128;
    pub const MaxCID: u32 = 128;
}

impl functionland_fula::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type PalletId = FulaModuleId;
    type MaxManifestMetadata = MaxManifestMetadata;
    type MaxCID = MaxCID;
    type Pool = Pool;
    type Currency = Balances;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Asset: sugarfunge_asset::{Pallet, Call, Storage, Event<T>},
        Pool: fula_pool::{Pallet, Call, Storage, Event<T>},
        Fula: functionland_fula::{Pallet, Call, Storage, Event<T>},
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
}

impl system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap()
        .into()
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        Fula::on_finalize(System::block_number());
        Balances::on_finalize(System::block_number());
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
        Balances::on_initialize(System::block_number());
        Fula::on_initialize(System::block_number());
    }
}