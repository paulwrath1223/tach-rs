use defmt::Formatter;
use crate::errors::ToRustAGaugeError;

#[derive(defmt::Format, Debug)]
pub struct StaticCommand(&'static str);


impl StaticCommand {
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}


pub const ELM_RESET: StaticCommand = StaticCommand("ATZ\r");
pub const DISABLE_ECHO: StaticCommand = StaticCommand("ATE0\r");
pub const ENABLE_HEADERS: StaticCommand = StaticCommand("ATH1\r");
pub const SET_PROTOCOL_5: StaticCommand = StaticCommand("ATSP5\r");
pub const SET_TIMEOUT_64: StaticCommand = StaticCommand("ATST64\r");
pub const DISABLE_SPACES: StaticCommand = StaticCommand("ATS0\r");
pub const DISABLE_MEMORY: StaticCommand = StaticCommand("ATM0\r");
pub const ENABLE_AUTO_TIMINGS_1: StaticCommand = StaticCommand("ATAT1\r");
pub const SET_CUSTOM_HEADERS: StaticCommand = StaticCommand("ATSH8210F0\r");
pub const ELM_REQUEST_VBAT: StaticCommand = StaticCommand("ATRV\r");


const PID_COMMAND_PADDING: [u8; 7] = [0x32, 0x31, 0x30, 0x30, 0x30, 0x31, 0x0d];

#[repr(u8)]
pub enum PID{
    AvailablePids = 0x00,
    EngineCoolantTemp = 0x05,
    EngineRpm = 0x0c,
}


pub struct PidCommand{
    pub pid: u8,
    pub num_bytes_in_response: usize,
    value_calculation: fn(&[u8]) -> f64,
    pub ascii_command: [u8; 7]
}
pub const fn get_ascii_command(pid: u8) -> [u8; 7] {
    let mut output = PID_COMMAND_PADDING;
    let hex_digit_1: u8 = HexDigits::from_val(pid >> 4) as u8;
    let hex_digit_2: u8 = HexDigits::from_val(pid) as u8;
    output[2] = hex_digit_1;
    output[3] = hex_digit_2;
    output
}
impl PidCommand{

    pub const fn new(pid: u8,
               num_bytes_in_response: usize,
               value_calculation: fn(&[u8]) -> f64
    ) -> Self {
        Self {
            pid,
            num_bytes_in_response,
            value_calculation,
            ascii_command: get_ascii_command(pid),
        }
    }


    pub fn extract_val_from_parsed_resp(&self, response: &[u8]) -> Result<f64, ToRustAGaugeError>{
        let resp_len = response.len();
        if resp_len != self.num_bytes_in_response+6{
            defmt::warn!("UartIncorrectLengthError: {:?}", response);
            return Err(ToRustAGaugeError::UartIncorrectLengthError())
        }
        if response[4] != self.pid{
            defmt::warn!("UartPidMismatchError: {:?}", response);
            return Err(ToRustAGaugeError::UartPidMismatchError())
        }
        let mut actual_sum: u8 = 0;
        for temp_byte in &response[0..resp_len-1]{
            actual_sum = actual_sum.overflowing_add(*temp_byte).0;
        }
        if response[resp_len-1] != actual_sum {
            return Err(ToRustAGaugeError::UartBadChecksumError())
        }

        Ok((self.value_calculation)(&response[5..5+self.num_bytes_in_response]))

    }
}


impl defmt::Format for PidCommand{
    fn format(&self, fmt: Formatter) {
        defmt::write!(fmt, "PidCommand(pid = {:?}, num_resp_bytes = {:?}, ascii_command = {:?})", self.pid, self.num_bytes_in_response, self.ascii_command)
    }
}

pub const ENGINE_RPM_PID: PidCommand = PidCommand::new(
    0x0c,
    2,
    |slice| {
        assert_eq!(slice.len(), 2);
        (slice[0] as f64 * 256.0 + slice[1] as f64) / 4f64
    }
);

pub const ENGINE_COOLANT_TEMP_PID: PidCommand = PidCommand::new(
    0x05,
    1,
    |slice|{
        assert_eq!(slice.len(), 1);
        slice[0] as f64 -40f64
    }
);

pub const HEARTBEAT_PID: PidCommand = PidCommand::new(
    0x00,
    4,
    |_|{ 0f64 } // <- this isn't actually how to interpret the response,
    // but I don't ever use the return in this project
);



#[repr(u8)]
pub enum HexDigits{
    Hex0 = b'0',
    Hex1 = b'1',
    Hex2 = b'2',
    Hex3 = b'3',
    Hex4 = b'4',
    Hex5 = b'5',
    Hex6 = b'6',
    Hex7 = b'7',
    Hex8 = b'8',
    Hex9 = b'9',
    HexA = b'A',
    HexB = b'B',
    HexC = b'C',
    HexD = b'D',
    HexE = b'E',
    HexF = b'F',
}

impl HexDigits{
    pub const fn from_val(value: u8) -> Self{
        use HexDigits::*;
        match value & 0b_0000_1111{
            0x0 => Hex0,
            0x1 => Hex1,
            0x2 => Hex2,
            0x3 => Hex3,
            0x4 => Hex4,
            0x5 => Hex5,
            0x6 => Hex6,
            0x7 => Hex7,
            0x8 => Hex8,
            0x9 => Hex9,
            0xa => HexA,
            0xb => HexB,
            0xc => HexC,
            0xd => HexD,
            0xe => HexE,
            0xf => HexF,
            _ => panic!("this should not be possible, u8 & 00001111 returned a value greater than 15")
        }
    }
}