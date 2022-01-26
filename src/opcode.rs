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
    Jmp = 12,
    Reserved = 13,
    Lea = 14,
    Trap = 15,
}

impl Opcode {
    pub const fn from_int(i: u8) -> Self {
        match i {
            0 => Self::Br,
            1 => Self::Add,
            2 => Self::Ld,
            3 => Self::St,
            4 => Self::Jsr,
            5 => Self::And,
            6 => Self::Ldr,
            7 => Self::Str,
            8 => Self::Rti,
            9 => Self::Not,
            10 => Self::Ldi,
            11 => Self::Sti,
            12 => Self::Jmp,
            13 => Self::Reserved,
            14 => Self::Lea,
            15 => Self::Trap,
            _ => panic!("invalid opcode"),
        }
    }

    pub const fn to_int(&self) -> u16 {
        match self {
            Self::Br => 0,
            Self::Add => 1,
            Self::Ld => 2,
            Self::St => 3,
            Self::Jsr => 4,
            Self::And => 5,
            Self::Ldr => 6,
            Self::Str => 7,
            Self::Rti => 8,
            Self::Not => 9,
            Self::Ldi => 10,
            Self::Sti => 11,
            Self::Jmp => 12,
            Self::Reserved => 13,
            Self::Lea => 14,
            Self::Trap => 15,
        }
    }
}
