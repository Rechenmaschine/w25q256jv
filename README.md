# W25Q256JV Flash Driver

[![CI](https://github.com/Rechenmaschine/w25q256jv/actions/workflows/rust-ci.yml/badge.svg?branch=master)](https://github.com/Rechenmaschine/w25q256jv/actions/workflows/rust-ci.yml)

> `no_std` driver for the Winbond W25Q256JV NOR flash chip.

- Async API using `embedded-hal-async`
- Blocking API using `embedded-hal`
- Implements `embedded-storage-async` and `embedded-storage`
- Optional `littlefs2` adapter via `LittlefsAdapter`
- Optional `defmt` support

This crate is based on the [w25q32jv](https://crates.io/crates/w25q32jv) driver by tweedegolf.

## Usage Examples

### Blocking API

```rust
use w25q256jv::W25q256jv;

let mut flash = W25q256jv::new(spi, hold, wp)?;

flash.blocking_erase_sector(0)?;
flash.blocking_write(0, b"hello")?;

let mut buf = [0u8; 5];
flash.blocking_read(0, &mut buf)?;
```

### Async API

```rust
use w25q256jv::{W25q256jv, SECTOR_SIZE};

let mut flash = W25q256jv::new(spi, hold, wp)?;

flash.erase_range(0, SECTOR_SIZE).await?;
flash.write(0, b"hello").await?;

let mut buf = [0u8; 5];
flash.read(0, &mut buf).await?;
```

### littlefs2 Adapter

Enable the `littlefs2` feature and wrap the flash driver in `LittlefsAdapter`:

```rust
use typenum::{U16, U256};
use w25q256jv::{LittlefsAdapter, W25q256jv};

type FlashStorage<'a, SPI, HOLD, WP> = LittlefsAdapter<'a, SPI, HOLD, WP, U256, U16>;

let mut flash = W25q256jv::new(spi, hold, wp)?;
let mut storage = FlashStorage::new(&mut flash);
```

## Cargo Features

- `defmt`: enables `defmt::Format` for `Error`
- `littlefs2`: enables `LittlefsAdapter` and `littlefs2` integration
- `readback-check`: verifies writes and erases by reading back data (slower)

## License

MIT
