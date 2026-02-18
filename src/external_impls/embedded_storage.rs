use crate::{SECTOR_SIZE, W25q256jv};
use core::fmt::Debug;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiDevice as BlockingSpiDevice;
use embedded_hal_async::spi::SpiDevice as AsyncSpiDevice;
use embedded_storage::nor_flash::{
    NorFlash as BlockingNorFlash, ReadNorFlash as BlockingReadNorFlash,
};
use embedded_storage_async::nor_flash::{
    NorFlash as AsyncNorFlash, ReadNorFlash as AsyncReadNorFlash,
};

impl<SPI, S: Debug, P: Debug, HOLD, WP> AsyncReadNorFlash for W25q256jv<SPI, HOLD, WP>
where
    SPI: AsyncSpiDevice<Error = S>,
    HOLD: OutputPin<Error = P>,
    WP: OutputPin<Error = P>,
    S: Debug,
    P: Debug,
{
    const READ_SIZE: usize = 1;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.read(offset, bytes).await
    }

    fn capacity(&self) -> usize {
        Self::capacity()
    }
}

impl<SPI, S: Debug, P: Debug, HOLD, WP> AsyncNorFlash for W25q256jv<SPI, HOLD, WP>
where
    SPI: AsyncSpiDevice<Error = S>,
    HOLD: OutputPin<Error = P>,
    WP: OutputPin<Error = P>,
    S: Debug,
    P: Debug,
{
    const WRITE_SIZE: usize = 1;
    const ERASE_SIZE: usize = SECTOR_SIZE as usize;

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        self.erase_range(from, to).await
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.write(offset, bytes).await
    }
}

impl<SPI, S: Debug, P: Debug, HOLD, WP> BlockingReadNorFlash for W25q256jv<SPI, HOLD, WP>
where
    SPI: BlockingSpiDevice<Error = S>,
    HOLD: OutputPin<Error = P>,
    WP: OutputPin<Error = P>,
    S: Debug,
    P: Debug,
{
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.blocking_read(offset, bytes)
    }

    fn capacity(&self) -> usize {
        Self::capacity()
    }
}

impl<SPI, S: Debug, P: Debug, HOLD, WP> BlockingNorFlash for W25q256jv<SPI, HOLD, WP>
where
    SPI: BlockingSpiDevice<Error = S>,
    HOLD: OutputPin<Error = P>,
    WP: OutputPin<Error = P>,
    S: Debug,
    P: Debug,
{
    const WRITE_SIZE: usize = 1;
    const ERASE_SIZE: usize = SECTOR_SIZE as usize;

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        self.blocking_erase_range(from, to)
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.blocking_write(offset, bytes)
    }
}
