use std::{marker::PhantomData, ops::*};

use serde::{Deserialize, Serialize};

use super::prelude::*;
use std::fmt::{Debug, Display};

/// A data structure that keeps track of a staker's staked coins and the epochs for different
/// capabilities.
#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Stake<Address, Coins, Epoch, Power>
where
    Address: Default,
    Epoch: Default,
{
    /// An amount of staked coins.
    pub coins: Coins,
    /// The average epoch used to derive coin age for different capabilities.
    pub epochs: CapabilityMap<Epoch>,
    // These two phantom fields are here just for the sake of specifying generics.
    phantom_address: PhantomData<Address>,
    phantom_power: PhantomData<Power>,
}

impl<Address, Coins, Epoch, Power> Stake<Address, Coins, Epoch, Power>
where
    Address: Default + Debug + Display + Sync + Send,
    Coins: Copy
        + From<u64>
        + PartialOrd
        + num_traits::Zero
        + Add<Output = Coins>
        + Sub<Output = Coins>
        + Mul
        + Mul<Epoch, Output = Power>
        + Debug
        + Display
        + Send
        + Sync,
    Epoch: Copy
        + Default
        + num_traits::Saturating
        + Sub<Output = Epoch>
        + From<u32>
        + Debug
        + Display
        + Sync
        + Send,
    Power: Add<Output = Power> + Div<Output = Power>,
    u64: From<Coins> + From<Power>,
{
    /// Increase the amount of coins staked by a certain staker.
    ///
    /// When adding stake:
    /// - Amounts are added together.
    /// - Epochs are weight-averaged, using the amounts as the weight.
    ///
    /// This type of averaging makes the entry equivalent to an unbounded record of all stake
    /// additions and removals, without the overhead in memory and computation.
    pub fn add_stake(
        &mut self,
        coins: Coins,
        epoch: Epoch,
        minimum_stakeable: Option<Coins>,
    ) -> StakesResult<Coins, Address, Coins, Epoch> {
        // Make sure that the amount to be staked is equal or greater than the minimum
        let minimum = minimum_stakeable.unwrap_or(Coins::from(MINIMUM_STAKEABLE_AMOUNT_WITS));
        if coins < minimum {
            Err(StakesError::AmountIsBelowMinimum {
                amount: coins,
                minimum,
            })?;
        }

        let coins_before = self.coins;
        let epoch_before = self.epochs.get(Capability::Mining);

        let product_before = coins_before * epoch_before;
        let product_added = coins * epoch;

        let coins_after = coins_before + coins;
        #[allow(clippy::cast_possible_truncation)]
        let epoch_after = Epoch::from(
            (u64::from(product_before + product_added) / u64::from(coins_after)) as u32,
        );

        self.coins = coins_after;
        self.epochs.update_all(epoch_after);

        Ok(coins_after)
    }

    /// Construct a Stake entry from a number of coins and a capability map. This is only useful for
    /// tests.
    #[cfg(test)]
    pub fn from_parts(coins: Coins, epochs: CapabilityMap<Epoch>) -> Self {
        Self {
            coins,
            epochs,
            phantom_address: Default::default(),
            phantom_power: Default::default(),
        }
    }

    /// Derives the power of an identity in the network on a certain epoch from an entry. Most
    /// normally, the epoch is the current epoch.
    pub fn power(&self, capability: Capability, current_epoch: Epoch) -> Power {
        self.coins * (current_epoch.saturating_sub(self.epochs.get(capability)))
    }

    /// Remove a certain amount of staked coins.
    pub fn remove_stake(
        &mut self,
        coins: Coins,
        minimum_stakeable: Option<Coins>,
    ) -> StakesResult<Coins, Address, Coins, Epoch> {
        let coins_after = self.coins.sub(coins);

        if coins_after > Coins::zero() {
            let minimum = minimum_stakeable.unwrap_or(Coins::from(MINIMUM_STAKEABLE_AMOUNT_WITS));

            if coins_after < minimum {
                Err(StakesError::AmountIsBelowMinimum {
                    amount: coins_after,
                    minimum,
                })?;
            }
        }

        self.coins = coins_after;

        Ok(self.coins)
    }

    /// Set the epoch for a certain capability. Most normally, the epoch is the current epoch.
    pub fn reset_age(&mut self, capability: Capability, current_epoch: Epoch) {
        self.epochs.update(capability, current_epoch);
    }
}
