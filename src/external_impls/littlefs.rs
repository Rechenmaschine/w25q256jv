use crate::{Error, W25q256jv, N_SECTORS, PAGE_SIZE, SECTOR_SIZE};
use core::fmt::Debug;
use core::marker::PhantomData;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiDevice;
#[allow(deprecated)]
use generic_array::ArrayLength;
use littlefs2::driver::Storage;
use littlefs2::io::{Error as LittleFsError, Result as LittleFsResult};

fn to_littlefs_error<S: Debug, P: Debug>(error: Error<S, P>) -> LittleFsError {
    match error {
        Error::NotAligned | Error::OutOfBounds => LittleFsError::INVALID,
        _ => LittleFsError::IO,
    }
}

/// littlefs2 storage adapter for [`W25q256jv`] with configurable cache/lookahead sizes.
pub struct W25q256jvLfsStorage<'a, SPI, HOLD, WP, CacheSize, LookaheadSize> {
    flash: &'a mut W25q256jv<SPI, HOLD, WP>,
    _marker: PhantomData<(CacheSize, LookaheadSize)>,
}

impl<'a, SPI, HOLD, WP, CacheSize, LookaheadSize>
    W25q256jvLfsStorage<'a, SPI, HOLD, WP, CacheSize, LookaheadSize>
{
    pub fn new(flash: &'a mut W25q256jv<SPI, HOLD, WP>) -> Self {
        Self {
            flash,
            _marker: PhantomData,
        }
    }

    pub fn into_inner(self) -> &'a mut W25q256jv<SPI, HOLD, WP> {
        self.flash
    }
}

#[allow(deprecated)]
impl<'a, SPI, S: Debug, P: Debug, HOLD, WP, CacheSize, LookaheadSize> Storage
    for W25q256jvLfsStorage<'a, SPI, HOLD, WP, CacheSize, LookaheadSize>
where
    SPI: SpiDevice<Error = S>,
    HOLD: OutputPin<Error = P>,
    WP: OutputPin<Error = P>,
    CacheSize: ArrayLength<u8>,
    LookaheadSize: ArrayLength<u64>,
    S: Debug,
    P: Debug,
{
    const READ_SIZE: usize = 1;
    const WRITE_SIZE: usize = PAGE_SIZE as usize;
    const BLOCK_SIZE: usize = SECTOR_SIZE as usize;
    const BLOCK_COUNT: usize = N_SECTORS as usize;
    const BLOCK_CYCLES: isize = 100_000;

    type CACHE_SIZE = CacheSize;
    type LOOKAHEAD_SIZE = LookaheadSize;

    fn read(&mut self, off: usize, buf: &mut [u8]) -> LittleFsResult<usize> {
        let off = u32::try_from(off).map_err(|_| LittleFsError::INVALID)?;
        self.flash
            .blocking_read(off, buf)
            .map_err(to_littlefs_error)?;
        Ok(buf.len())
    }

    fn write(&mut self, off: usize, data: &[u8]) -> LittleFsResult<usize> {
        let off = u32::try_from(off).map_err(|_| LittleFsError::INVALID)?;
        self.flash
            .blocking_write(off, data)
            .map_err(to_littlefs_error)?;
        Ok(data.len())
    }

    fn erase(&mut self, off: usize, len: usize) -> LittleFsResult<usize> {
        if len == 0 {
            return Ok(0);
        }

        let off = u32::try_from(off).map_err(|_| LittleFsError::INVALID)?;
        let len = u32::try_from(len).map_err(|_| LittleFsError::INVALID)?;
        let end = off.checked_add(len).ok_or(LittleFsError::INVALID)?;

        self.flash
            .blocking_erase_range(off, end)
            .map_err(to_littlefs_error)?;
        Ok(len as usize)
    }
}
