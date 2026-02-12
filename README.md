# W25Q256JV Flash driver

This is a generic driver for the W25Q256JV flash chip from Winbond. 
It is based on the [W25Q32JV](https://crates.io/crates/w25q32jv) driver by [tweedegolf](https://github.com/tweedegolf).

It supports:
- Async SPI using `embedded-hal-async`
- Async `embedded-storage-async`
- Blocking SPI using `embedded-hal`
- Blocking `embedded-storage`
- Optional `littlefs2` storage adapter (`W25q256jvLfsStorage`) behind the `littlefs2` feature

Blocking API methods are prefixed with `blocking_` (for example: `blocking_read`, `blocking_write`, `blocking_erase_sector`).

For littlefs2, use the adapter wrapper instead of implementing `Storage` directly on the
driver, so cache/lookahead sizes are user-configurable at the type level:

`W25q256jvLfsStorage<'a, SPI, HOLD, WP, CacheSize, LookaheadSize>`

Defmt is also supported through the `defmt` feature.
