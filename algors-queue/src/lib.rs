#![cfg_attr(not(feature = "std"), no_std)]

pub mod mpmc;
pub mod slot;
pub mod spsc;
pub mod wait;

pub use mpmc::{MpmcConsumer, MpmcInner, MpmcProducer};
pub use spsc::{SpscConsumer, SpscInner, SpscProducer, new_spsc};
