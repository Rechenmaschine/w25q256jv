use super::*;
use core::fmt::Debug;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::{Operation, SpiDevice as BlockingSpiDevice};
use embedded_hal_async::spi::SpiDevice as AsyncSpiDevice;

impl<SPI, S: Debug, P: Debug, HOLD, WP> W25q256jv<SPI, HOLD, WP>
where
    SPI: AsyncSpiDevice<Error = S>,
    HOLD: OutputPin<Error = P>,
    WP: OutputPin<Error = P>,
    S: Debug,
    P: Debug,
{
    /// Reads status register 1 of the flash chip.
    async fn read_status_register(&mut self) -> Result<u8, Error<S, P>> {
        let mut buf: [u8; 2] = [Command::ReadStatusRegister1 as u8, 0];

        AsyncSpiDevice::transfer_in_place(&mut self.spi, &mut buf)
            .await
            .map_err(Error::SpiError)?;

        Ok(buf[1])
    }

    /// The flash chip is unable to perform new commands while it is still working on a previous one. Especially erases take a long time.
    /// This function returns true while the chip is unable to respond to commands (with the exception of the busy command).
    pub async fn busy(&mut self) -> Result<bool, Error<S, P>> {
        Ok((self.read_status_register().await? & 0x01) != 0)
    }

    /// Sets the enable_write flag on the flash chip to true.
    /// Writes and erases to the chip only have effect when this flag is true.
    /// Each write and erase clears the flag, requiring it to be set to true again for the next command.
    async fn enable_write(&mut self) -> Result<(), Error<S, P>> {
        AsyncSpiDevice::write(&mut self.spi, &[Command::WriteEnable as u8])
            .await
            .map_err(Error::SpiError)?;

        if !self.write_enabled().await? {
            return Err(Error::WriteEnableFail);
        }

        Ok(())
    }

    /// The flash chip must be write-enabled for write and erase operations to work.
    /// This function returns true while the write-enable flag is set.
    pub async fn write_enabled(&mut self) -> Result<bool, Error<S, P>> {
        Ok((self.read_status_register().await? & 0x02) != 0)
    }

    /// The flash chip will enter into 4-byte address mode. The factory default is 3-byte
    /// address mode. Note that the W25Q256JV supports dedicated 4-byte address mode commands,
    /// which take 4-byte addresses regardless of the address mode.
    async fn enter_4_byte_address_mode(&mut self) -> Result<(), Error<S, P>> {
        AsyncSpiDevice::write(&mut self.spi, &[Command::Enter4ByteAddressMode as u8])
            .await
            .map_err(Error::SpiError)?;

        Ok(())
    }

    /// The flash chip will exit 4-byte address mode. The factory default is 3-byte
    /// address mode. Note that the W25Q256JV supports dedicated 4-byte address mode commands,
    /// which take 4-byte addresses regardless of the address mode.
    #[allow(dead_code)]
    async fn exit_4_byte_address_mode(&mut self) -> Result<(), Error<S, P>> {
        AsyncSpiDevice::write(&mut self.spi, &[Command::Exit4ByteAddressMode as u8])
            .await
            .map_err(Error::SpiError)?;

        Ok(())
    }

    /// Resets the chip without respect to ongoing operations. Data corruption may happen if
    /// there is an ongoing or suspended internal Erase or Program operation
    #[allow(clippy::missing_safety_doc)]
    pub async unsafe fn reset(&mut self) -> Result<(), Error<S, P>> {
        AsyncSpiDevice::write(&mut self.spi, &[Command::ResetDevice as u8])
            .await
            .map_err(Error::SpiError)?;
        AsyncSpiDevice::write(&mut self.spi, &[Command::EnableReset as u8])
            .await
            .map_err(Error::SpiError)?;
        Ok(())
    }

    /// Reads a chunk of bytes from the flash chip.
    /// The number of bytes read is equal to the length of the buf slice.
    /// The first byte is read from the provided address. This address is then incremented for each following byte.
    ///
    /// # Arguments
    /// * `address` - Address where the first byte of the buf will be read.
    /// * `buf` - Slice that is going to be filled with the read bytes.
    pub async fn read(&mut self, address: u32, buf: &mut [u8]) -> Result<(), Error<S, P>> {
        if address + buf.len() as u32 > CAPACITY {
            return Err(Error::OutOfBounds);
        }

        let command = command_and_address(Command::ReadDataWith4ByteAddress as u8, address);
        let mut ops = [Operation::Write(&command), Operation::Read(buf)];

        AsyncSpiDevice::transaction(&mut self.spi, &mut ops)
            .await
            .map_err(Error::SpiError)?;

        Ok(())
    }

    /// Writes a chunk of bytes to the flash chip.
    /// The first byte is written to the provided address. This address is then incremented for each following byte.
    ///
    /// This function will wait for any ongoing operations to complete before starting the write operation,
    /// to prevent data corruption.
    ///
    /// As this is a NOR-flash chip, the write operation will only change bits from 1 to 0.
    /// Overwriting pages that have already been written to may lead to unexpected behavior.
    /// It is recommended to erase the sector before writing to it.
    ///
    /// # Arguments
    /// * `address` - Address where the first byte of the buf will be written.
    /// * `buf` - Slice of bytes that will be written.
    pub async fn write(&mut self, mut address: u32, buf: &[u8]) -> Result<(), Error<S, P>> {
        if address + buf.len() as u32 > CAPACITY {
            return Err(Error::OutOfBounds);
        }

        // Wait for any ongoing operations to complete
        while self.busy().await? {}

        // Write first chunk, taking into account that given address might
        // point to a location that is not on a page boundary,
        let chunk_len = core::cmp::min((PAGE_SIZE - (address & 0x000000FF)) as usize, buf.len());
        self.write_page(address, &buf[..chunk_len]).await?;
        address += chunk_len as u32;

        for chunk in buf[chunk_len..].chunks(PAGE_SIZE as usize) {
            self.write_page(address, chunk).await?;
            address += chunk.len() as u32;
        }

        Ok(())
    }

    /// Executes a page write operation on the flash chip.
    ///
    /// This function assumes that there are no ongoing operations on the chip, otherwise
    /// the write operation will be silently ignored.
    async fn write_page(&mut self, address: u32, buf: &[u8]) -> Result<(), Error<S, P>> {
        // We don't support wrapping writes. They're scary
        if (address & 0x000000FF) + buf.len() as u32 > PAGE_SIZE {
            return Err(Error::OutOfBounds);
        }

        self.enable_write().await?;

        let command = command_and_address(Command::PageProgramWith4ByteAddress as u8, address);
        let mut ops = [Operation::Write(&command), Operation::Write(buf)];

        AsyncSpiDevice::transaction(&mut self.spi, &mut ops)
            .await
            .map_err(Error::SpiError)?;

        // typical 0.7ms, max 3ms
        while self.busy().await? {}

        if cfg!(feature = "readback-check") {
            self.readback_check(address, buf).await?;
        }

        Ok(())
    }

    /// Checks if the data at the provided address matches the provided slice.
    async fn readback_check(&mut self, mut address: u32, data: &[u8]) -> Result<(), Error<S, P>> {
        const CHUNK_SIZE: usize = 64;

        let mut buf = [0; CHUNK_SIZE];

        for chunk in data.chunks(CHUNK_SIZE) {
            let buf = &mut buf[..chunk.len()];
            self.read(address, buf).await?;
            address += CHUNK_SIZE as u32;

            if buf != chunk {
                return Err(Error::ReadbackFail);
            }
        }

        Ok(())
    }

    /// Erases a range of sectors. The range is expressed in bytes. These bytes need to be a multiple of SECTOR_SIZE.
    /// If the range starts at SECTOR_SIZE * 3 then the erase starts at the fourth sector.
    /// All sectors are erased in the range [start_sector..end_sector].
    /// The start address may not be a higher value than the end address.
    ///
    /// # Arguments
    /// * `start_address` - Address of the first byte of the start of the range of sectors that need to be erased.
    /// * `end_address` - Address of the first byte of the end of the range of sectors that need to be erased.
    pub async fn erase_range(
        &mut self,
        start_address: u32,
        end_address: u32,
    ) -> Result<(), Error<S, P>> {
        if !start_address.is_multiple_of(SECTOR_SIZE) {
            return Err(Error::NotAligned);
        }

        if !end_address.is_multiple_of(SECTOR_SIZE) {
            return Err(Error::NotAligned);
        }

        if start_address > end_address {
            return Err(Error::OutOfBounds);
        }

        let start_sector = start_address / SECTOR_SIZE;
        let end_sector = end_address / SECTOR_SIZE;

        for sector in start_sector..end_sector {
            self.erase_sector(sector).await?;
        }

        Ok(())
    }

    /// Erases a single sector of flash memory with the size of SECTOR_SIZE.
    ///
    /// # Arguments
    /// * `index` - the index of the sector that needs to be erased. The address of the first byte of the sector is the provided index * SECTOR_SIZE.
    pub async fn erase_sector(&mut self, index: u32) -> Result<(), Error<S, P>> {
        if index >= N_SECTORS {
            return Err(Error::OutOfBounds);
        }

        // in case the chip is still busy from previous operation
        while self.busy().await? {}

        self.enable_write().await?;
        let address = index * SECTOR_SIZE;

        let command = command_and_address(Command::SectorErase4KBWith4ByteAddress as u8, address);
        AsyncSpiDevice::write(&mut self.spi, &command)
            .await
            .map_err(Error::SpiError)?;

        // typical 50ms, max 400ms
        while self.busy().await? {}

        if cfg!(feature = "readback-check") {
            for offset in (0..SECTOR_SIZE).step_by(64) {
                self.readback_check(address + offset, &[0xFF; 64]).await?;
            }
        }

        Ok(())
    }

    /// Erases a single block of flash memory with the size of BLOCK_32K_SIZE.
    ///
    /// # Arguments
    /// * `index` - the index of the block that needs to be erased. The address of the first byte of the block is the provided index * BLOCK_32K_SIZE.
    pub async fn erase_block_32k(&mut self, index: u32) -> Result<(), Error<S, P>> {
        if index >= N_BLOCKS_32K {
            return Err(Error::OutOfBounds);
        }

        self.enable_write().await?;

        // this command requires 4-byte address mode, so we enter it here.
        self.enter_4_byte_address_mode().await?;

        let address = index * BLOCK_32K_SIZE;

        let command = command_and_address(Command::BlockErase32KB as u8, address);
        AsyncSpiDevice::write(&mut self.spi, &command)
            .await
            .map_err(Error::SpiError)?;

        // typical 120ms, max 1600ms
        while self.busy().await? {}

        // we don't need to exit 4-byte address mode as no command in our driver
        // requires 3-byte address mode.

        if cfg!(feature = "readback-check") {
            for offset in (0..BLOCK_32K_SIZE).step_by(64) {
                self.readback_check(address + offset, &[0xFF; 64]).await?;
            }
        }

        Ok(())
    }

    /// Erases a single block of flash memory with the size of BLOCK_64K_SIZE.
    ///
    /// Waits for the chip to complete its current operation before starting the erase operation.
    ///
    /// # Arguments
    /// * `index` - the index of the block that needs to be erased. The address of the first byte of the block is the provided index * BLOCK_64K_SIZE.
    pub async fn erase_block_64k(&mut self, index: u32) -> Result<(), Error<S, P>> {
        if index >= N_BLOCKS_64K {
            return Err(Error::OutOfBounds);
        }

        while self.busy().await? {} // in case the chip is still busy from previous operation

        self.enable_write().await?;

        let address = index * BLOCK_64K_SIZE;

        let command = command_and_address(Command::BlockErase64KBWith4ByteAddress as u8, address);
        AsyncSpiDevice::write(&mut self.spi, &command)
            .await
            .map_err(Error::SpiError)?;

        // typical 150ms, max 1600ms
        while self.busy().await? {}

        if cfg!(feature = "readback-check") {
            for offset in (0..BLOCK_64K_SIZE).step_by(64) {
                self.readback_check(address + offset, &[0xFF; 64]).await?;
            }
        }

        Ok(())
    }

    /// Erases all sectors on the flash chip. This is a very expensive operation.
    ///
    /// Waits for the chip to complete its current operation before starting the erase operation.
    pub async fn erase_chip(&mut self) -> Result<(), Error<S, P>> {
        while self.busy().await? {} // in case the chip is still busy from a previous operation

        self.enable_write().await?;

        AsyncSpiDevice::write(&mut self.spi, &[Command::ChipErase as u8])
            .await
            .map_err(Error::SpiError)?;

        // typical 80s, max 400s
        while self.busy().await? {}

        if cfg!(feature = "readback-check") {
            for address in (0..CAPACITY).step_by(64) {
                self.readback_check(address, &[0xFF; 64]).await?;
            }
        }

        Ok(())
    }
}

impl<SPI, S: Debug, P: Debug, HOLD, WP> W25q256jv<SPI, HOLD, WP>
where
    SPI: BlockingSpiDevice<Error = S>,
    HOLD: OutputPin<Error = P>,
    WP: OutputPin<Error = P>,
    S: Debug,
    P: Debug,
{
    /// Reads status register 1 of the flash chip.
    fn blocking_read_status_register(&mut self) -> Result<u8, Error<S, P>> {
        let mut buf: [u8; 2] = [Command::ReadStatusRegister1 as u8, 0];

        BlockingSpiDevice::transfer_in_place(&mut self.spi, &mut buf).map_err(Error::SpiError)?;

        Ok(buf[1])
    }

    /// The flash chip is unable to perform new commands while it is still working on a previous one. Especially erases take a long time.
    /// This function returns true while the chip is unable to respond to commands (with the exception of the busy command).
    pub fn blocking_busy(&mut self) -> Result<bool, Error<S, P>> {
        Ok((self.blocking_read_status_register()? & 0x01) != 0)
    }

    /// Sets the enable_write flag on the flash chip to true.
    /// Writes and erases to the chip only have effect when this flag is true.
    /// Each write and erase clears the flag, requiring it to be set to true again for the next command.
    fn blocking_enable_write(&mut self) -> Result<(), Error<S, P>> {
        BlockingSpiDevice::write(&mut self.spi, &[Command::WriteEnable as u8])
            .map_err(Error::SpiError)?;

        if !self.blocking_write_enabled()? {
            return Err(Error::WriteEnableFail);
        }

        Ok(())
    }

    /// The flash chip must be write-enabled for write and erase operations to work.
    /// This function returns true while the write-enable flag is set.
    pub fn blocking_write_enabled(&mut self) -> Result<bool, Error<S, P>> {
        Ok((self.blocking_read_status_register()? & 0x02) != 0)
    }

    /// The flash chip will enter into 4-byte address mode. The factory default is 3-byte
    /// address mode. Note that the W25Q256JV supports dedicated 4-byte address mode commands,
    /// which take 4-byte addresses regardless of the address mode.
    fn blocking_enter_4_byte_address_mode(&mut self) -> Result<(), Error<S, P>> {
        BlockingSpiDevice::write(&mut self.spi, &[Command::Enter4ByteAddressMode as u8])
            .map_err(Error::SpiError)?;

        Ok(())
    }

    /// The flash chip will exit 4-byte address mode. The factory default is 3-byte
    /// address mode. Note that the W25Q256JV supports dedicated 4-byte address mode commands,
    /// which take 4-byte addresses regardless of the address mode.
    #[allow(dead_code)]
    fn blocking_exit_4_byte_address_mode(&mut self) -> Result<(), Error<S, P>> {
        BlockingSpiDevice::write(&mut self.spi, &[Command::Exit4ByteAddressMode as u8])
            .map_err(Error::SpiError)?;

        Ok(())
    }

    /// Resets the chip without respect to ongoing operations. Data corruption may happen if
    /// there is an ongoing or suspended internal Erase or Program operation
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn blocking_reset(&mut self) -> Result<(), Error<S, P>> {
        BlockingSpiDevice::write(&mut self.spi, &[Command::ResetDevice as u8])
            .map_err(Error::SpiError)?;
        BlockingSpiDevice::write(&mut self.spi, &[Command::EnableReset as u8])
            .map_err(Error::SpiError)?;
        Ok(())
    }

    /// Reads a chunk of bytes from the flash chip.
    /// The number of bytes read is equal to the length of the buf slice.
    /// The first byte is read from the provided address. This address is then incremented for each following byte.
    ///
    /// # Arguments
    /// * `address` - Address where the first byte of the buf will be read.
    /// * `buf` - Slice that is going to be filled with the read bytes.
    pub fn blocking_read(&mut self, address: u32, buf: &mut [u8]) -> Result<(), Error<S, P>> {
        if address + buf.len() as u32 > CAPACITY {
            return Err(Error::OutOfBounds);
        }

        let command = command_and_address(Command::ReadDataWith4ByteAddress as u8, address);
        let mut ops = [Operation::Write(&command), Operation::Read(buf)];

        BlockingSpiDevice::transaction(&mut self.spi, &mut ops).map_err(Error::SpiError)?;

        Ok(())
    }

    /// Writes a chunk of bytes to the flash chip.
    /// The first byte is written to the provided address. This address is then incremented for each following byte.
    ///
    /// This function will wait for any ongoing operations to complete before starting the write operation,
    /// to prevent data corruption.
    ///
    /// As this is a NOR-flash chip, the write operation will only change bits from 1 to 0.
    /// Overwriting pages that have already been written to may lead to unexpected behavior.
    /// It is recommended to erase the sector before writing to it.
    ///
    /// # Arguments
    /// * `address` - Address where the first byte of the buf will be written.
    /// * `buf` - Slice of bytes that will be written.
    pub fn blocking_write(&mut self, mut address: u32, buf: &[u8]) -> Result<(), Error<S, P>> {
        if address + buf.len() as u32 > CAPACITY {
            return Err(Error::OutOfBounds);
        }

        // Wait for any ongoing operations to complete
        while self.blocking_busy()? {}

        // Write first chunk, taking into account that given address might
        // point to a location that is not on a page boundary,
        let chunk_len = core::cmp::min((PAGE_SIZE - (address & 0x000000FF)) as usize, buf.len());
        self.blocking_write_page(address, &buf[..chunk_len])?;
        address += chunk_len as u32;

        for chunk in buf[chunk_len..].chunks(PAGE_SIZE as usize) {
            self.blocking_write_page(address, chunk)?;
            address += chunk.len() as u32;
        }

        Ok(())
    }

    /// Executes a page write operation on the flash chip.
    ///
    /// This function assumes that there are no ongoing operations on the chip, otherwise
    /// the write operation will be silently ignored.
    fn blocking_write_page(&mut self, address: u32, buf: &[u8]) -> Result<(), Error<S, P>> {
        // We don't support wrapping writes. They're scary
        if (address & 0x000000FF) + buf.len() as u32 > PAGE_SIZE {
            return Err(Error::OutOfBounds);
        }

        self.blocking_enable_write()?;

        let command = command_and_address(Command::PageProgramWith4ByteAddress as u8, address);
        let mut ops = [Operation::Write(&command), Operation::Write(buf)];

        BlockingSpiDevice::transaction(&mut self.spi, &mut ops).map_err(Error::SpiError)?;

        // typical 0.7ms, max 3ms
        while self.blocking_busy()? {}

        if cfg!(feature = "readback-check") {
            self.blocking_readback_check(address, buf)?;
        }

        Ok(())
    }

    /// Checks if the data at the provided address matches the provided slice.
    fn blocking_readback_check(
        &mut self,
        mut address: u32,
        data: &[u8],
    ) -> Result<(), Error<S, P>> {
        const CHUNK_SIZE: usize = 64;

        let mut buf = [0; CHUNK_SIZE];

        for chunk in data.chunks(CHUNK_SIZE) {
            let buf = &mut buf[..chunk.len()];
            self.blocking_read(address, buf)?;
            address += CHUNK_SIZE as u32;

            if buf != chunk {
                return Err(Error::ReadbackFail);
            }
        }

        Ok(())
    }

    /// Erases a range of sectors. The range is expressed in bytes. These bytes need to be a multiple of SECTOR_SIZE.
    /// If the range starts at SECTOR_SIZE * 3 then the erase starts at the fourth sector.
    /// All sectors are erased in the range [start_sector..end_sector].
    /// The start address may not be a higher value than the end address.
    ///
    /// # Arguments
    /// * `start_address` - Address of the first byte of the start of the range of sectors that need to be erased.
    /// * `end_address` - Address of the first byte of the end of the range of sectors that need to be erased.
    pub fn blocking_erase_range(
        &mut self,
        start_address: u32,
        end_address: u32,
    ) -> Result<(), Error<S, P>> {
        if !start_address.is_multiple_of(SECTOR_SIZE) {
            return Err(Error::NotAligned);
        }

        if !end_address.is_multiple_of(SECTOR_SIZE) {
            return Err(Error::NotAligned);
        }

        if start_address > end_address {
            return Err(Error::OutOfBounds);
        }

        let start_sector = start_address / SECTOR_SIZE;
        let end_sector = end_address / SECTOR_SIZE;

        for sector in start_sector..end_sector {
            self.blocking_erase_sector(sector)?;
        }

        Ok(())
    }

    /// Erases a single sector of flash memory with the size of SECTOR_SIZE.
    ///
    /// # Arguments
    /// * `index` - the index of the sector that needs to be erased. The address of the first byte of the sector is the provided index * SECTOR_SIZE.
    pub fn blocking_erase_sector(&mut self, index: u32) -> Result<(), Error<S, P>> {
        if index >= N_SECTORS {
            return Err(Error::OutOfBounds);
        }

        // in case the chip is still busy from previous operation
        while self.blocking_busy()? {}

        self.blocking_enable_write()?;
        let address = index * SECTOR_SIZE;

        let command = command_and_address(Command::SectorErase4KBWith4ByteAddress as u8, address);
        BlockingSpiDevice::write(&mut self.spi, &command).map_err(Error::SpiError)?;

        // typical 50ms, max 400ms
        while self.blocking_busy()? {}

        if cfg!(feature = "readback-check") {
            for offset in (0..SECTOR_SIZE).step_by(64) {
                self.blocking_readback_check(address + offset, &[0xFF; 64])?;
            }
        }

        Ok(())
    }

    /// Erases a single block of flash memory with the size of BLOCK_32K_SIZE.
    ///
    /// # Arguments
    /// * `index` - the index of the block that needs to be erased. The address of the first byte of the block is the provided index * BLOCK_32K_SIZE.
    pub fn blocking_erase_block_32k(&mut self, index: u32) -> Result<(), Error<S, P>> {
        if index >= N_BLOCKS_32K {
            return Err(Error::OutOfBounds);
        }

        self.blocking_enable_write()?;

        // this command requires 4-byte address mode, so we enter it here.
        self.blocking_enter_4_byte_address_mode()?;

        let address = index * BLOCK_32K_SIZE;

        let command = command_and_address(Command::BlockErase32KB as u8, address);
        BlockingSpiDevice::write(&mut self.spi, &command).map_err(Error::SpiError)?;

        // typical 120ms, max 1600ms
        while self.blocking_busy()? {}

        // we don't need to exit 4-byte address mode as no command in our driver
        // requires 3-byte address mode.

        if cfg!(feature = "readback-check") {
            for offset in (0..BLOCK_32K_SIZE).step_by(64) {
                self.blocking_readback_check(address + offset, &[0xFF; 64])?;
            }
        }

        Ok(())
    }

    /// Erases a single block of flash memory with the size of BLOCK_64K_SIZE.
    ///
    /// Waits for the chip to complete its current operation before starting the erase operation.
    ///
    /// # Arguments
    /// * `index` - the index of the block that needs to be erased. The address of the first byte of the block is the provided index * BLOCK_64K_SIZE.
    pub fn blocking_erase_block_64k(&mut self, index: u32) -> Result<(), Error<S, P>> {
        if index >= N_BLOCKS_64K {
            return Err(Error::OutOfBounds);
        }

        while self.blocking_busy()? {} // in case the chip is still busy from previous operation

        self.blocking_enable_write()?;

        let address = index * BLOCK_64K_SIZE;

        let command = command_and_address(Command::BlockErase64KBWith4ByteAddress as u8, address);
        BlockingSpiDevice::write(&mut self.spi, &command).map_err(Error::SpiError)?;

        // typical 150ms, max 1600ms
        while self.blocking_busy()? {}

        if cfg!(feature = "readback-check") {
            for offset in (0..BLOCK_64K_SIZE).step_by(64) {
                self.blocking_readback_check(address + offset, &[0xFF; 64])?;
            }
        }

        Ok(())
    }

    /// Erases all sectors on the flash chip. This is a very expensive operation.
    ///
    /// Waits for the chip to complete its current operation before starting the erase operation.
    pub fn blocking_erase_chip(&mut self) -> Result<(), Error<S, P>> {
        while self.blocking_busy()? {} // in case the chip is still busy from a previous operation

        self.blocking_enable_write()?;

        BlockingSpiDevice::write(&mut self.spi, &[Command::ChipErase as u8])
            .map_err(Error::SpiError)?;

        // typical 80s, max 400s
        while self.blocking_busy()? {}

        if cfg!(feature = "readback-check") {
            for address in (0..CAPACITY).step_by(64) {
                self.blocking_readback_check(address, &[0xFF; 64])?;
            }
        }

        Ok(())
    }
}
