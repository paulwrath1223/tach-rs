use core::cmp::Ordering;
use core::fmt::{Debug, Formatter};
use thiserror_no_std::Error;


// TODO: WTF is this file. Valve pls fix
/// macro would be nice here, but rust macro language is on the same spiritual level as [DreamBerd](https://github.com/TodePond/DreamBerd)

#[derive(Error, Debug, defmt::Format, PartialEq, Clone)]
pub enum ToRustAGaugeError {
    #[error("Nondescript error")]
    NondescriptError(),
    #[error("Embassy uart error")]
    UartError(#[from] embassy_rp::uart::Error),
    #[error("Embassy uart timeout error")]
    UartTimeoutError(#[from] embassy_time::TimeoutError),
    #[error("Embassy buffer overflow error. Attempted to read until a delimiter, \
    but read LOCAL_RX_BUFFER_LEN (currently 256, but subject to change) bytes with no delimiter.")]
    UartBufferOverflowError(),
    #[error("Failed to parse bytes from UART.")]
    UartByteParseError(),
    #[error("Response from ELM failed checksum test")]
    UartBadChecksumError(),
    #[error("Response from ELM was not the expected length")]
    UartIncorrectLengthError(),
    #[error("Response from ELM did not match the requested PID")]
    UartPidMismatchError(),
    #[error("Failed to parse voltage from ELM")]
    UartVoltageParseError(),
    #[error("Error communicating with LCD")]
    MipiDsiError(),
    #[error("RPM data does NOT pass the vibe check")]
    UnreliableRPM(),
    #[error("VBAT data does NOT pass the vibe check")]
    UnreliableVBAT(),
    #[error("Coolant data does NOT pass the vibe check")]
    UnreliableCoolant(),
    
}

const NONDESCRIPT_ERROR_STR: &'static str =           "non-descr- \nipt error! \n   :(      \n   :(      ";
const UART_ERROR_STR: &'static str =                  "UART had an\ninternal   \nerror.     \n(hardware) ";
const UART_TIMEOUT_ERROR_STR: &'static str =          "UART timed \nout waiting\n           \n           ";
const UART_BUFFER_OVERFLOW_ERROR_STR: &'static str =  "UART soft- \nware buffer\noverflowed!\n           ";
const UART_BYTE_PARSE_ERROR_STR: &'static str =       "UART soft- \nware failed\nto parse in\ncoming byte";
const UART_BAD_CHECKSUM_ERROR_STR: &'static str =     "UART soft- \nware failed\nto verify  \nchecksum   ";
const UART_INCORRECT_LENGTH_ERROR_STR: &'static str = "UART resp. \nincluded   \nwrong num  \nof bytes   ";
const UART_PID_MISMATCH_ERROR_STR: &'static str =     "UART resp. \nincluded   \nwrong PID  \n           ";
const UART_VOLTAGE_PARSE_ERROR_STR: &'static str =    "UART soft- \nware failed\nto parse   \nvoltage!   ";
const MIPI_DSI_ERROR_STR: &'static str =              "LCD Error! \nSPI commun-\nication    \nfailure!   ";
const UNRELIABLE_RPM: &'static str =                  "Unreliable \nRPM data!  \nCaution!   \n           ";
const UNRELIABLE_VBAT: &'static str =                 "Unreliable \nVBAT data! \nCaution!   \n           ";
const UNRELIABLE_COOLANT: &'static str =              "Unreliable \nTemp data! \nCaution!   \n           ";



impl ToRustAGaugeError{

    pub fn to_str(&self) -> &'static str {
        match self {
            ToRustAGaugeError::NondescriptError() => { NONDESCRIPT_ERROR_STR }
            ToRustAGaugeError::UartError(_) => { UART_ERROR_STR }
            ToRustAGaugeError::UartTimeoutError(_) => { UART_TIMEOUT_ERROR_STR }
            ToRustAGaugeError::UartBufferOverflowError() => { UART_BUFFER_OVERFLOW_ERROR_STR }
            ToRustAGaugeError::UartByteParseError() => { UART_BYTE_PARSE_ERROR_STR }
            ToRustAGaugeError::UartBadChecksumError() => { UART_BAD_CHECKSUM_ERROR_STR }
            ToRustAGaugeError::UartIncorrectLengthError() => { UART_INCORRECT_LENGTH_ERROR_STR }
            ToRustAGaugeError::UartPidMismatchError() => { UART_PID_MISMATCH_ERROR_STR }
            ToRustAGaugeError::UartVoltageParseError() => { UART_VOLTAGE_PARSE_ERROR_STR }
            ToRustAGaugeError::MipiDsiError() => { MIPI_DSI_ERROR_STR }
            ToRustAGaugeError::UnreliableRPM() => { UNRELIABLE_RPM }
            ToRustAGaugeError::UnreliableVBAT() => { UNRELIABLE_VBAT }
            ToRustAGaugeError::UnreliableCoolant() => { UNRELIABLE_COOLANT }
        }
    }
}

#[repr(u8)]
#[derive(Debug, defmt::Format, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum ToRustAGaugeErrorSeverity {
    CompleteFailure = 10,
    LossOfSomeFunctionality = 9,
    MaybeRecoverable = 8,
    BadIfReoccurring = 7,
    EntirelyRecoverable = 2,
}


impl ToRustAGaugeErrorWithSeverity {
    pub fn from_with_severity<E>(error: E, severity: ToRustAGaugeErrorSeverity) -> Self 
    where E: Into<ToRustAGaugeError> 
    {
        ToRustAGaugeErrorWithSeverity{
            error: E::into(error),
            severity,
        }
    }
}

#[derive(defmt::Format, PartialEq, Clone)]
pub struct ToRustAGaugeErrorWithSeverity {
    pub error: ToRustAGaugeError,
    pub severity: ToRustAGaugeErrorSeverity,
}

impl Debug for ToRustAGaugeErrorWithSeverity {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "Error: {:?} with severity: {:?}", self.error, self.severity)
    }
}