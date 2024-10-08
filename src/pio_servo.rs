use core::time::Duration;
use embassy_rp::pio::{Common, Config, Direction, Instance, InterruptHandler, Pio, PioPin, StateMachine};
use embassy_rp::{bind_interrupts, clocks};
use embassy_rp::gpio::Level;

use pio::InstructionOperands;

const DEFAULT_MIN_PULSE_WIDTH: u64 = 500; // uncalibrated default, the shortest duty cycle sent to a servo
const DEFAULT_MAX_PULSE_WIDTH: u64 = 2500; // uncalibrated default, the longest duty cycle sent to a servo
const DEFAULT_MAX_DEGREE_ROTATION: ServoDegrees = 270.0;
const REFRESH_INTERVAL: u64 = 20000; // The period of each cycle


pub type ServoDegrees = f64;

pub fn to_pio_cycles(duration: Duration) -> u32 {
    (clocks::clk_sys_freq() / 1_000_000) / 3 * duration.as_micros() as u32 // parentheses are required to prevent overflow
}

pub struct PwmPio<'d, T: Instance, const SM: usize> {
    sm: StateMachine<'d, T, SM>,
}

impl<'d, T: Instance, const SM: usize> PwmPio<'d, T, SM> {
    pub fn new(pio: &mut Common<'d, T>, mut sm: StateMachine<'d, T, SM>, pin: impl PioPin) -> Self {
        let prg = pio_proc::pio_asm!(
            ".side_set 1 opt" // define one optional side set bit for an output pin
                "pull noblock    side 0" // load 32 bits from TX FIFO to OSR (take input from caller). If there is no input, take it from X
                // also set pin low
                "mov x, osr" // copy 32 bits from OSR to X
                "mov y, isr" // copy 32 bits from ISR to Y (ISR contains the period)
            "countloop:"
                "jmp x!=y noset" // jump to `noset` if x != y
                "jmp skip        side 1" // jump to `skip`
                // also set pin high
            "noset:"
                "nop"
            "skip:"
                "jmp y-- countloop" // jump to `countloop` if y is not 0, and then decrement y
        );

        pio.load_program(&prg.program);
        let pin = pio.make_pio_pin(pin);
        sm.set_pins(Level::High, &[&pin]);
        sm.set_pin_dirs(Direction::Out, &[&pin]);

        let mut cfg = Config::default();
        cfg.use_program(&pio.load_program(&prg.program), &[&pin]);

        sm.set_config(&cfg);

        Self { sm }
    }

    pub fn start(&mut self) {
        self.sm.set_enable(true);
    }

    pub fn stop(&mut self) {
        self.sm.set_enable(false);
    }

    pub fn set_period(&mut self, duration: Duration) {
        let is_enabled = self.sm.is_enabled();
        while !self.sm.tx().empty() {} // Make sure that the queue is empty
        self.sm.set_enable(false);
        self.sm.tx().push(to_pio_cycles(duration));
        unsafe {
            self.sm.exec_instr(
                InstructionOperands::PULL {
                    if_empty: false,
                    block: false,
                }
                    .encode(),
            ); // pull result of `to_pio_cycles(duration)` into OSR
            self.sm.exec_instr(
                InstructionOperands::OUT {
                    destination: ::pio::OutDestination::ISR,
                    bit_count: 32,
                }
                    .encode(),
            ); // move value in OSR to ISR, emptying OSR in the process
        };
        if is_enabled {
            self.sm.set_enable(true) // Enable if previously enabled
        }
    }

    pub fn set_level(&mut self, level: u32) {
        self.sm.tx().push(level);
    }

    pub fn write(&mut self, duration: Duration) {
        self.set_level(to_pio_cycles(duration));
    }
}

pub struct ServoBuilder<'d, T: Instance, const SM: usize> {
    pwm: PwmPio<'d, T, SM>,
    period: Duration,
    min_pulse_width: Duration,
    max_pulse_width: Duration,
    max_degree_rotation: ServoDegrees,
}

impl<'d, T: Instance, const SM: usize> ServoBuilder<'d, T, SM> {
    pub fn new(pwm: PwmPio<'d, T, SM>) -> Self {
        Self {
            pwm,
            period: Duration::from_micros(REFRESH_INTERVAL),
            min_pulse_width: Duration::from_micros(DEFAULT_MIN_PULSE_WIDTH),
            max_pulse_width: Duration::from_micros(DEFAULT_MAX_PULSE_WIDTH),
            max_degree_rotation: DEFAULT_MAX_DEGREE_ROTATION,
        }
    }

    pub fn set_period(mut self, duration: Duration) -> Self {
        self.period = duration;
        self
    }

    pub fn set_min_pulse_width(mut self, duration: Duration) -> Self {
        self.min_pulse_width = duration;
        self
    }

    pub fn set_max_pulse_width(mut self, duration: Duration) -> Self {
        self.max_pulse_width = duration;
        self
    }

    pub fn set_max_degree_rotation(mut self, degree: ServoDegrees) -> Self {
        self.max_degree_rotation = degree;
        self
    }

    pub fn build(mut self) -> Servo<'d, T, SM> {
        self.pwm.set_period(self.period);
        Servo {
            pwm: self.pwm,
            min_pulse_width: self.min_pulse_width,
            max_pulse_width: self.max_pulse_width,
            max_degree_rotation: self.max_degree_rotation,
        }
    }
}

pub struct Servo<'d, T: Instance, const SM: usize> {
    pwm: PwmPio<'d, T, SM>,
    min_pulse_width: Duration,
    max_pulse_width: Duration,
    max_degree_rotation: ServoDegrees,
}

impl<'d, T: Instance, const SM: usize> Servo<'d, T, SM> {
    pub fn start(&mut self) {
        self.pwm.start();
    }

    pub fn stop(&mut self) {
        self.pwm.stop();
    }

    pub fn write_time(&mut self, duration: Duration) {
        self.pwm.write(duration);
    }

    pub fn rotate(&mut self, degree: ServoDegrees) {
        let degree_per_nano_second = (self.max_pulse_width.as_nanos() as f64 - self.min_pulse_width.as_nanos() as f64)
            / self.max_degree_rotation;
        
        let nanos = degree * degree_per_nano_second + self.min_pulse_width.as_nanos() as f64;
        
        let mut duration =
            Duration::from_nanos(nanos as u64);
        
        if self.max_pulse_width < duration {
            duration = self.max_pulse_width;
        }

        self.pwm.write(duration);
    }
}
