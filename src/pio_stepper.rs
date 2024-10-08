use core::mem::{self, MaybeUninit};
use embassy_rp::pio::{Common, Config, Direction, Instance, InterruptHandler, Irq, Pio, PioPin, StateMachine};
use fixed::FixedU32;
use fixed::traits::ToFixed;
use fixed::types::extra::U8;


pub struct PioStepper<'d, T: Instance, const SM: usize> {
    irq: Irq<'d, T, SM>,
    sm: StateMachine<'d, T, SM>,
    current_position: Option<u32>, // none if uncalibrated
    max_steps: u32,
}

impl<'d, T: Instance, const SM: usize> PioStepper<'d, T, SM> {
    
    pub fn new(
        pio: &mut Common<'d, T>,
        mut sm: StateMachine<'d, T, SM>,
        irq: Irq<'d, T, SM>,
        pin0: impl PioPin,
        pin1: impl PioPin,
        pin2: impl PioPin,
        pin3: impl PioPin,
        max_steps: u32,
    ) -> Self {
        let prg = pio_proc::pio_asm!(
            "pull block", // pull 32 bits from fifo reg, blocking if empty
            "mov x, osr", // copy data from OSR (Output Shift Register) to scratch X.
            // Scratch X now contains number of steps to execute
            "pull block", // pull 32 bits from fifo reg, blocking if empty
            "mov y, osr", // copy data from OSR to scratch Y.
            // Scratch Y now contains the pattern to use
            "jmp !x end", // jump to end if scratch x is 0
            // end if 'steps to take' is 0
            "loop:", // jump label
            "jmp !osre step", // jump to 'step' if OSR is not empty
            "mov osr, y", // copy data from Scratch Y to OSR
            // this loads the pattern into the OSR if it is empty
            "step:", // jump label
            "out pins, 4 [31]" // shift 4 bits from OSR to pins and then delay 31 cycles
            // the step pattern is in 4 bit chunks, so here we execute one step
            "jmp x-- loop", // jump to 'loop' if scratch X is not 0. and the decrement scratch X
            // step again if 'steps to take' is not 0;
            "end:", // jump label
            "irq 0 rel" // set IRQ that has the same number as the current state machine
        );
        let pin0 = pio.make_pio_pin(pin0);
        let pin1 = pio.make_pio_pin(pin1);
        let pin2 = pio.make_pio_pin(pin2);
        let pin3 = pio.make_pio_pin(pin3);
        sm.set_pin_dirs(Direction::Out, &[&pin0, &pin1, &pin2, &pin3]);
        let mut cfg = Config::default();
        cfg.set_out_pins(&[&pin0, &pin1, &pin2, &pin3]);
        cfg.clock_divider = (125_000_000 / (100 * 136)).to_fixed();
        cfg.use_program(&pio.load_program(&prg.program), &[]);
        sm.set_config(&cfg);
        sm.set_enable(true);
        Self { irq, sm, current_position: None, max_steps }
    }

    /// Set pulse frequency
    pub fn set_frequency(&mut self, freq: u32) {
        let clock_divider: FixedU32<U8> = (125_000_000 / (freq * 136)).to_fixed();
        assert!(clock_divider <= 65536, "clkdiv must be <= 65536");
        assert!(clock_divider >= 1, "clkdiv must be >= 1");
        self.sm.set_clock_divider(clock_divider);
        self.sm.clkdiv_restart();
    }

    pub async fn step_double(&mut self, steps: i32) {
        if steps > 0 {
            self.run(steps*4, 0b1010_0110_0101_1001_1010_0110_0101_1001).await
        } else {
            self.run(-steps*4, 0b1001_0101_0110_1010_1001_0101_0110_1010).await
        }
    }

    async fn run(&mut self, steps: i32, pattern: u32) {
        self.sm.tx().wait_push(steps as u32).await; // send 'steps to take' to pio
        self.sm.tx().wait_push(pattern).await; // send the pattern to pio
        let drop = OnDrop::new(|| { // this drop stuff means when you drop the future, it will stop executing safely and reset itself
            self.sm.clear_fifos();
            unsafe {
                self.sm.exec_instr(
                    pio::InstructionOperands::JMP {
                        address: 0,
                        condition: pio::JmpCondition::Always,
                    }.encode(),
                );
            }
        });
        self.irq.wait().await; // wait for the irq to get set again (happens at end of PIO prog)
        drop.defuse();
    }
    
    pub async fn calibrate(&mut self){
        self.step_double(-175).await;
        self.step_double(1).await;
        self.current_position = Some(0);
    }

    /// ! dropping this future will cause a disconnect between the actual and internal position of the stepper 
    pub async fn set_position(&mut self, target_position: u32){
        let delta: i32 = target_position as i32 - self.current_position
            .expect("tried to set stepper pos before calibration") as i32;
        self.current_position = Some(target_position);
        self.step_double(delta).await;
    }

    /// if this future is dropped, the motor must be recalibrated
    pub async fn set_position_from_val(&mut self, value: f64){
        let scaled_value = (self.max_steps * value as u32 / 9000).clamp(0, self.max_steps);
        self.set_position(scaled_value).await;
    }
}

struct OnDrop<F: FnOnce()> {
    f: MaybeUninit<F>,
}

impl<F: FnOnce()> OnDrop<F> {
    pub fn new(f: F) -> Self {
        Self { f: MaybeUninit::new(f) }
    }

    pub fn defuse(self) {
        mem::forget(self)
    }
}

impl<F: FnOnce()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        unsafe { self.f.as_ptr().read()() }
    }
}