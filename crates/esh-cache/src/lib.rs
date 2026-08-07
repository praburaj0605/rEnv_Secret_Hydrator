//! Cache backends for Env-Secret-Hydrator.

#[cfg(feature = "file")]
mod file;
#[cfg(feature = "memory")]
mod memory;
#[cfg(feature = "redis-cache")]
mod redis_cache;

#[cfg(feature = "file")]
pub use file::FileCache;
#[cfg(feature = "memory")]
pub use memory::MemoryCache;
#[cfg(feature = "redis-cache")]
pub use redis_cache::RedisCache;
