use std::env::args;
use std::fs::File;
use std::io::stdin;
use std::io::stdout;

use alic3::emulator::*;

fn main() -> anyhow::Result<()> {
    let args = args().collect::<Vec<String>>();
    if args.len() != 3 {
        return Err(anyhow::anyhow!("Invalid arguments"));
    }

    let os = File::open(&args[1])?;
    let pgm = File::open(&args[2])?;

    crossterm::terminal::enable_raw_mode()?;

    let stdin = stdin();
    let stdout_v = stdout();
    let mut cpu = Cpu::new(stdin, stdout_v);
    cpu.load_program(os)?;
    cpu.load_program(pgm)?;

    cpu.pc = 0x0200;
    loop {
        cpu.step();

        // Exit once the machine control register says to
        if cpu.should_halt() {
            break;
        }
    }

    Ok(())
}
