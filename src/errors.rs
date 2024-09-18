use core::fmt::{Debug, Formatter};
use thiserror_no_std::Error;

#[derive(Error, Debug, defmt::Format)]
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
}

#[repr(u8)]
#[derive(Debug, defmt::Format)]
pub enum ToRustAGaugeErrorSeverity {
    CompleteFailure,
    LossOfSomeFunctionality,
    BadIfReoccurring,
    EntirelyRecoverable,
    MaybeRecoverable,
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

#[derive(defmt::Format)]
pub struct ToRustAGaugeErrorWithSeverity {
    error: ToRustAGaugeError,
    severity: ToRustAGaugeErrorSeverity,
}

impl Debug for ToRustAGaugeErrorWithSeverity {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "Error: {:?} with severity: {:?}", self.error, self.severity)
    }
}