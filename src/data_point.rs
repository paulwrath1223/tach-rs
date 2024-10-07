use core::fmt::{Debug, Formatter};

#[derive(defmt::Format)]
pub struct DataPoint {
    pub data: Datum,
    pub time: embassy_time::Instant,
}

impl Debug for DataPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "DataPoint: data: {:?}, time received: {:?}", 
               self.data, self.time)
    }
}

#[derive(defmt::Format, Debug)]
pub enum Datum{
    RPM(f64),
    VBat(f64),
    CoolantTempC(f64),
}


// Sane meaning there is any possibility of being accurate
const MIN_SANE_RPM: f64 = -1f64; // this is not even possible for the 
// elm to send and can only mean a bug in this code
const MAX_SANE_RPM: f64 = 32_000f64; // the engine has already exploded if this value is correct
const MIN_SANE_VBAT: f64 = 0f64; // Considering power for this chip comes from VBAT, this cannot be true
const MAX_SANE_VBAT: f64 = 64f64; // doubt the BEC can handle 64v
const MIN_SANE_COOL_TEMP: f64 = -60f64; // this is not even possible for the 
// elm to send and can only mean a bug in this code
const MAX_SANE_COOL_TEMP: f64 = 220f64; // this is not even possible for the 
// elm to send and can only mean a bug in this code



// Normal meaning no cause for concern
const MIN_NORMAL_RPM: f64 = -1f64;
const MAX_NORMAL_RPM: f64 = 7_000f64;
const MIN_NORMAL_VBAT: f64 = 10f64;
const MAX_NORMAL_VBAT: f64 = 16f64;
const MIN_NORMAL_COOL_TEMP: f64 = -30f64;
const MAX_NORMAL_COOL_TEMP: f64 = 100f64;


impl Datum{
    pub fn is_value_sane_check(&self) -> bool{
        match self {
            Datum::RPM(value) => is_rpm_sane_check(*value),
            Datum::VBat(value) => value.is_finite() && value < &MAX_SANE_VBAT && value > &MIN_SANE_VBAT,
            Datum::CoolantTempC(value) => value.is_finite() && value < &MAX_SANE_COOL_TEMP && value > &MIN_SANE_COOL_TEMP,
        }
    }
    
    pub fn is_value_normal(&self) -> bool {
        match self {
            Datum::RPM(value) => is_rpm_normal_check(*value),
            Datum::VBat(value) => value.is_finite() && value < &MAX_NORMAL_VBAT && value > &MIN_NORMAL_VBAT,
            Datum::CoolantTempC(value) => value.is_finite() && value < &MAX_NORMAL_COOL_TEMP && value > &MIN_NORMAL_COOL_TEMP,
        }
    }
}


pub fn is_rpm_sane_check(rpm_val: f64) -> bool{
    rpm_val.is_finite() && rpm_val < MAX_SANE_RPM && rpm_val > MIN_SANE_RPM
}

pub fn is_rpm_normal_check(rpm_val: f64) -> bool {
    rpm_val.is_finite() && rpm_val < MAX_NORMAL_RPM && rpm_val > MIN_NORMAL_RPM
}