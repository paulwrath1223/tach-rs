use core::fmt::{Debug, Formatter};

#[derive(defmt::Format)]
pub struct DataPoint {
    pub rpm: Option<f64>,
    pub vbat: Option<f64>,
    pub coolant_temp_c: Option<f64>,
    pub time: embassy_time::Instant,
}

impl Debug for DataPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "DataPoint: rpm: {:?}, vbat: {:?}, coolant temp: {:?}, time received: {:?}", 
               self.rpm, self.vbat, self.coolant_temp_c, self.time)
    }
}