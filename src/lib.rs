#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
    dispatch::DispatchResult,
    ensure,
    traits::{Currency, Get, ReservableCurrency},
    BoundedVec, PalletId,
};
use frame_system::{self as system};
use fula_pool::PoolInterface;
use libm::exp;
use scale_info::TypeInfo;
use sp_runtime::traits::BlakeTwo256;
use sp_runtime::traits::Hash;
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;
use sp_std::vec::Vec;

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;

const YEARLY_TOKENS: u64 = 48000000;

const DAILY_TOKENS_MINING: f64 = YEARLY_TOKENS as f64 * 0.70 / (12 * 30) as f64;
const DAILY_TOKENS_STORAGE: f64 = YEARLY_TOKENS as f64 * 0.20 / (12 * 30) as f64;

const NUMBER_CYCLES_TO_ADVANCE: u16 = 3;
const NUMBER_CYCLES_TO_RESET: u16 = 3;
pub trait MaxRange {
    type Range;
    fn random(max_range: Self::Range) -> u64;
}

pub type Range = u64;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// Manifest struct store the data related to an specific CID
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Manifest<AccountId, ManifestMetadataOf> {
    pub users_data: Vec<UploaderData<AccountId>>,
    pub manifest_metadata: ManifestMetadataOf,
    pub size: Option<FileSize>,
}

// Struct to store the data related to the uploader of a manifest
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct UploaderData<AccountId> {
    pub uploader: AccountId,
    pub storers: Vec<AccountId>,
    pub replication_factor: ReplicationFactor,
}

// Manifest struct for the call Get_manifests
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ManifestWithPoolId<PoolId, AccountId, ManifestMetadataOf> {
    pub pool_id: PoolId,
    pub users_data: Vec<UploaderData<AccountId>>,
    pub manifest_metadata: ManifestMetadataOf,
    pub size: Option<FileSize>,
}

// Manifest struct for the call Get_available_manifests
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ManifestAvailable<PoolId, ManifestMetadataOf> {
    pub pool_id: PoolId,
    pub replication_factor: ReplicationFactor,
    pub manifest_metadata: ManifestMetadataOf,
}

//Manifest struct for the call Get_manifest_storers_data
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct StorerData<PoolId, Cid, AccountId> {
    pub pool_id: PoolId,
    pub cid: Cid,
    pub account: AccountId,
    pub manifest_data: ManifestStorageData,
}

// Challenge Struct that will store the Open challenges to verify the existence of a file in the challenged IPFS node
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Challenge<AccountId> {
    pub challenger: AccountId,
    pub challenge_state: ChallengeState,
}

// Enum that represent the current verify state of a file on-chain if a challenge was successful / failed / open
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum ChallengeState {
    Open,
    Successful,
    Failed,
}

// Manifests struct to store the variables needed to the calculation of the labor_tokens of a given file
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ManifestStorageData {
    pub active_cycles: Cycles,
    pub missed_cycles: Cycles,
    pub active_days: ActiveDays,
    pub challenge_state: ChallengeState,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
    use frame_system::pallet_prelude::*;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config + sugarfunge_asset::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        #[pallet::constant]
        type PalletId: Get<PalletId>;

        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        // Constant values
        #[pallet::constant]
        type MaxManifestMetadata: Get<u32>;
        type MaxCID: Get<u32>;
        type Pool: PoolInterface<AccountId = Self::AccountId>;
    }
    // Custom types used to handle the calls and events
    pub type ManifestMetadataOf<T> = BoundedVec<u8, <T as Config>::MaxManifestMetadata>;
    pub type CIDOf<T> = BoundedVec<u8, <T as Config>::MaxCID>;
    pub type PoolIdOf<T> = <<T as Config>::Pool as PoolInterface>::PoolId;
    pub type ClassId = u64;
    pub type AssetId = u64;
    pub type MintBalance = u128;
    pub type FileSize = u64;
    pub type ReplicationFactor = u16;
    pub type Cycles = u16;
    pub type ActiveDays = i32;
    pub type ManifestOf<T> =
        Manifest<<T as frame_system::Config>::AccountId, ManifestMetadataOf<T>>;
    pub type ManifestWithPoolIdOf<T> = ManifestWithPoolId<
        PoolIdOf<T>,
        <T as frame_system::Config>::AccountId,
        ManifestMetadataOf<T>,
    >;
    pub type ManifestAvailableOf<T> = ManifestAvailable<PoolIdOf<T>, ManifestMetadataOf<T>>;
    pub type StorerDataOf<T> =
        StorerData<PoolIdOf<T>, CIDOf<T>, <T as frame_system::Config>::AccountId>;
    pub type ChallengeRequestsOf<T> = Challenge<<T as frame_system::Config>::AccountId>;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // Storage to keep the manifest data - Keys: Pool_id (Pool of the file) - CID (Identifier of the file)
    #[pallet::storage]
    #[pallet::getter(fn manifests)]
    pub(super) type Manifests<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        PoolIdOf<T>,
        Blake2_128Concat,
        CIDOf<T>,
        ManifestOf<T>,
    >;

    // Storage to keep the data of the storers - Pool_id (Pool of the file) - Account (of the storer) - CID (Identifier of the file)
    #[pallet::storage]
    #[pallet::getter(fn manifests_storage_data)]
    pub(super) type ManifestsStorerData<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, PoolIdOf<T>>,
            NMapKey<Blake2_128Concat, T::AccountId>,
            NMapKey<Blake2_128Concat, CIDOf<T>>,
        ),
        ManifestStorageData,
        OptionQuery,
    >;

    // Storage to keep the open challenge Requests to verify the existence of a file - Keys: Account (of the user challenged) - CID (Identifier of the file)
    #[pallet::storage]
    #[pallet::getter(fn challenges)]
    pub(super) type ChallengeRequests<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        CIDOf<T>,
        ChallengeRequestsOf<T>,
    >;

    // A value to keep track of the Network size when a file is stored or removed
    #[pallet::storage]
    pub type NetworkSize<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ManifestOutput {
            uploader: T::AccountId,
            storer: Vec<T::AccountId>,
            pool_id: PoolIdOf<T>,
            manifest: Vec<u8>,
        },
        StorageManifestOutput {
            storer: T::AccountId,
            pool_id: PoolIdOf<T>,
            cid: Vec<u8>,
        },
        RemoveStorerOutput {
            storer: Option<T::AccountId>,
            pool_id: PoolIdOf<T>,
            cid: Vec<u8>,
        },
        ManifestRemoved {
            uploader: T::AccountId,
            pool_id: PoolIdOf<T>,
            cid: Vec<u8>,
        },
        ManifestStorageUpdated {
            storer: T::AccountId,
            pool_id: PoolIdOf<T>,
            cid: Vec<u8>,
            active_cycles: Cycles,
            missed_cycles: Cycles,
            active_days: ActiveDays,
        },
        BatchManifestOutput {
            uploader: T::AccountId,
            pool_ids: Vec<PoolIdOf<T>>,
            manifests: Vec<Vec<u8>>,
        },
        BatchStorageManifestOutput {
            storer: T::AccountId,
            pool_id: PoolIdOf<T>,
            cids: Vec<Vec<u8>>,
        },
        BatchRemoveStorerOutput {
            storer: T::AccountId,
            pool_id: PoolIdOf<T>,
            cids: Vec<Vec<u8>>,
        },
        BatchManifestRemoved {
            uploader: T::AccountId,
            pool_ids: Vec<PoolIdOf<T>>,
            cids: Vec<Vec<u8>>,
        },
        VerifiedStorerManifests {
            storer: T::AccountId,
            valid_cids: Vec<Vec<u8>>,
            invalid_cids: Vec<Vec<u8>>,
        },
        UpdateFileSizeOutput {
            account: T::AccountId,
            pool_id: PoolIdOf<T>,
            cid: Vec<u8>,
            size: u64,
        },
        UpdateFileSizesOutput {
            account: T::AccountId,
            pool_id: PoolIdOf<T>,
            cids: Vec<Vec<u8>>,
            sizes: Vec<u64>,
        },
        GetManifests {
            manifests: Vec<ManifestWithPoolIdOf<T>>,
        },
        GetAvailableManifests {
            manifests: Vec<ManifestAvailableOf<T>>,
        },
        GetManifestsStorerData {
            manifests: Vec<StorerDataOf<T>>,
        },
        Challenge {
            challenger: T::AccountId,
            challenged: T::AccountId,
            cid: Vec<u8>,
            state: ChallengeState,
        },
        VerifiedChallenges {
            challenged: T::AccountId,
            successful: Vec<Vec<u8>>,
            failed: Vec<Vec<u8>>,
        },
        MintedLaborTokens {
            account: T::AccountId,
            class_id: ClassId,
            asset_id: AssetId,
            amount: MintBalance,
            calculated_amount: u64,
        },
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        NoneValue,
        StorageOverflow,
        ReplicationFactorLimitReached,
        ReplicationFactorInvalid,
        AccountAlreadyStorer,
        AccountNotStorer,
        AccountNotInPool,
        AccountNotUploader,
        ManifestAlreadyExist,
        ManifestNotFound,
        ManifestNotStored,
        InvalidArrayLength,
        ErrorPickingCIDToChallenge,
        ErrorPickingAccountToChallenge,
        ManifestStorerDataNotFound,
        NoFileSizeProvided,
    }

    // Dispatchable functions allows users to interact with the pallet and invoke state changes.
    // These functions materialize as "extrinsics", which are often compared to transactions.
    // Dispatchable functions must be annotated with a weight and must return a DispatchResult.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Updates the values of the manifest storer data given the specific data
        #[pallet::weight(10_000)]
        pub fn update_manifest(
            origin: OriginFor<T>,
            cid: CIDOf<T>,
            pool_id: PoolIdOf<T>,
            active_cycles: Cycles,
            missed_cycles: Cycles,
            active_days: ActiveDays,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_update_manifest(
                &who,
                pool_id,
                cid,
                active_days,
                active_cycles,
                missed_cycles,
            )?;
            Ok(().into())
        }

        // Upload a manifest to the chain
        #[pallet::weight(10_000)]
        pub fn upload_manifest(
            origin: OriginFor<T>,
            manifest: ManifestMetadataOf<T>,
            cid: CIDOf<T>,
            pool_id: PoolIdOf<T>,
            replication_factor: ReplicationFactor,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_upload_manifest(&who, pool_id, cid, manifest, replication_factor)?;
            Ok(().into())
        }

        // Upload multiple manifests to the chain
        #[pallet::weight(10_000)]
        pub fn batch_upload_manifest(
            origin: OriginFor<T>,
            manifest: Vec<ManifestMetadataOf<T>>,
            cids: Vec<CIDOf<T>>,
            pool_id: Vec<PoolIdOf<T>>,
            replication_factor: Vec<ReplicationFactor>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_batch_upload_manifest(&who, pool_id, cids, manifest, replication_factor)?;
            Ok(().into())
        }

        // Storage an available manifest from the chain
        #[pallet::weight(10_000)]
        pub fn storage_manifest(
            origin: OriginFor<T>,
            cid: CIDOf<T>,
            pool_id: PoolIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_storage_manifest(&who, pool_id, cid)?;
            Ok(().into())
        }

        // Storage multiple manifests from the chain
        #[pallet::weight(10_000)]
        pub fn batch_storage_manifest(
            origin: OriginFor<T>,
            cids: Vec<CIDOf<T>>,
            pool_id: PoolIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_batch_storage_manifest(&who, pool_id, cids)?;
            Ok(().into())
        }

        // A storer remove a manifest that was being stored on chain
        #[pallet::weight(10_000)]
        pub fn remove_stored_manifest(
            origin: OriginFor<T>,
            cid: CIDOf<T>,
            pool_id: PoolIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_remove_storer(&who, pool_id, cid)?;
            Ok(().into())
        }

        // A storer removed multiple manifests that were being stored on chain
        #[pallet::weight(10_000)]
        pub fn batch_remove_stored_manifest(
            origin: OriginFor<T>,
            cids: Vec<CIDOf<T>>,
            pool_id: PoolIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_batch_remove_storer(&who, pool_id, cids)?;
            Ok(().into())
        }

        // The Uploader remove the manifest from the chain
        #[pallet::weight(10_000)]
        pub fn remove_manifest(
            origin: OriginFor<T>,
            cid: CIDOf<T>,
            pool_id: PoolIdOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_remove_manifest(&who, pool_id, cid)?;
            Ok(().into())
        }

        // The Uploader remove multiple manifests from the chain
        #[pallet::weight(10_000)]
        pub fn batch_remove_manifest(
            origin: OriginFor<T>,
            cids: Vec<CIDOf<T>>,
            pool_ids: Vec<PoolIdOf<T>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_batch_remove_manifest(&who, pool_ids, cids)?;
            Ok(().into())
        }

        // A storer verifies if there are invalid manifests still stored on chain and removed them
        #[pallet::weight(10_000)]
        pub fn verify_manifests(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_verify_manifests(&who)?;
            Ok(().into())
        }

        // A call to get all the manifest from the chain with possible filters
        #[pallet::weight(10_000)]
        pub fn get_manifests(
            _origin: OriginFor<T>,
            pool_id: Option<PoolIdOf<T>>,
            uploader: Option<T::AccountId>,
            storer: Option<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            Self::do_get_manifests(pool_id, uploader, storer)?;
            Ok(().into())
        }

        // A call to get all available manifests from the chain with possible filters
        #[pallet::weight(10_000)]
        pub fn get_available_manifests(
            _origin: OriginFor<T>,
            pool_id: Option<PoolIdOf<T>>,
        ) -> DispatchResultWithPostInfo {
            Self::do_get_available_manifests(pool_id)?;
            Ok(().into())
        }

        // A call to get all the storers data from the chain with possible filters
        #[pallet::weight(10_000)]
        pub fn get_manifests_storer_data(
            _origin: OriginFor<T>,
            pool_id: Option<PoolIdOf<T>>,
            storer: Option<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            Self::do_get_manifest_storer_data(pool_id, storer)?;
            Ok(().into())
        }

        // Generates a challenge to verify if a storer is still holding a random CID that he has on chain
        #[pallet::weight(10_000)]
        pub fn generate_challenge(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::do_generate_challenge(&who)?;
            Ok(().into())
        }

        // Verifies if the challenged account has the CID that was challenged stored
        #[pallet::weight(10_000)]
        pub fn verify_challenge(
            origin: OriginFor<T>,
            pool_id: PoolIdOf<T>,
            cids: Vec<CIDOf<T>>,
            class_id: ClassId,
            asset_id: AssetId,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_verify_challenge(&who, cids, pool_id, class_id, asset_id)?;
            Ok(().into())
        }

        // Mint the labor tokens that are going to be used to changed for claimed tokens and then for Fula Tokens - This correspond to the previously known rewards
        #[pallet::weight(10_000)]
        pub fn mint_labor_tokens(
            origin: OriginFor<T>,
            class_id: ClassId,
            asset_id: AssetId,
            amount: MintBalance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_mint_labor_tokens(&who, class_id, asset_id, amount)?;
            Ok(().into())
        }

        // Updates the file_size of a manifest in the chain
        #[pallet::weight(10_000)]
        pub fn update_file_size(
            origin: OriginFor<T>,
            cid: CIDOf<T>,
            pool_id: PoolIdOf<T>,
            size: FileSize,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_update_size(&who, pool_id, cid, size)?;
            Ok(().into())
        }

        // Updates multiple file_sizes of manifests in the chain
        #[pallet::weight(10_000)]
        pub fn update_file_sizes(
            origin: OriginFor<T>,
            cids: Vec<CIDOf<T>>,
            pool_id: PoolIdOf<T>,
            sizes: Vec<FileSize>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::do_update_sizes(&who, pool_id, cids, sizes)?;
            Ok(().into())
        }
    }

    impl<T: Config> MaxRange for Pallet<T> {
        type Range = Range;
        fn random(max_range: Self::Range) -> u64 {
            //let who = system::ensure_signed(origin)?;
            let block_number = <system::Pallet<T>>::block_number();

            let mut input = Vec::new();
            //input.extend_from_slice(&who.encode());
            input.extend_from_slice(&block_number.encode());

            let hash_result = BlakeTwo256::hash_of(&input);

            let random_number = T::Hashing::hash(hash_result.as_bytes());

            let result = random_number
                .as_ref()
                .iter()
                .take(8) // take only the first 8 bytes
                .fold(0, |acc, &byte| (acc << 8) + byte as u32)
                % max_range as u32;

            return result as u64;
        }
    }
}

impl<T: Config> Pallet<T> {
    pub fn do_upload_manifest(
        uploader: &T::AccountId,
        pool_id: PoolIdOf<T>,
        cid: CIDOf<T>,
        manifest: ManifestMetadataOf<T>,
        replication_factor: ReplicationFactor,
    ) -> DispatchResult {
        // Validations made to verify some parameters given
        ensure!(replication_factor > 0, Error::<T>::ReplicationFactorInvalid);

        // Auxiliar structures
        let mut uploader_vec = Vec::new();
        let storers_vec = Vec::new();

        let uploader_data = UploaderData {
            uploader: uploader.clone(),
            storers: storers_vec.to_owned(),
            replication_factor,
        };
        // If the CID of the manifest already exists the  it creates another Uploader into the Vec of uploader data, if not then it creates the CID in the manifests storage
        if let Some(_manifest) = Self::manifests(pool_id, cid.clone()) {
            Manifests::<T>::try_mutate(pool_id, cid.clone(), |value| -> DispatchResult {
                if let Some(manifest) = value {
                    manifest.users_data.push(uploader_data)
                }
                Ok(())
            })?;
        } else {
            uploader_vec.push(uploader_data);
            Manifests::<T>::insert(
                pool_id,
                cid,
                Manifest {
                    users_data: uploader_vec.to_owned(),
                    manifest_metadata: manifest.clone(),
                    size: None,
                },
            );
        }

        Self::deposit_event(Event::ManifestOutput {
            uploader: uploader.clone(),
            storer: storers_vec.to_owned(),
            manifest: manifest.to_vec(),
            pool_id: pool_id.clone(),
        });
        Ok(())
    }

    pub fn do_batch_upload_manifest(
        uploader: &T::AccountId,
        pool_ids: Vec<PoolIdOf<T>>,
        cids: Vec<CIDOf<T>>,
        manifests: Vec<ManifestMetadataOf<T>>,
        replication_factors: Vec<ReplicationFactor>,
    ) -> DispatchResult {
        // Validations made to verify some parameters given
        ensure!(
            cids.len() == manifests.len(),
            Error::<T>::InvalidArrayLength
        );
        ensure!(
            cids.len() == replication_factors.len(),
            Error::<T>::InvalidArrayLength
        );

        // The cycle to execute multiple times the upload_manifest call
        let n = cids.len();
        for i in 0..n {
            let cid = cids[i].to_owned();
            let manifest = manifests[i].to_owned();
            let replication_factor = replication_factors[i];
            let pool_id = pool_ids[i];
            Self::do_upload_manifest(uploader, pool_id, cid, manifest, replication_factor)?;
        }

        Self::deposit_event(Event::BatchManifestOutput {
            uploader: uploader.clone(),
            manifests: Self::transform_manifest_to_vec(manifests),
            pool_ids: pool_ids.clone(),
        });
        Ok(())
    }

    pub fn do_update_manifest(
        storer: &T::AccountId,
        pool_id: PoolIdOf<T>,
        cid: CIDOf<T>,
        active_days: ActiveDays,
        active_cycles: Cycles,
        missed_cycles: Cycles,
    ) -> DispatchResult {
        // Validations made to verify some parameters given
        ensure!(
            T::Pool::is_member(storer.clone(), pool_id),
            Error::<T>::AccountNotInPool
        );
        ensure!(
            ManifestsStorerData::<T>::try_get((pool_id, storer.clone(), cid.clone())).is_ok(),
            Error::<T>::ManifestNotStored
        );

        // Update the values in the ManifestStorerData storage
        ManifestsStorerData::<T>::try_mutate(
            (pool_id, storer, cid.clone()),
            |value| -> DispatchResult {
                if let Some(manifest) = value {
                    manifest.active_cycles = active_cycles;
                    manifest.missed_cycles = missed_cycles;
                    manifest.active_days = active_days;
                }
                Ok(())
            },
        )?;

        Self::deposit_event(Event::ManifestStorageUpdated {
            storer: storer.clone(),
            pool_id: pool_id.clone(),
            cid: cid.to_vec(),
            active_cycles,
            missed_cycles,
            active_days,
        });
        Ok(())
    }

    pub fn do_storage_manifest(
        storer: &T::AccountId,
        pool_id: PoolIdOf<T>,
        cid: CIDOf<T>,
    ) -> DispatchResult {
        // Validations made to verify some parameters given
        ensure!(
            T::Pool::is_member(storer.clone(), pool_id),
            Error::<T>::AccountNotInPool
        );
        ensure!(
            Manifests::<T>::try_get(pool_id, cid.clone()).is_ok(),
            Error::<T>::ManifestNotFound
        );
        // Add the storer to the manifest storage
        Manifests::<T>::try_mutate(pool_id, cid.clone(), |value| -> DispatchResult {
            if let Some(manifest) = value {
                // Get the next uploader that has available storers
                if let Some(index) =
                    Self::get_next_available_uploader_index(manifest.users_data.to_vec())
                {
                    ensure!(
                        !manifest.users_data[index].storers.contains(&storer.clone()),
                        Error::<T>::AccountAlreadyStorer
                    );
                    manifest.users_data[index].storers.push(storer.clone());
                    // Insert the default data to the storer data storage
                    ManifestsStorerData::<T>::insert(
                        (pool_id, storer, cid.clone()),
                        ManifestStorageData {
                            active_cycles: 0,
                            missed_cycles: 0,
                            active_days: 0,
                            challenge_state: ChallengeState::Open,
                        },
                    );
                }
                // If the manifest has the file_size available, update the network size
                if let Some(file_size) = manifest.size {
                    NetworkSize::<T>::mutate(|total_value| {
                        *total_value += file_size;
                    });
                }
            }
            Ok(())
        })?;

        Self::deposit_event(Event::StorageManifestOutput {
            storer: storer.clone(),
            pool_id: pool_id.clone(),
            cid: cid.to_vec(),
        });
        Ok(())
    }

    pub fn do_batch_storage_manifest(
        storer: &T::AccountId,
        pool_id: PoolIdOf<T>,
        cids: Vec<CIDOf<T>>,
    ) -> DispatchResult {
        // The cycle to execute multiple times the storage manifests
        let n = cids.len();
        for i in 0..n {
            let cid = cids[i].to_owned();
            Self::do_storage_manifest(storer, pool_id, cid)?;
        }

        Self::deposit_event(Event::BatchStorageManifestOutput {
            storer: storer.clone(),
            cids: Self::transform_cid_to_vec(cids),
            pool_id: pool_id.clone(),
        });
        Ok(())
    }

    pub fn do_remove_manifest(
        uploader: &T::AccountId,
        pool_id: PoolIdOf<T>,
        cid: CIDOf<T>,
    ) -> DispatchResult {
        // Validations made to verify some parameters given
        ensure!(
            Manifests::<T>::try_get(pool_id, cid.clone()).is_ok(),
            Error::<T>::ManifestNotFound
        );
        // Get the manifests and the uploaders
        let manifest = Self::manifests(pool_id, cid.clone()).unwrap();

        // Found the uploader in the vector of uploader data
        if let Some(index) =
            Self::get_uploader_index(manifest.users_data.to_owned(), uploader.clone())
        {
            // If there are more uploaders, just remove the value from the vector of uploaders, if not remove the entire manifest
            if manifest.users_data.to_owned().len() > 1 {
                Manifests::<T>::try_mutate(pool_id, cid.clone(), |value| -> DispatchResult {
                    if let Some(manifest) = value {
                        let value_removed = manifest.users_data.remove(index);
                        // Update the network size, removing the size that belong to the storers before removing the uploader
                        if let Some(file_size) = manifest.size {
                            NetworkSize::<T>::mutate(|total_value| {
                                *total_value -= file_size * value_removed.storers.len() as u64;
                            });
                        }
                    }
                    Ok(())
                })?;
            } else {
                // Update the network size, removing the size that belong to the storers before removing the uploader
                if let Some(file_size) = manifest.size {
                    NetworkSize::<T>::mutate(|total_value| {
                        *total_value -= file_size * manifest.users_data[0].storers.len() as u64;
                    });
                }
                Manifests::<T>::remove(pool_id, cid.clone());
            }
        }

        Self::deposit_event(Event::ManifestRemoved {
            uploader: uploader.clone(),
            cid: cid.to_vec(),
            pool_id: pool_id.clone(),
        });
        Ok(())
    }

    pub fn do_batch_remove_manifest(
        uploader: &T::AccountId,
        pool_ids: Vec<PoolIdOf<T>>,
        cids: Vec<CIDOf<T>>,
    ) -> DispatchResult {
        // Validations made to verify some parameters given
        ensure!(cids.len() == pool_ids.len(), Error::<T>::InvalidArrayLength);

        // The cycle to execute multiple times the remove manifests
        let n = cids.len();
        for i in 0..n {
            let cid = cids[i].to_owned();
            let pool_id = pool_ids[i];
            Self::do_remove_manifest(uploader, pool_id, cid)?;
        }

        Self::deposit_event(Event::BatchManifestRemoved {
            uploader: uploader.clone(),
            cids: Self::transform_cid_to_vec(cids),
            pool_ids: pool_ids.clone(),
        });
        Ok(())
    }

    pub fn do_remove_storer(
        storer: &T::AccountId,
        pool_id: PoolIdOf<T>,
        cid: CIDOf<T>,
    ) -> DispatchResult {
        // Validations made to verify some parameters given
        ensure!(
            Manifests::<T>::try_get(pool_id, cid.clone()).is_ok(),
            Error::<T>::ManifestNotFound
        );

        // Auxiliar structures
        let mut removed_storer = None;

        // Try to remove the storer from the manifest storage
        Manifests::<T>::try_mutate(pool_id, cid.clone(), |value| -> DispatchResult {
            if let Some(manifest) = value {
                // Verify if the account is a storer
                ensure!(
                    Self::verify_account_is_storer(manifest.users_data.to_owned(), storer),
                    Error::<T>::AccountNotStorer
                );
                // Get the uploader that correspond to the storer
                if let Some(uploader_index) =
                    Self::get_uploader_index_given_storer(manifest.users_data.to_owned(), storer)
                {
                    // Get the index of the storer inside the storers vector
                    if let Some(storer_index) = Self::get_storer_index(
                        manifest.users_data.to_owned(),
                        uploader_index,
                        storer.clone(),
                    ) {
                        // Remove the storer from the storers vector
                        let value_removed = manifest.users_data[uploader_index]
                            .storers
                            .remove(storer_index);
                        removed_storer = Some(value_removed);
                        // Update the network size, removing the size that belong to the removed storer
                        if let Some(file_size) = manifest.size {
                            NetworkSize::<T>::mutate(|total_value| {
                                *total_value -= file_size as u64;
                            });
                        }
                    }
                };
            }
            Ok(())
        })?;

        Self::deposit_event(Event::RemoveStorerOutput {
            storer: removed_storer,
            cid: cid.to_vec(),
            pool_id: pool_id.clone(),
        });
        Ok(())
    }

    pub fn do_batch_remove_storer(
        storer: &T::AccountId,
        pool_id: PoolIdOf<T>,
        cids: Vec<CIDOf<T>>,
    ) -> DispatchResult {
        // The cycle to execute multiple times the remove storers
        let n = cids.len();
        for i in 0..n {
            let cid = cids[i].to_owned();
            Self::do_remove_storer(storer, pool_id, cid)?;
        }

        Self::deposit_event(Event::BatchRemoveStorerOutput {
            storer: storer.clone(),
            cids: Self::transform_cid_to_vec(cids),
            pool_id: pool_id.clone(),
        });
        Ok(())
    }

    pub fn do_verify_manifests(storer: &T::AccountId) -> DispatchResult {
        // Auxiliar structures
        let mut invalid_cids = Vec::new();
        let mut valid_cids = Vec::new();

        // Verify the valid or invalid cids from the manifest storer data
        for item in ManifestsStorerData::<T>::iter() {
            if storer.clone() == item.0 .1.clone() {
                // If the storer has manifests from another pool that is not the current it's invalid and is removed
                if T::Pool::is_member(item.0 .1.clone(), item.0 .0) {
                    // If the manifest is still in the manifest storage is a valid cid, if not it's invalid and is removed
                    if Manifests::<T>::try_get(item.0 .0, item.0 .2.clone()).is_ok() {
                        valid_cids.push(item.0 .2.to_vec());
                    } else {
                        invalid_cids.push(item.0 .2.to_vec());

                        ManifestsStorerData::<T>::remove((
                            item.0 .0,
                            item.0 .1.clone(),
                            item.0 .2.clone(),
                        ));
                    }
                } else {
                    invalid_cids.push(item.0 .2.to_vec());
                    ManifestsStorerData::<T>::remove((
                        item.0 .0,
                        item.0 .1.clone(),
                        item.0 .2.clone(),
                    ));
                }
            }
        }

        Self::deposit_event(Event::VerifiedStorerManifests {
            storer: storer.clone(),
            valid_cids: valid_cids.to_vec(),
            invalid_cids: invalid_cids.to_vec(),
        });
        Ok(())
    }

    pub fn do_get_manifests(
        pool_id: Option<PoolIdOf<T>>,
        uploader: Option<T::AccountId>,
        storer: Option<T::AccountId>,
    ) -> DispatchResult {
        // Auxiliar structures
        let mut manifests_result = Vec::new();

        // Validate the manifests that match the filters
        for item in Manifests::<T>::iter() {
            let mut meet_requirements = true;

            // Checks for the pool_id
            if let Some(pool_id_value) = pool_id {
                if pool_id_value != item.0 {
                    meet_requirements = false;
                }
            }

            // Checks for the uploader account
            if let Some(ref uploader_value) = uploader {
                if Self::verify_account_is_uploader(
                    item.2.users_data.to_vec(),
                    uploader_value.clone(),
                ) {
                    meet_requirements = false;
                }
            }

            // Checks for the storer account
            if let Some(ref storer_value) = storer {
                if Self::verify_account_is_storer(item.2.users_data.to_vec(), storer_value) {
                    meet_requirements = false;
                }
            }

            // if everything match, add them to the vector of results
            if meet_requirements {
                manifests_result.push(ManifestWithPoolId {
                    pool_id: item.0,
                    users_data: item.2.users_data,
                    manifest_metadata: item.2.manifest_metadata,
                    size: item.2.size,
                });
            }
        }
        Self::deposit_event(Event::GetManifests {
            manifests: manifests_result,
        });
        Ok(())
    }

    pub fn do_get_available_manifests(pool_id: Option<PoolIdOf<T>>) -> DispatchResult {
        // Auxiliar structures
        let mut manifests_result = Vec::new();

        // Validate the manifests that match the filters
        for item in Manifests::<T>::iter() {
            let mut meet_requirements = true;

            //Checks that there is still available storers to be added
            if let Some(_) = Self::get_next_available_uploader_index(item.2.users_data.to_vec()) {
                //Checks for the pool_id
                if let Some(pool_id_value) = pool_id {
                    if pool_id_value != item.0 {
                        meet_requirements = false;
                    }
                }

                // if everything match, add them to the vector of results
                if meet_requirements {
                    manifests_result.push(ManifestAvailable {
                        pool_id: item.0,
                        manifest_metadata: item.2.manifest_metadata,
                        replication_factor: Self::get_added_replication_factor(
                            item.2.users_data.to_vec(),
                        ),
                    });
                }
            }
        }
        Self::deposit_event(Event::GetAvailableManifests {
            manifests: manifests_result,
        });
        Ok(())
    }

    pub fn do_get_manifest_storer_data(
        pool_id: Option<PoolIdOf<T>>,
        storer: Option<T::AccountId>,
    ) -> DispatchResult {
        // Auxiliar structures
        let mut manifests_result = Vec::new();

        // Validate the manifests storer data that match the filters
        for item in ManifestsStorerData::<T>::iter() {
            let mut meet_requirements = true;

            //Checks for the pool_id
            if let Some(pool_id_value) = pool_id {
                if pool_id_value != item.0 .0 {
                    meet_requirements = false;
                }
            }

            //Checks for the storer account
            if let Some(ref storer_value) = storer {
                if storer_value.clone() != item.0 .1.clone() {
                    meet_requirements = false;
                }
            }

            // if everything match, add them to the vector of results
            if meet_requirements {
                manifests_result.push(StorerData {
                    pool_id: item.0 .0,
                    cid: item.0 .2,
                    account: item.0 .1.clone(),
                    manifest_data: ManifestStorageData {
                        active_cycles: item.1.active_cycles,
                        missed_cycles: item.1.missed_cycles,
                        active_days: item.1.active_days,
                        challenge_state: item.1.challenge_state,
                    },
                });
            }
        }
        Self::deposit_event(Event::GetManifestsStorerData {
            manifests: manifests_result,
        });
        Ok(())
    }

    pub fn pick_random_account_cid_pair(
        challenger: &T::AccountId,
    ) -> (Option<T::AccountId>, Option<CIDOf<T>>) {
        let mut account = challenger.clone();

        while account == challenger.clone() {
            let max_value = ManifestsStorerData::<T>::iter().count();
            let random_value = <pallet::Pallet<T> as MaxRange>::random(max_value as u64);

            if let Some(item) = ManifestsStorerData::<T>::iter().nth(random_value as usize) {
                account = item.0 .1;
                let cid = Some(item.0 .2);

                if account != challenger.clone() {
                    return (Some(account), cid);
                }
            } else {
                return (None, None);
            }
        }

        return (None, None);
    }

    pub fn do_generate_challenge(challenger: &T::AccountId) -> DispatchResult {
        // Get the random pair of account - cid
        let pair = Self::pick_random_account_cid_pair(challenger);

        // Validations made to verify some parameters
        ensure!(pair.0.is_some(), Error::<T>::ErrorPickingAccountToChallenge);
        ensure!(pair.1.is_some(), Error::<T>::ErrorPickingCIDToChallenge);

        // Insert the value in the challenge storage as an Open State
        ChallengeRequests::<T>::insert(
            pair.0.clone().unwrap(),
            pair.1.clone().unwrap(),
            Challenge {
                challenger: challenger.clone(),
                challenge_state: ChallengeState::Open,
            },
        );

        Self::deposit_event(Event::Challenge {
            challenger: challenger.clone(),
            challenged: pair.0.unwrap(),
            cid: pair.1.unwrap().to_vec(),
            state: ChallengeState::Open,
        });
        Ok(())
    }

    pub fn do_verify_challenge(
        challenged: &T::AccountId,
        cids: Vec<CIDOf<T>>,
        pool_id: PoolIdOf<T>,
        class_id: ClassId,
        asset_id: AssetId,
    ) -> DispatchResult {
        // Validations made to verify some parameters given
        ensure!(
            T::Pool::is_member(challenged.clone(), pool_id),
            Error::<T>::AccountNotInPool
        );

        // Auxiliar structures
        let mut successful_cids = Vec::new();
        let mut failed_cids = Vec::new();

        // Validates if the challenge is successful given the cids given and the open challenge cid
        for item in ChallengeRequests::<T>::iter() {
            // Checks if the user is the challenged account
            if item.0.clone() == challenged.clone() {
                let cid = item.1.clone();
                //Checks that the pool_id + account  + cid exists in the manifests storer data
                let mut data =
                    Self::manifests_storage_data((pool_id, challenged.clone(), item.1.clone()))
                        .ok_or(Error::<T>::ManifestStorerDataNotFound)?;
                let state;
                // Verifies if the cids provided contain the cid of the challenge to determine if it's a successful or failed challenge
                if cids.contains(&item.1) {
                    state = ChallengeState::Successful;
                    successful_cids.push(item.1.to_vec());
                    // Mint the challenge tokens corresponding to the cid file size
                    let mut amount = 0;
                    if let Some(file_check) = Manifests::<T>::get(pool_id, cid.clone()) {
                        if let Some(file_size) = file_check.size {
                            amount = file_size * 10;
                        } else {
                            amount = 10;
                        }
                    }
                    let _value = sugarfunge_asset::Pallet::<T>::do_mint(
                        challenged,
                        challenged,
                        class_id.into(),
                        asset_id.into(),
                        amount as u128,
                    );
                } else {
                    state = ChallengeState::Failed;
                    failed_cids.push(item.1.to_vec())
                }

                // Once the mint happens, the challenge is removed
                ChallengeRequests::<T>::remove(challenged, cid.clone());

                // The latest state of the cid is stored in the manifests storer data storage
                data.challenge_state = state;
                ManifestsStorerData::<T>::set((pool_id, challenged, cid.clone()), Some(data));
            }
        }
        Self::deposit_event(Event::VerifiedChallenges {
            challenged: challenged.clone(),
            successful: successful_cids,
            failed: failed_cids,
        });
        Ok(())
    }

    // pub fn mint_challenge_tokens_and_update(
    //     who: &T::AccountId,
    //     cid: CIDOf<T>,
    //     pool: PoolIdOf<T>,
    //     class_id: ClassId,
    //     asset_id: AssetId,
    //     mut data: ManifestStorageData,
    //     state: ChallengeState,
    // ) {
    //     let mut amount = 0;
    //     if let Some(file_check) = Manifests::<T>::get(pool, cid.clone()) {
    //         if let Some(file_size) = file_check.size {
    //             amount = file_size * 10;
    //         } else {
    //             amount = 10;
    //         }
    //     }
    //     sugarfunge_asset::Pallet::<T>::do_mint(
    //         who,
    //         who,
    //         class_id.into(),
    //         asset_id.into(),
    //         amount as u128,
    //     );

    //     // Once the mint happens, the challenge is removed
    //     ChallengeRequests::<T>::remove(who, cid.clone());

    //     // The latest state of the cid is stored in the manifests storer data storage
    //     data.challenge_state = state;
    //     ManifestsStorerData::<T>::set((pool, who, cid.clone()), Some(data));
    // }

    pub fn get_network_size() -> f64 {
        // Returns the value of the network size value
        return NetworkSize::<T>::get() as f64;
    }

    pub fn do_mint_labor_tokens(
        account: &T::AccountId,
        class_id: ClassId,
        asset_id: AssetId,
        amount: MintBalance,
    ) -> DispatchResult {
        // Auxiliar structures
        let mut mining_rewards: f64 = 0.0;
        let mut storage_rewards: f64 = 0.0;

        // Iterate over the manifest storer data to update the labor tokens to be minted
        for manifest in ManifestsStorerData::<T>::iter() {
            let mut file_participation = 0.0;

            //Checks for the account
            if account.clone() == manifest.0 .1.clone() {
                // Store the data to be updated later in the manifests storer data
                let mut updated_data = ManifestsStorerData::<T>::get((
                    manifest.0 .0,
                    manifest.0 .1.clone(),
                    manifest.0 .2.clone(),
                ))
                .unwrap();

                // Checks that the manifest has the file_size available to calculate the file participation in the network
                if let Some(file_check) = Manifests::<T>::get(manifest.0 .0, manifest.0 .2.clone())
                {
                    if let Some(file_size) = file_check.size {
                        file_participation = file_size as f64 / Self::get_network_size();
                    }
                }
                // If the state of the manifest is successful
                if manifest.1.challenge_state == ChallengeState::Successful {
                    // When the active cycles reached {NUMBER_CYCLES_TO_ADVANCE} which is equal to 1 day, the manifest active days are increased and the rewards are calculated
                    if manifest.1.active_cycles >= NUMBER_CYCLES_TO_ADVANCE {
                        let active_days = manifest.1.active_days + 1;

                        // The calculation of the storage rewards
                        storage_rewards += (1 as f64
                            / (1 as f64 + exp(-0.1 * (active_days - 45) as f64)))
                            * DAILY_TOKENS_STORAGE
                            * file_participation;

                        // The calculation of the mining rewards
                        mining_rewards += DAILY_TOKENS_MINING as f64 * file_participation;

                        updated_data.active_days += 1;
                        updated_data.active_cycles = 0;
                    } else {
                        updated_data.active_cycles += 1;
                    }
                } else {
                    // If the verification of the IPFS File failed {NUMBER_CYCLES_TO_RESET} times, the active_days are reset to 0
                    if manifest.1.missed_cycles >= NUMBER_CYCLES_TO_RESET {
                        updated_data.missed_cycles = 0;
                        updated_data.active_days = 0;
                    } else {
                        // If the times failed are lower, the missed cycles are increased
                        updated_data.missed_cycles += 1;
                    }
                }
                // Update the variables values for the next cycle
                ManifestsStorerData::<T>::try_mutate(
                    (manifest.0 .0, account.clone(), manifest.0 .2.clone()),
                    |value| -> DispatchResult {
                        if let Some(manifest) = value {
                            manifest.active_cycles = updated_data.active_cycles;
                            manifest.missed_cycles = updated_data.missed_cycles;
                            manifest.active_days = updated_data.active_days;
                        }
                        Ok(())
                    },
                )?;
            }
        }

        // Calculate the total amount of rewards for the cycle
        let calculated_amount = mining_rewards + storage_rewards;

        // TO DO: Here would be the call to mint the labor tokens
        let _value = sugarfunge_asset::Pallet::<T>::do_mint(
            account,
            account,
            class_id.into(),
            asset_id.into(),
            amount,
        );

        Self::deposit_event(Event::MintedLaborTokens {
            account: account.clone(),
            class_id,
            asset_id,
            amount,
            calculated_amount: calculated_amount as u64,
        });
        Ok(())
    }

    pub fn do_update_size(
        storer: &T::AccountId,
        pool_id: PoolIdOf<T>,
        cid: CIDOf<T>,
        size: FileSize,
    ) -> DispatchResult {
        // Validations made to verify some parameters given
        ensure!(
            T::Pool::is_member(storer.clone(), pool_id),
            Error::<T>::AccountNotInPool
        );
        ensure!(
            Manifests::<T>::try_get(pool_id, cid.clone()).is_ok(),
            Error::<T>::ManifestNotFound
        );

        // Updated the file_size of a manifest
        Manifests::<T>::try_mutate(pool_id, cid.clone(), |value| -> DispatchResult {
            if let Some(manifest) = value {
                // Checks if there is a uploader that has the storer on chain
                if let Some(_) =
                    Self::get_uploader_index_given_storer(manifest.users_data.to_vec(), &storer)
                {
                    manifest.size = Some(size);
                    //Update the network size for the total amount of people that had that manifest before the file_size was given
                    NetworkSize::<T>::mutate(|total_value| {
                        *total_value +=
                            size * Self::get_total_storers(manifest.users_data.to_vec());
                    });
                }
            }
            Ok(())
        })?;

        Self::deposit_event(Event::UpdateFileSizeOutput {
            account: storer.clone(),
            pool_id: pool_id.clone(),
            cid: cid.to_vec(),
            size,
        });

        Ok(())
    }

    pub fn do_update_sizes(
        storer: &T::AccountId,
        pool_id: PoolIdOf<T>,
        cids: Vec<CIDOf<T>>,
        sizes: Vec<FileSize>,
    ) -> DispatchResult {
        // The cycle to execute multiple times the update_size for the manifests file_size
        let n = cids.len();
        for i in 0..n {
            let cid = cids[i].to_owned();
            let size = sizes[i].to_owned();
            Self::do_update_size(storer, pool_id, cid, size)?;
        }

        Self::deposit_event(Event::UpdateFileSizesOutput {
            account: storer.clone(),
            cids: Self::transform_cid_to_vec(cids),
            pool_id: pool_id.clone(),
            sizes: Self::transform_filesize_to_u64(sizes),
        });
        Ok(())
    }

    // function to get the next available uploader index for when a user is going to become a storer
    pub fn get_next_available_uploader_index(
        data: Vec<UploaderData<T::AccountId>>,
    ) -> Option<usize> {
        return data
            .iter()
            .position(|x| x.replication_factor - x.storers.len() as u16 > 0);
    }

    // Get the final replication factor if there is multiple uploaders in a manifest
    pub fn get_added_replication_factor(data: Vec<UploaderData<T::AccountId>>) -> u16 {
        let mut result = 0;
        for user_data in data {
            result += user_data.replication_factor - user_data.storers.len() as u16;
        }
        return result.into();
    }

    // Get the uploader index given the uploader account
    pub fn get_uploader_index(
        data: Vec<UploaderData<T::AccountId>>,
        account: T::AccountId,
    ) -> Option<usize> {
        return data.iter().position(|x| x.uploader == account);
    }

    // Verifies if the account given is an uploader
    pub fn verify_account_is_uploader(
        data: Vec<UploaderData<T::AccountId>>,
        account: T::AccountId,
    ) -> bool {
        return data.iter().position(|x| x.uploader == account).is_some();
    }

    // Get the uploader index given an storer account
    pub fn get_uploader_index_given_storer(
        data: Vec<UploaderData<T::AccountId>>,
        account: &T::AccountId,
    ) -> Option<usize> {
        return data.iter().position(|x| x.storers.contains(account));
    }

    // Verify that the account is a storer
    pub fn verify_account_is_storer(
        data: Vec<UploaderData<T::AccountId>>,
        account: &T::AccountId,
    ) -> bool {
        return data
            .iter()
            .position(|x| x.storers.contains(account))
            .is_some();
    }

    // Get the storer index given the storer account and the uploader index
    pub fn get_storer_index(
        data: Vec<UploaderData<T::AccountId>>,
        uploader_index: usize,
        account: T::AccountId,
    ) -> Option<usize> {
        return data[uploader_index]
            .storers
            .iter()
            .position(|x| *x == account.clone());
    }

    // Get the total amount of storers given the uploaders vector
    pub fn get_total_storers(data: Vec<UploaderData<T::AccountId>>) -> u64 {
        let mut total_storers = 0;

        for uploader in data {
            total_storers += uploader.storers.len() as u64;
        }

        return total_storers;
    }

    // Some functions to transform some values into another inside a Vec
    fn transform_manifest_to_vec(in_vec: Vec<ManifestMetadataOf<T>>) -> Vec<Vec<u8>> {
        in_vec
            .into_iter()
            .map(|manifest| manifest.to_vec())
            .collect()
    }

    fn transform_cid_to_vec(in_vec: Vec<CIDOf<T>>) -> Vec<Vec<u8>> {
        in_vec.into_iter().map(|cid| cid.to_vec()).collect()
    }

    fn transform_filesize_to_u64(in_vec: Vec<FileSize>) -> Vec<u64> {
        in_vec.into_iter().map(|size| size).collect()
    }
}
