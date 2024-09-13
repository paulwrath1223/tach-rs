pub struct StaticCommand(&'static str);

impl StaticCommand {
    fn as_bytes<'a>(&self) -> &[u8] {
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
pub const REQUEST_ECU_RPM: StaticCommand = StaticCommand("210C01\r");
pub const REQUEST_ECU_COOLANT: StaticCommand = StaticCommand("210501\r");
pub const HEARTBEAT_AVAILABLE_PIDS: StaticCommand = StaticCommand("210001\r");