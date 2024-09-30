use core::mem::{self, MaybeUninit};
use embassy_rp::pio::{Common, Config, Direction, Instance, InterruptHandler, Irq, Pio, PioPin, StateMachine};
use fixed::FixedU32;
use fixed::traits::ToFixed;
use fixed::types::extra::U8;

// I don't understand anything in this file, taken from the Embassy examples

pub struct PioStepper<'d, T: Instance, const SM: usize> {
    irq: Irq<'d, T, SM>,
    sm: StateMachine<'d, T, SM>,
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
            // the step pattern is in 4 byte chunks, so here we execute one step
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
        Self { irq, sm }
    }

    /// Set pulse frequency
    pub fn set_frequency(&mut self, freq: u32) {
        let clock_divider: FixedU32<U8> = (125_000_000 / (freq * 136)).to_fixed();
        assert!(clock_divider <= 65536, "clkdiv must be <= 65536");
        assert!(clock_divider >= 1, "clkdiv must be >= 1");
        self.sm.set_clock_divider(clock_divider);
        self.sm.clkdiv_restart();
    }

    // Full step, one phase
    pub async fn step(&mut self, steps: i32) {
        if steps > 0 {
            self.run(steps, 0b1000_0100_0010_0001_1000_0100_0010_0001).await
        } else {
            self.run(-steps, 0b0001_0010_0100_1000_0001_0010_0100_1000).await
        }
    }

    // Full step, two phase
    pub async fn step2(&mut self, steps: i32) {
        if steps > 0 {
            self.run(steps, 0b1001_1100_0110_0011_1001_1100_0110_0011).await
        } else {
            self.run(-steps, 0b0011_0110_1100_1001_0011_0110_1100_1001).await
        }
    }

    // Half step
    pub async fn step_half(&mut self, steps: i32) {
        if steps > 0 {
            self.run(steps, 0b1001_1000_1100_0100_0110_0010_0011_0001).await
        } else {
            self.run(-steps, 0b0001_0011_0010_0110_0100_1100_1000_1001).await
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
                    }
                        .encode(),
                );
            }
        });
        self.irq.wait().await; // wait for the irq to get set again (happens at end of PIO prog)
        drop.defuse();
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