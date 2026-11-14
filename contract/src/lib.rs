//! $NEPA "Up NEPA Streak": a NEP-141 meme token whose rules live in the contract.
//! Each "day" (length set at launch) holders can check in once. If the community
//! reaches the target, everyone who checked in earns a mint and the streak grows.
//! If not, a slice of the treasury (the contract's own balance) is burned and the
//! streak resets. All parameters are fixed at launch. There is no admin setter.

use near_contract_standards::fungible_token::events::{FtBurn, FtMint};
use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::borsh::BorshSerialize;
use near_sdk::collections::LazyOption;
use near_sdk::json_types::U128;
use near_sdk::store::LookupMap;
use near_sdk::{env, near, require, AccountId, BorshStorageKey, PanicOnDefault, PromiseOrValue};

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum Key {
    Ft,
    Meta,
    LastCheckin,
    Claimed,
    DaySuccess,
}

#[near(serializers=[json])]
pub struct State {
    pub target: u64,
    pub mint_amount: U128,
    pub burn_bps: u16,
    pub epoch_secs: u64,
    pub streak: u64,
    pub best_streak: u64,
    pub today_day: u64,
    pub checkins_today: u64,
    pub needs_settle: bool,
}

#[derive(PanicOnDefault)]
#[near(contract_state)]
pub struct Contract {
    token: FungibleToken,
    metadata: LazyOption<FungibleTokenMetadata>,
    target: u64,
    mint_amount: u128,
    burn_bps: u16,
    epoch_ns: u64,
    current_day: u64,
    today_count: u64,
    streak: u64,
    best_streak: u64,
    last_checkin: LookupMap<AccountId, u64>,
    claimed: LookupMap<AccountId, u64>,
    day_success: LookupMap<u64, bool>,
}

#[near]
impl Contract {
    /// treasury_bps: share of supply held by the contract itself (burn source), out of 10000.
    #[init]
    pub fn new(
        owner_id: AccountId,
        total_supply: U128,
        treasury_bps: u16,
        target: u64,
        mint_amount: U128,
        burn_bps: u16,
        epoch_secs: u64,
    ) -> Self {
        require!(!env::state_exists(), "Already initialized");
        require!(treasury_bps <= 10_000 && burn_bps <= 10_000, "bps must be <= 10000");
        require!(target > 0 && epoch_secs > 0, "target and epoch_secs must be > 0");
        let metadata = FungibleTokenMetadata {
            spec: FT_METADATA_SPEC.to_string(),
            name: "NEPA Streak".to_string(),
            symbol: "NEPA".to_string(),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 8,
        };
        let epoch_ns = epoch_secs * 1_000_000_000;
        let mut this = Self {
            token: FungibleToken::new(Key::Ft),
            metadata: LazyOption::new(Key::Meta, Some(&metadata)),
            target,
            mint_amount: mint_amount.0,
            burn_bps,
            epoch_ns,
            current_day: env::block_timestamp() / epoch_ns,
            today_count: 0,
            streak: 0,
            best_streak: 0,
            last_checkin: LookupMap::new(Key::LastCheckin),
            claimed: LookupMap::new(Key::Claimed),
            day_success: LookupMap::new(Key::DaySuccess),
        };
        let me = env::current_account_id();
        let treasury = total_supply.0 * treasury_bps as u128 / 10_000;
        this.token.internal_register_account(&owner_id);
        this.token.internal_register_account(&me);
        this.token.internal_deposit(&owner_id, total_supply.0 - treasury);
        this.token.internal_deposit(&me, treasury);
        FtMint { owner_id: &owner_id, amount: U128(total_supply.0 - treasury), memo: Some("Launch") }.emit();
        FtMint { owner_id: &me, amount: U128(treasury), memo: Some("Treasury") }.emit();
        this
    }

    /// Once per day per holder. The caller must have registered with storage_deposit.
    pub fn check_in(&mut self) {
        let a = env::predecessor_account_id();
        require!(
            self.token.accounts.contains_key(&a),
            "Register first: call storage_deposit"
        );
        self.roll();
        self.pay_pending(&a);
        require!(
            self.last_checkin.get(&a).copied() != Some(self.current_day),
            "Already checked in today"
        );
        self.last_checkin.insert(a, self.current_day);
        self.today_count += 1;
    }

    /// Anyone can close a finished day. This applies the mint or burn rule.
    pub fn settle(&mut self) {
        self.roll();
    }

    /// Collect the reward for your last successful check-in.
    pub fn claim(&mut self) {
        let a = env::predecessor_account_id();
        self.roll();
        self.pay_pending(&a);
    }

    pub fn get_state(&self) -> State {
        let today = env::block_timestamp() / self.epoch_ns;
        let stale = today > self.current_day;
        State {
            target: self.target,
            mint_amount: U128(self.mint_amount),
            burn_bps: self.burn_bps,
            epoch_secs: self.epoch_ns / 1_000_000_000,
            streak: self.streak,
            best_streak: self.best_streak,
            today_day: today,
            checkins_today: if stale { 0 } else { self.today_count },
            needs_settle: stale,
        }
    }

    fn roll(&mut self) {
        let today = env::block_timestamp() / self.epoch_ns;
        if today <= self.current_day {
            return;
        }
        if self.today_count >= self.target {
            self.streak += 1;
            if self.streak > self.best_streak {
                self.best_streak = self.streak;
            }
            self.day_success.insert(self.current_day, true);
        } else {
            self.streak = 0;
            self.burn_slice();
        }
        if today > self.current_day + 1 {
            // whole days passed with nobody settling: the streak is broken
            self.streak = 0;
            self.burn_slice();
        }
        self.current_day = today;
        self.today_count = 0;
    }

    fn burn_slice(&mut self) {
        let me = env::current_account_id();
        let bal = self.token.internal_unwrap_balance_of(&me);
        let amt = bal * self.burn_bps as u128 / 10_000;
        if amt > 0 {
            self.token.internal_withdraw(&me, amt);
            self.token.total_supply -= amt;
            FtBurn { owner_id: &me, amount: U128(amt), memo: Some("Streak missed") }.emit();
        }
    }

    fn pay_pending(&mut self, a: &AccountId) {
        if let Some(day) = self.last_checkin.get(a).copied() {
            let won = self.day_success.get(&day).copied() == Some(true);
            if won && self.claimed.get(a).copied() != Some(day) {
                self.claimed.insert(a.clone(), day);
                self.token.internal_deposit(a, self.mint_amount);
                FtMint { owner_id: a, amount: U128(self.mint_amount), memo: Some("Up NEPA!") }.emit();
            }
        }
    }
}

near_contract_standards::impl_fungible_token_core!(Contract, token);

#[near]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        self.metadata.get().unwrap()
    }
}

use near_contract_standards::storage_management::{StorageBalance, StorageBalanceBounds, StorageManagement};

#[near]
impl Contract {
    #[payable]
    pub fn storage_deposit(&mut self, account_id: Option<AccountId>, registration_only: Option<bool>) -> StorageBalance {
        self.token.storage_deposit(account_id, registration_only)
    }
    pub fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.token.storage_balance_of(account_id)
    }
    pub fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        self.token.storage_balance_bounds()
    }
}
