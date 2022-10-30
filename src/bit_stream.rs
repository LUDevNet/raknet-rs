use std::{
    mem::{self, MaybeUninit},
    slice,
};

use crate::{bits_to_bytes, packet_priority::OrderingChannel, PacketReliability, Result, ID};

pub type BitSize = usize;

pub trait ReadSafe {}
impl ReadSafe for u64 {}
impl ReadSafe for u32 {}
impl ReadSafe for u16 {}
impl ReadSafe for u8 {}

pub trait WriteSafe {}
impl WriteSafe for u64 {}
impl WriteSafe for u32 {}
impl WriteSafe for u16 {}
impl WriteSafe for u8 {}
impl WriteSafe for ID {}

/// # Safety
///
/// TODO: Document
pub unsafe trait Bits {
    const NUM: usize;
}

unsafe impl Bits for OrderingChannel {
    const NUM: usize = 5;
}

unsafe impl Bits for PacketReliability {
    const NUM: usize = 3;
}

fn bytes_from_maybe_uninit<T>(u: &mut MaybeUninit<T>) -> &mut [MaybeUninit<u8>] {
    unsafe {
        // From std
        slice::from_raw_parts_mut(u.as_mut_ptr() as *mut MaybeUninit<u8>, mem::size_of::<T>())
    }
}

pub struct BitStreamRead<'a> {
    number_of_bits_used: BitSize,
    //number_of_bits_allocated: BitSize,
    read_offset: BitSize,
    data: &'a [u8],
}

impl<'a> BitStreamRead<'a> {
    /// Create a new instance with a specified number of bits
    pub fn with_size(data: &'a [u8], number_of_bits_used: BitSize) -> Self {
        assert!(data.len() << 3 >= number_of_bits_used);
        Self {
            number_of_bits_used,
            read_offset: 0,
            data,
        }
    }

    /// Create a new instance from a byte slice
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            number_of_bits_used: data.len() << 3,
            read_offset: 0,
        }
    }

    pub fn ignore_bits(&mut self, number_of_bits: BitSize) -> Result<()> {
        if self.read_offset + number_of_bits > self.number_of_bits_used {
            Err(crate::Error::BitStreamEos)
        } else {
            self.read_offset += number_of_bits;
            Ok(())
        }
    }

    pub const fn get_number_of_unread_bits(&self) -> BitSize {
        self.number_of_bits_used - self.read_offset
    }

    pub fn read_bit(&mut self) -> bool {
        let result = (self.data[self.read_offset >> 3] & (0x80 >> (self.read_offset & 7))) != 0;
        self.read_offset += 1;
        result
    }

    pub fn read<T: ReadSafe>(&mut self) -> Result<T> {
        let mut u = MaybeUninit::<T>::uninit();
        let bytes = bytes_from_maybe_uninit(&mut u);

        self._read_bits(bytes, mem::size_of::<T>() * 8, true)?;
        Ok(unsafe { u.assume_init() })
    }

    pub fn read_bool(&mut self) -> Result<bool> {
        if self.read_offset + 1 > self.number_of_bits_used {
            return Err(crate::Error::BitStreamEos);
        }

        let var = (self.data[self.read_offset >> 3] & (0x80 >> (self.read_offset & 7))) != 0;

        // Has to be on a different line for Mac
        self.read_offset += 1;

        Ok(var)
    }

    pub fn read_bits<T: Bits>(&mut self) -> Result<T> {
        assert!(T::NUM <= mem::size_of::<T>() << 3);
        let mut buffer = MaybeUninit::uninit();
        self._read_bits(bytes_from_maybe_uninit(&mut buffer), T::NUM, true)?;
        Ok(unsafe { buffer.assume_init() })
    }

    fn _read_bits(
        &mut self,
        output: &mut [MaybeUninit<u8>],
        mut number_of_bits_to_read: usize,
        align_bits_to_right: bool,
    ) -> Result<()> {
        if self.read_offset + number_of_bits_to_read > self.number_of_bits_used {
            return Err(crate::Error::BitStreamEos);
        }

        let read_offset_mod_8: BitSize = self.read_offset & 7;
        let mut offset: BitSize = 0;

        output.fill(MaybeUninit::new(0));

        while number_of_bits_to_read > 0 {
            unsafe {
                *output[offset].as_mut_ptr() |=
                    self.data[(self.read_offset >> 3)] << read_offset_mod_8
            }; // First half

            if read_offset_mod_8 > 0 && number_of_bits_to_read > 8 - read_offset_mod_8 {
                // If we have a second half, we didn't read enough bytes in the first half
                unsafe {
                    *output[offset].as_mut_ptr() |=
                        self.data[(self.read_offset >> 3) + 1] >> (8 - read_offset_mod_8)
                }; // Second half (overlaps byte boundary)
            }

            if number_of_bits_to_read >= 8 {
                number_of_bits_to_read -= 8;
                self.read_offset += 8;
                offset += 1;
            } else {
                let neg = number_of_bits_to_read as isize - 8;

                if neg < 0
                // Reading a partial byte for the last byte, shift right so the data is aligned on the right
                {
                    if align_bits_to_right {
                        unsafe { *output[offset].as_mut_ptr() >>= -neg };
                    }

                    self.read_offset += (8 + neg) as usize;
                } else {
                    self.read_offset += 8;
                }

                offset += 1;

                number_of_bits_to_read = 0;
            }
        }

        Ok(())
    }

    /// Public version of `ReadCompressed`
    pub fn read_compressed<T: ReadSafe>(&mut self) -> Result<T> {
        let mut buf = MaybeUninit::uninit();
        self._read_compressed(
            bytes_from_maybe_uninit(&mut buf),
            mem::size_of::<T>() << 3,
            true,
        )?;
        Ok(unsafe { buf.assume_init() })
    }

    // Align the next write and/or read to a byte boundary.  This can be used to 'waste' bits to byte align for efficiency reasons
    fn _align_read_to_byte_boundary(&mut self) {
        if self.read_offset > 0 {
            self.read_offset += 8 - (((self.read_offset - 1) & 7) + 1);
        }
    }

    pub fn read_aligned_bytes(&mut self, number_of_bytes_to_read: usize) -> Result<&'a [u8]> {
        if number_of_bytes_to_read == 0 {
            return Err(crate::Error::BitStreamEos);
        }

        self._align_read_to_byte_boundary();
        if self.read_offset + (number_of_bytes_to_read << 3) > self.number_of_bits_used {
            return Err(crate::Error::BitStreamEos);
        }

        let slice = &self.data[self.read_offset >> 3..][..number_of_bytes_to_read];
        self.read_offset += number_of_bytes_to_read << 3;

        Ok(slice)
    }

    /// Internal version of `ReadCompressed`
    fn _read_compressed(
        &mut self,
        output: &mut [MaybeUninit<u8>],
        size: usize,
        unsigned_data: bool,
    ) -> Result<()> {
        let mut current_byte = (size >> 3) - 1;

        let (byte_match, half_byte_match): (u8, u8) = match unsigned_data {
            true => (0, 0),
            false => (0xFF, 0xF0),
        };

        // Upper bytes are specified with a single 1 if they match byteMatch
        // From high byte to low byte, if high byte is a byteMatch then write a 1 bit. Otherwise write a 0 bit and then write the remaining bytes
        while current_byte > 0 {
            // If we read a 1 then the data is byteMatch.
            if self.read_bool()?
            // Check that bit
            {
                unsafe { *output[current_byte].as_mut_ptr() = byte_match };
                current_byte -= 1;
            } else {
                // Read the rest of the bytes
                self._read_bits(output, (current_byte + 1) << 3, true)?;
                return Ok(());
            }
        }

        // All but the first bytes are byteMatch.  If the upper half of the last byte is a 0 (positive) or 16 (negative) then what we read will be a 1 and the remaining 4 bits.
        // Otherwise we read a 0 and the 8 bytes
        //assert(readOffset+1 <=numberOfBitsUsed); // If this assert is hit the stream wasn't long enough to read from
        if self.read_offset + 1 > self.number_of_bits_used {
            return Err(crate::Error::BitStreamEos);
        }

        let b = self.read_bool()?;

        if b
        // Check that bit
        {
            self._read_bits(&mut output[current_byte..], 4, true)?;

            unsafe { *output[current_byte].as_mut_ptr() |= half_byte_match }; // We have to set the high 4 bits since these are set to 0 by ReadBits
        } else {
            self._read_bits(&mut output[current_byte..], 8, true)?;
        }

        Ok(())
    }
}

#[derive(Default, Debug, Clone)]
pub struct BitStreamWrite {
    number_of_bits_used: usize,
    data: Vec<u8>,
}

impl BitStreamWrite {
    pub fn new() -> Self {
        Self {
            number_of_bits_used: 0,
            data: Vec::new(),
        }
    }

    pub fn with_capacity(bits: usize) -> Self {
        Self {
            data: Vec::with_capacity(bits_to_bytes!(bits)),
            number_of_bits_used: 0,
        }
    }

    pub fn num_bits(&self) -> usize {
        self.number_of_bits_used
    }

    pub fn data(&self) -> &[u8] {
        &self.data[..bits_to_bytes!(self.number_of_bits_used)]
    }

    fn add_bits_and_reallocate(&mut self, number_of_bits_to_write: usize) {
        if number_of_bits_to_write == 0 {
            return;
        }

        let mut new_number_of_bits_allocated: BitSize =
            number_of_bits_to_write + self.number_of_bits_used;

        if new_number_of_bits_allocated > 0
            && (self.data.len() <= ((new_number_of_bits_allocated - 1) >> 3))
        // If we need to allocate 1 or more new bytes
        {
            // Less memory efficient but saves on news and deletes
            // Cap to 1MB buffer to save on huge allocations
            new_number_of_bits_allocated = (number_of_bits_to_write + self.number_of_bits_used) * 2;
            if new_number_of_bits_allocated - (number_of_bits_to_write + self.number_of_bits_used)
                > 1048576
            {
                new_number_of_bits_allocated =
                    number_of_bits_to_write + self.number_of_bits_used + 1048576;
            }

            // BitSize_t newByteOffset = BITS_TO_BYTES( numberOfBitsAllocated );
            // Use realloc and free so we are more efficient than delete and new for resizing
            let amount_to_allocate: BitSize = bits_to_bytes!(new_number_of_bits_allocated);

            // FIXME: Just allocate, don't fill with zeros?
            self.data.resize(amount_to_allocate, 0);
        }

        // FIXME: update the number of bits allocated
    }

    // Write a 0
    pub fn write_0(&mut self) {
        self.add_bits_and_reallocate(1);

        // New bytes need to be zeroed
        if (self.number_of_bits_used & 7) == 0 {
            self.data[self.number_of_bits_used >> 3] = 0;
        }

        self.number_of_bits_used += 1;
    }

    // Write a 1
    pub fn write_1(&mut self) {
        self.add_bits_and_reallocate(1);

        let number_of_bits_mod_8: BitSize = self.number_of_bits_used & 7;
        if number_of_bits_mod_8 == 0 {
            self.data[self.number_of_bits_used >> 3] = 0x80;
        } else {
            self.data[self.number_of_bits_used >> 3] |= 0x80 >> (number_of_bits_mod_8);
            // Set the bit to 1
        }

        self.number_of_bits_used += 1;
    }

    pub fn write_bool(&mut self, value: bool) {
        match value {
            true => self.write_1(),
            false => self.write_0(),
        }
    }

    pub fn write<T: WriteSafe>(&mut self, data: T) {
        let input =
            unsafe { slice::from_raw_parts((&data) as *const T as *const u8, mem::size_of::<T>()) };
        self._write_bits(input, std::mem::size_of::<T>() << 3, true);
    }

    /// Write some bits to the stream
    pub fn write_bits<T: Bits>(&mut self, value: T) {
        assert!(T::NUM <= mem::size_of::<T>() << 3);
        let input = unsafe {
            slice::from_raw_parts((&value) as *const T as *const u8, mem::size_of::<T>())
        };
        self._write_bits(input, T::NUM, true);
    }

    pub fn write_compressed<T: WriteSafe>(&mut self, data: T) {
        let input =
            unsafe { slice::from_raw_parts((&data) as *const T as *const u8, mem::size_of::<T>()) };
        self._write_compressed(input, true)
    }

    // Write an array or casted stream
    pub fn write_bytes(&mut self, input: &[u8], number_of_bytes: usize) {
        if number_of_bytes == 0 {
            return;
        }

        // Optimization:
        if (self.number_of_bits_used & 7) == 0 {
            self.data.extend_from_slice(input);
            self.number_of_bits_used += number_of_bytes << 3;
        } else {
            self._write_bits(input, number_of_bytes << 3, true);
        }
    }

    // Align the next write and/or read to a byte boundary.  This can be used to 'waste' bits to byte align for efficiency reasons
    fn align_write_to_byte_boundary(&mut self) {
        if self.number_of_bits_used > 0 {
            self.number_of_bits_used += 8 - (((self.number_of_bits_used - 1) & 7) + 1);
        }
    }

    pub fn write_aligned_bytes(&mut self, bytes: &[u8]) {
        self.align_write_to_byte_boundary();

        // This is inlined from [`write_bytes`]
        self.data.extend_from_slice(bytes);
        self.number_of_bits_used += bytes.len() << 3;
    }

    // Assume the input source points to a native type, compress and write it
    fn _write_compressed(&mut self, input: &[u8], unsigned_data: bool) {
        // NOTE: We use input slice len instead of bit count
        let mut current_byte: BitSize = input.len() - 1; // PCs

        let byte_match = match unsigned_data {
            true => 0,
            false => 0xFF,
        };

        // Write upper bytes with a single 1
        // From high byte to low byte, if high byte is a byteMatch then write a 1 bit. Otherwise write a 0 bit and then write the remaining bytes
        while current_byte > 0 {
            if input[current_byte] == byte_match {
                // If high byte is byteMatch (0 of 0xff) then it would have the same value shifted
                self.write_1();
            } else {
                // Write the remainder of the data after writing 0
                self.write_0();
                self._write_bits(input, (current_byte + 1) << 3, true);

                return;
            }

            current_byte -= 1;
        }

        let half_match = match unsigned_data {
            true => 0x00,
            false => 0xF0,
        };
        // If the upper half of the last byte is a 0 (positive) or 16 (negative) then write a 1 and the remaining 4 bits.  Otherwise write a 0 and the 8 bites.
        if (input[current_byte] & 0xF0) == half_match {
            self.write_1();
            self._write_bits(&input[current_byte..], 4, true);
        } else {
            self.write_0();
            self._write_bits(&input[current_byte..], 8, true);
        }
    }

    fn _write_bits(
        &mut self,
        input: &[u8],
        mut number_of_bits_to_write: usize,
        right_aligned_bits: bool,
    ) {
        if number_of_bits_to_write == 0 {
            return;
        }

        self.add_bits_and_reallocate(number_of_bits_to_write);
        let mut offset: BitSize = 0;
        let mut data_byte: u8;

        let number_of_bits_used_mod_8: BitSize = self.number_of_bits_used & 7;

        // Faster to put the while at the top surprisingly enough
        while number_of_bits_to_write > 0 {
            data_byte = input[offset];

            if number_of_bits_to_write < 8 && right_aligned_bits {
                // rightAlignedBits means in the case of a partial byte, the bits are aligned from the right (bit 0) rather than the left (as in the normal internal representation)
                data_byte <<= 8 - number_of_bits_to_write; // shift left to get the bits on the left, as in our internal representation
            }

            // Writing to a new byte each time
            if number_of_bits_used_mod_8 == 0 {
                self.data[self.number_of_bits_used >> 3] = data_byte;
            } else {
                // Copy over the new data.
                self.data[self.number_of_bits_used >> 3] |=
                    data_byte >> (number_of_bits_used_mod_8); // First half

                if 8 - (number_of_bits_used_mod_8) < 8
                    && 8 - (number_of_bits_used_mod_8) < number_of_bits_to_write
                // If we didn't write it all out in the first half (8 - (numberOfBitsUsed%8) is the number we wrote in the first half)
                {
                    self.data[(self.number_of_bits_used >> 3) + 1] =
                        data_byte << (8 - number_of_bits_used_mod_8); // Second half (overlaps byte boundary)
                }
            }

            if number_of_bits_to_write >= 8 {
                self.number_of_bits_used += 8;
            } else {
                self.number_of_bits_used += number_of_bits_to_write;
            }

            if number_of_bits_to_write >= 8 {
                number_of_bits_to_write -= 8;
            } else {
                number_of_bits_to_write = 0;
            }

            offset += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_read() {
        let mut bit_stream = super::BitStreamRead::new(&[0b11100011, 0b00000100]);
        assert!(bit_stream.read_bit());
        assert!(bit_stream.read_bit());
        assert!(bit_stream.read_bit());
        assert!(!bit_stream.read_bit());
        assert!(!bit_stream.read_bit());
        assert!(!bit_stream.read_bit());
        assert_eq!(bit_stream.read::<u8>(), Ok(0b11000001));
    }
}
