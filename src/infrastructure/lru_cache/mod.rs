//! LRU cache implementation.

mod cache_impl;
mod state;
#[cfg(test)]
mod tests;

pub use cache_impl::LruCacheImpl;
