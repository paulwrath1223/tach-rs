use core::cmp::Ordering;
use circular_buffer::CircularBuffer;
use crate::errors::{ToRustAGaugeError, ToRustAGaugeErrorSeverity, ToRustAGaugeErrorWithSeverity};


// This file handles determining which error to render on the display.
// (this does not affect debug probe logs)

const ERROR_LIFETIME_SECONDS: u64 = 20; // how many seconds to keep error on the display.
// (this excludes complete failures, which stay forever)

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

    /// Please drop when not active
    pub fn is_active(&self) -> bool{
        if self.error_with_severity.severity == ToRustAGaugeErrorSeverity::CompleteFailure {
            return true;
        }
        embassy_time::Instant::now() - self.time_received > embassy_time::Duration::from_secs(ERROR_LIFETIME_SECONDS)
    }
}

pub struct ErrorFifo(CircularBuffer<10, ErrorWithLifetime>);

impl ErrorFifo{
    pub fn new()->Self{
        ErrorFifo(CircularBuffer::<10, ErrorWithLifetime>::new())
    }

    pub fn clear_inactive(&mut self){
        for index in 0..self.0.len(){
            if !self.0[index].is_active(){
                self.0.remove(index);
            }
        }
    }
    
    pub fn get_most_relevant_error(&self) -> Option<ToRustAGaugeErrorWithSeverity>{
        let mut most_relevant_error: Option<&ErrorWithLifetime> = None;
        self.0.iter().for_each(|error|{
            most_relevant_error = match most_relevant_error{
                None => { Some(error) },
                Some(old_relevant) => {
                    Some(core::cmp::max(error, old_relevant))
                }
            }
        });
        most_relevant_error.map(|err|{
            err.error_with_severity.clone()
        })
    }
    
    pub fn add_and_update(&mut self, error: ToRustAGaugeErrorWithSeverity){
        self.clear_inactive();
        self.0.push_back(ErrorWithLifetime::new(error));
    }
}