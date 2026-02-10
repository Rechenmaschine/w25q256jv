use crate::{Error, W25q256jv, N_SECTORS, PAGE_SIZE, SECTOR_SIZE};
use core::fmt::Debug;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiDevice;
use littlefs2::driver::Storage;
use littlefs2::io::{Error as LittleFsError, Result as LittleFsResult};

fn to_littlefs_error<S: Debug, P: Debug>(error: Error<S, P>) -> LittleFsError {
    match error {
        Error::NotAligned | Error::OutOfBounds => LittleFsError::INVALID,
        _ => LittleFsError::IO,
    }
}

impl<SPI, S: Debug, P: Debug, HOLD, WP> Storage for W25q256jv<SPI, HOLD, WP>
where
    SPI: SpiDevice<Error = S>,
    HOLD: OutputPin<Error = P>,
    WP: OutputPin<Error = P>,
    S: Debug,
    P: Debug,
{
    const READ_SIZE: usize = 1;
    const WRITE_SIZE: usize = PAGE_SIZE as usize;
    const BLOCK_SIZE: usize = SECTOR_SIZE as usize;
    const BLOCK_COUNT: usize = N_SECTORS as usize;
    const BLOCK_CYCLES: isize = 100_000;

    type CACHE_SIZE = littlefs2::consts::U256;
    type LOOKAHEAD_SIZE = littlefs2::consts::U16;

    fn read(&mut self, off: usize, buf: &mut [u8]) -> LittleFsResult<usize> {
        let off = u32::try_from(off).map_err(|_| LittleFsError::INVALID)?;
        self.blocking_read(off, buf).map_err(to_littlefs_error)?;
        Ok(buf.len())
    }

    fn write(&mut self, off: usize, data: &[u8]) -> LittleFsResult<usize> {
        let off = u32::try_from(off).map_err(|_| LittleFsError::INVALID)?;
        self.blocking_write(off, data).map_err(to_littlefs_error)?;
        Ok(data.len())
    }

    fn erase(&mut self, off: usize, len: usize) -> LittleFsResult<usize> {
        if len == 0 {
            return Ok(0);
        }

        let off = u32::try_from(off).map_err(|_| LittleFsError::INVALID)?;
        let len = u32::try_from(len).map_err(|_| LittleFsError::INVALID)?;
        let end = off.checked_add(len).ok_or(LittleFsError::INVALID)?;

        self.blocking_erase_range(off, end)
            .map_err(to_littlefs_error)?;
        Ok(len as usize)
    }
}
