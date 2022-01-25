
#[repr(u8)]
#[derive(PartialEq, Eq)]
pub enum Opcode {
    Br = 0,
    Add = 1,
    Ld = 2,
    St = 3,
    Jsr = 4,
    And = 5,
    Ldr = 6,
    Str = 7,
    Rti = 8,
    Not = 9,
    Ldi = 10,
    Sti = 11,
    Jmp = 12, // also RET
    Reserved = 13,
    Lea = 14,
    Trap = 15,
}

impl Opcode {
    pub const fn from_int(i: u8) -> Self {
        match Self::try_from_int(i) {
            Ok(op) => op,
            Err(_) => panic!("unknown opcode")
        }
    }

    pub const fn try_from_int(i: u8) -> Result<Self, ()> {
        match i {
            0 => Ok(Opcode::Br),
            1 => Ok(Opcode::Add),
            2 => Ok(Opcode::Ld),
            3 => Ok(Opcode::St),
            4 => Ok(Opcode::Jsr),
            5 => Ok(Opcode::And),
            6 => Ok(Opcode::Ldr),
            7 => Ok(Opcode::Str),
            8 => Ok(Opcode::Rti),
            9 => Ok(Opcode::Not),
            10 => Ok(Opcode::Ldi),
            11 => Ok(Opcode::Sti),
            12 => Ok(Opcode::Jmp), // also RET
            13 => Ok(Opcode::Reserved),
            14 => Ok(Opcode::Lea),
            15 => Ok(Opcode::Trap),
            _ => Err(())
        }
    }
}
