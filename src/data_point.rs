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