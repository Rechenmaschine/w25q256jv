#![no_std]

use core::fmt::Debug;
use embedded_hal::digital::{OutputPin, PinState};
use embedded_storage::nor_flash::{ErrorType, NorFlashError, NorFlashErrorKind};

mod external_impls;
#[cfg(feature = "littlefs2")]
pub use external_impls::LittlefsAdapter;
pub mod w25q256jv;

pub const PAGE_SIZE: u32 = 256;
pub const N_PAGES: u32 = 131_072;
pub const CAPACITY: u32 = PAGE_SIZE * N_PAGES;
pub const SECTOR_SIZE: u32 = PAGE_SIZE * 16;
pub const N_SECTORS: u32 = N_PAGES / 16;
pub const BLOCK_32K_SIZE: u32 = SECTOR_SIZE * 8;
pub const N_BLOCKS_32K: u32 = N_SECTORS / 8;
pub const BLOCK_64K_SIZE: u32 = BLOCK_32K_SIZE * 2;
pub const N_BLOCKS_64K: u32 = N_BLOCKS_32K / 2;

/// Low level driver for the W25q256jv flash memory chip.
pub struct W25q256jv<SPI, HOLD, WP> {
    spi: SPI,
    hold: HOLD,
    wp: WP,
}

impl<SPI, HOLD, WP> W25q256jv<SPI, HOLD, WP> {
    /// Get the capacity of the flash chip in bytes.
    pub fn capacity() -> usize {
        CAPACITY as usize
    }
}

impl<SPI, S: Debug, P: Debug, HOLD, WP> W25q256jv<SPI, HOLD, WP>
where
    SPI: embedded_hal::spi::ErrorType<Error = S>,
    HOLD: OutputPin<Error = P>,
    WP: OutputPin<Error = P>,
{
    pub fn new(spi: SPI, hold: HOLD, wp: WP) -> Result<Self, Error<S, P>> {
        let mut flash = Self { spi, hold, wp };

        flash.hold.set_high().map_err(Error::PinError)?;
        flash.wp.set_high().map_err(Error::PinError)?;

        Ok(flash)
    }

    /// Set the hold pin state.
    ///
    /// The driver doesn't do anything with this pin. When using the chip, make sure the hold pin is not asserted.
    /// By default, this means the pin needs to be high (true).
    ///
    /// This function sets the pin directly and can cause the chip to not work.
    pub fn set_hold(&mut self, value: PinState) -> Result<(), Error<S, P>> {
        self.hold.set_state(value).map_err(Error::PinError)?;
        Ok(())
    }

    /// Set the write protect pin state.
    ///
    /// The driver doesn't do anything with this pin. When using the chip, make sure the hold pin is not asserted.
    /// By default, this means the pin needs to be high (true).
    ///
    /// This function sets the pin directly and can cause the chip to not work.
    pub fn set_wp(&mut self, value: PinState) -> Result<(), Error<S, P>> {
        self.wp.set_state(value).map_err(Error::PinError)?;
        Ok(())
    }

    /// Releases the SPI, HOLD and WP pins from the driver.
    pub fn release(self) -> (SPI, HOLD, WP) {
        (self.spi, self.hold, self.wp)
    }
}

impl<SPI, S: Debug, P: Debug, HOLD, WP> ErrorType for W25q256jv<SPI, HOLD, WP>
where
    HOLD: OutputPin<Error = P>,
    P: Debug,
    S: Debug,
    SPI: embedded_hal::spi::ErrorType<Error = S>,
    WP: OutputPin<Error = P>,
{
    type Error = Error<S, P>;
}

/// Custom error type for the various errors that can be thrown by W25q256jv.
/// Can be converted into a NorFlashError.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum Error<S: Debug, P: Debug> {
    SpiError(S),
    PinError(P),
    NotAligned,
    OutOfBounds,
    WriteEnableFail,
    ReadbackFail,
}

impl<S: Debug, P: Debug> NorFlashError for Error<S, P> {
    fn kind(&self) -> NorFlashErrorKind {
        match self {
            Error::NotAligned => NorFlashErrorKind::NotAligned,
            Error::OutOfBounds => NorFlashErrorKind::OutOfBounds,
            _ => NorFlashErrorKind::Other,
        }
    }
}

/// Easily readable representation of the command bytes used by the flash chip.
/// 4-byte addressing mode
#[repr(u8)]
enum Command {
    WriteEnable = 0x06,
    // WriteDisable = 0x04,
    // ReadUniqueId = 0x4B,
    ReadDataWith4ByteAddress = 0x13,
    PageProgramWith4ByteAddress = 0x12,
    SectorErase4KBWith4ByteAddress = 0x21,
    BlockErase32KB = 0x52, // can be used in both 3-byte and 4-byte addressing modes
    BlockErase64KBWith4ByteAddress = 0xDC,
    ChipErase = 0xC7, // alternatively 0x60 can be used
    ReadStatusRegister1 = 0x05,
    EnableReset = 0x66,
    ResetDevice = 0x99,
    Enter4ByteAddressMode = 0xB7,
    Exit4ByteAddressMode = 0xE9,
}

fn command_and_address(command: u8, address: u32) -> [u8; 5] {
    let addr_bytes = address.to_be_bytes();
    [
        command,
        addr_bytes[0],
        addr_bytes[1],
        addr_bytes[2],
        addr_bytes[3],
    ]
}
