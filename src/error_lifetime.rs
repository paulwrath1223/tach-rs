use core::cmp::Ordering;
use circular_buffer::CircularBuffer;
use arrayvec::ArrayVec;
use defmt::Format;
use crate::errors::{ToRustAGaugeErrorSeverity, ToRustAGaugeErrorWithSeverity};


// This file handles determining which error to render on the display.
// (this does not affect debug probe logs)

const ERROR_LIFETIME: embassy_time::Duration = embassy_time::Duration::from_secs(20); // how many seconds to keep error on the display.
// (this excludes complete failures, which stay forever)

const ERROR_BUF_LEN: usize = 16;

#[derive(Debug, Format)]
pub struct ErrorWithLifetime{
    error_with_severity: ToRustAGaugeErrorWithSeverity,
    time_received: embassy_time::Instant,
}

impl PartialEq for ErrorWithLifetime{
    /// For determining relevance
    fn eq(&self, other: &Self) -> bool{
        self.error_with_severity.severity == other.error_with_severity.severity && 
            self.time_received == other.time_received
    }
}

impl Eq for ErrorWithLifetime{}

impl PartialOrd for ErrorWithLifetime{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.error_with_severity.severity.partial_cmp(&other.error_with_severity.severity).unwrap(){
            Ordering::Equal => {
                self.time_received.partial_cmp(&other.time_received) // time_received is a u64 under the hood
            }
            different => Some(different),
        }
    }
}

impl Ord for ErrorWithLifetime{
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap() // Safety: see partial cmp impl.
    }
}

impl ErrorWithLifetime{
    pub fn new(error: ToRustAGaugeErrorWithSeverity)->Self{
        Self{
            error_with_severity: error,
            time_received: embassy_time::Instant::now(),
        }
    }
    //TODO: make no data less severe and adjust active time for different severities
    
    /// Please drop when not active
    pub fn is_active(&self) -> bool{
        if self.error_with_severity.severity == ToRustAGaugeErrorSeverity::CompleteFailure {
            return true;
        }
        embassy_time::Instant::now().duration_since(self.time_received) < ERROR_LIFETIME
    }
}

#[derive(Debug)]
pub struct ErrorFifo(
    ArrayVec<ErrorWithLifetime, ERROR_BUF_LEN>
);

impl ErrorFifo{
    pub fn new()->Self{
        Self(ArrayVec::new())
    }

    pub fn clear_inactive(&mut self){
        self.0.retain(|x| x.is_active());
    }
    
    pub fn get_most_relevant_error(&self) -> Option<ToRustAGaugeErrorWithSeverity>{
        let mut most_relevant_error: Option<&ErrorWithLifetime> = None;
        for error in self.0.iter() {
            most_relevant_error = match most_relevant_error{
                None => { Some(error) },
                Some(old_relevant) => {
                    Some(core::cmp::max(error, old_relevant))
                }
            }
        };
        let output = most_relevant_error.map(|err|{
            err.error_with_severity.clone()
        });
        output
    }
    
    pub fn add(&mut self, new_error: ToRustAGaugeErrorWithSeverity){
        let mut exists_already: bool = false;
        self.0.iter_mut().for_each(|error|{
            if error.error_with_severity == new_error {
                error.time_received = embassy_time::Instant::now();
                exists_already = true;
            }
        });
        if !exists_already {
            self.0.push(ErrorWithLifetime::new(new_error));
        }
    }
}