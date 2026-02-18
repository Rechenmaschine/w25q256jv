mod embedded_storage;

#[cfg(feature = "littlefs2")]
mod littlefs;

#[cfg(feature = "littlefs2")]
pub use littlefs::LittlefsAdapter;
