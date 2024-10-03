use core::fmt::{Debug, Formatter};
use core::marker::PhantomData;
use crate::errors::ToRustAGaugeError;

pub trait BufferMode {}


/// A buffer with this mode contains raw ascii u8's received from the elm
pub struct CharByte;
impl BufferMode for CharByte {}


/// These u8's are just hex digits that were sent by the elm in string form and then parsed to be values from 0 to 15 inclusive
pub struct HexDigit;
impl BufferMode for HexDigit {}


/// These u8's are actual bytes that were sent by the elm in hexadecimal form and then parsed
pub struct FullyAssembledByte;
impl BufferMode for FullyAssembledByte {}




impl<M: BufferMode> SizedUartBuffer<M>{
    ///true if success, false if full
    pub fn add_element(&mut self, byte: u8) -> bool{
        if self.end < crate::elm_uart::LOCAL_RX_BUFFER_LEN {
            self.buffer[self.end] = byte;
            self.end += 1;
            true
        } else {
            false
        }
    }
    
    pub fn get_slice(&self) -> &[u8]{
        &self.buffer[0..self.end]
    }
}

pub struct SizedUartBuffer<MODE: BufferMode>{
    pub buffer: [u8; crate::elm_uart::LOCAL_RX_BUFFER_LEN],
    pub end: usize,
    pub phantom: PhantomData<MODE>,
}

impl<M: BufferMode> defmt::Format for SizedUartBuffer<M> {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "{:?}", self.get_slice())
    }
}

impl<M: BufferMode> Debug for SizedUartBuffer<M> {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        write!(f, "{:?}", self.get_slice())
    }
}

pub fn parse_byte(input: &u8) -> Result<u8, ToRustAGaugeError>{
    match input{
        b'0' => {Ok(0x0)}
        b'1' => {Ok(0x1)}
        b'2' => {Ok(0x2)}
        b'3' => {Ok(0x3)}
        b'4' => {Ok(0x4)}
        b'5' => {Ok(0x5)}
        b'6' => {Ok(0x6)}
        b'7' => {Ok(0x7)}
        b'8' => {Ok(0x8)}
        b'9' => {Ok(0x9)}
        b'A' => {Ok(0xa)}
        b'B' => {Ok(0xb)}
        b'C' => {Ok(0xc)}
        b'D' => {Ok(0xd)}
        b'E' => {Ok(0xe)}
        b'F' => {Ok(0xf)}
        b'a' => {Ok(0xa)}
        b'b' => {Ok(0xb)}
        b'c' => {Ok(0xc)}
        b'd' => {Ok(0xd)}
        b'e' => {Ok(0xe)}
        b'f' => {Ok(0xf)}
        _ => Err(ToRustAGaugeError::UartByteParseError())
    }
}

pub fn combine_4bit_pair(input_slice: &[u8]) -> Result<u8, ToRustAGaugeError> {
    if input_slice.len() != 2 { return Err(ToRustAGaugeError::UartByteParseError()); }
    let byte_1 = input_slice[0];
    let byte_2 = input_slice[1];
    if byte_1>15 || byte_2>15 { return Err(ToRustAGaugeError::UartByteParseError()); }

    Ok(byte_1 << 4 | byte_2)
}
impl SizedUartBuffer<CharByte>

{
    const NO_DATA_MESSAGE: &'static[u8] = &[0x4E, 0x4F, 0x20, 0x44, 0x41, 0x54, 0x41, 0x0D, 0x0D];
    pub fn parse_bytes(&self, parsed_buf: &mut SizedUartBuffer<HexDigit>) {
        parsed_buf.end = 0;
        self.get_slice().iter().for_each(|char_byte|{
            match parse_byte(char_byte){
                Ok(parsed_byte) => {
                    if !parsed_buf.add_element(parsed_byte){
                        panic!("Parsed buffer was somehow longer than the unparsed buffer??")
                        // This should not be possible, but should be mentioned
                    }
                }
                Err(_) => {
                    // parsing byte failed, skipping. 
                    // This can be because one of the char-bytes in the pair wasn't a valid hex digit, 
                    // or this is the last byte in the buffer and couldn't form a pair
                }
            }
        });
    }
    
    pub fn is_no_data(&self) -> bool{
        let slice = &self.buffer[0..self.end];
        slice == Self::NO_DATA_MESSAGE
    }
}

impl SizedUartBuffer<FullyAssembledByte> {

    /// If this function fails, `parsed_buf` the calling instance becomes poisoned and should not be used
    pub fn populate_from_hex_digit_buffer(&mut self,
                                          parsed_byte_buffer: &SizedUartBuffer<HexDigit>
    ) -> Result<(), ToRustAGaugeError>{
        let digit_slice = &parsed_byte_buffer.buffer[0..parsed_byte_buffer.end];
        self.end = 0;
        
        digit_slice.chunks(2).try_for_each(|parsed_hex_digit_pair|{
            match combine_4bit_pair(parsed_hex_digit_pair){
                Ok(full_parsed_byte) => {
                    if self.add_element(full_parsed_byte) {
                        Ok(())
                    } else {
                        panic!("Parsed buffer was somehow longer than the unparsed buffer??")
                        // This should not be possible, but should be mentioned
                    }
                }
                Err(e) => {
                    Err(e)
                }
            }
        })
    }
}

pub fn parse_voltage(buffer: &mut SizedUartBuffer<CharByte>) -> Result<f64, ToRustAGaugeError>{
    let slice = buffer.get_slice();
    
    const MAX_NUM_DIGITS: usize = 4;
    
    let mut valid_digits: [u8; MAX_NUM_DIGITS] = [0u8; MAX_NUM_DIGITS];
    
    let mut char_index: usize = 0;
    let mut tenths_place_index: Option<usize> = None;
    
    for temp_byte in slice{
        match temp_byte{
            &v if v>=0x30 && v<=0x39 => {
                if char_index >=MAX_NUM_DIGITS{
                    return Err(ToRustAGaugeError::UartVoltageParseError())
                }
                valid_digits[char_index] = v-0x30;
                char_index +=1
            }
            b'.' => {
                tenths_place_index = Some(char_index);
            }
            _ => {}
        }
    }

    let tenths_place_index_unwrapped = match tenths_place_index{
        Some(v) => v,
        None => return Err(ToRustAGaugeError::UartVoltageParseError()),
    };
    
    let mut voltage = 0f64;
    let mut multiplier: f64 = 1f64;
    
    for digit in valid_digits{
        voltage += (digit as f64) * multiplier;
        multiplier /= 10f64;
    }
    
    let mut place_normalization_index: usize = 0;
    while place_normalization_index < tenths_place_index_unwrapped-1{
        voltage *= 10f64;
        place_normalization_index+=1;
    }
    Ok(voltage)
}


/// for a number: 420.69, place 0 is `0`, place 1 is `2`, place 2 is `4`, place -1 is `6`, place -2 is `9`. 
/// Places are inclusive so parsing 420.69 with `place_start` = 2 and `place_end` = -1 yields `"420.6"` 
/// Relies on caller to ensure that buffer is at least `place_start - place_end + 1` long. (+ one more if there's a decimal point) 
/// The result can only last until this function is called again (at least with the same buffer)
/// Returns the index of the first not included byte from the buffer such that the result is `buffer[..usize]`
pub fn float_as_str(float: f64, buffer: &mut [u8], place_start: i8, place_end: i8) -> usize{

    assert!(place_start >= place_end, "place start must be most significant place");
    debug_assert!(buffer.len() > ((place_start - place_end) + 1) as usize);
    const ASCII_DIGIT_OFFSET: u8 = 0x30;

    let mut buf_index: usize = 0;
    for i in place_end..=place_start{
        let digit_as_value = ((float / powi(10f64, i as i32)) % 10f64) as u8;
        buffer[buf_index] = digit_as_value + ASCII_DIGIT_OFFSET;
        buf_index += 1;
        if i == -1{
            buffer[buf_index] = b'.';
            buf_index += 1;
        }
    }
    let ascii_buf_slice = &mut buffer[0..buf_index];
    ascii_buf_slice.reverse();
    buf_index
}

/// stolen from [Micromath.rs](https://github.com/tarcieri/micromath) with some simplifications to reduce dependencies.
/// (Yes I stole multiplication in a for loop, suck my cock)
pub fn powi(float: f64, n: i32) -> f64 {
    let mut base = float;
    let mut abs_n = i32::abs(n);
    let mut result = 1f64;

    if n < 0 {
        base = 1.0 / float;
    }

    if n == 0 {
        return 1f64;
    }

    // 0.0 == 0.0 and -0.0 according to IEEE standards.
    if float == 0f64 && n > 0 {
        return float;
    }

    loop {
        if (abs_n & 1) == 1 {
            result *= base;
        }

        abs_n >>= 1;

        if abs_n == 0 {
            return result;
        }

        base *= base;
    }
}


#[cfg(test)]
mod tests {
    use crate::elm_uart::LOCAL_RX_BUFFER_LEN;
    use super::*;

    #[test]
    fn test_parsing() {
        let input: [u8; 51] = [0x30, 0x30, 0x20, 0x30, 0x31, 0x30, 0x32, 0x30, 0x33, 0x30, 0x34, 0x30, 0x35, 0x30, 0x36, 0x30, 0x37, 0x30, 0x38, 0x30, 0x39, 0x30, 0x41, 0x30, 0x62, 0x30, 0x43, 0x30, 0x20, 0x64, 0x30, 0x45, 0x30, 0x66, 0x66, 0x66, 0x31, 0x30, 0x20, 0x31, 0x31, 0x31, 0x32, 0x20, 0x31, 0x33, 0x31, 0x34, 0x31, 0x35, 0x0A];
        // 00 0102030405060708090A0b0C0 d0E0fff10 1112 131415
        // 00 01 02 03 04 05 06 07 08 09 0A 0b 0C 0d 0E 0f ff 10 11 12 13 14 15

        let mut raw_buf: SizedUartBuffer<CharByte> = SizedUartBuffer{
            buffer: [0u8; LOCAL_RX_BUFFER_LEN],
            end: 0,
            phantom: PhantomData,
        };
        let mut hex_buf: SizedUartBuffer<HexDigit> = SizedUartBuffer{
            buffer: [0u8; LOCAL_RX_BUFFER_LEN],
            end: 0,
            phantom: PhantomData,
        };
        let mut parsed_byte_buf: SizedUartBuffer<FullyAssembledByte> = SizedUartBuffer{
            buffer: [0u8; LOCAL_RX_BUFFER_LEN],
            end: 0,
            phantom: PhantomData,
        };
        for byte in input.iter() {
            raw_buf.add_element(*byte);
        }
        raw_buf.parse_bytes(&mut hex_buf);
        parsed_byte_buf.populate_from_hex_digit_buffer(&hex_buf).unwrap();

        defmt::println!("{:?}", parsed_byte_buf)
    }

    #[test]
    fn test_float_to_str() {
        let mut local_buffer = [0u8; 12];
        let str_len = float_as_str(420.69f64, &mut local_buffer, 2, 0);
        let str_ref = core::str::from_utf8(&local_buffer[..str_len]).unwrap();
        let expected = "420";
        assert_eq!(str_ref, expected, "brother...");
    }
}