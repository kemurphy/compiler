use mc::ast::{Visitor, UnOpNode, BinOpNode};
use util::{Width, Name};

use collections::TreeSet;

struct IRConverter {
    next_temp: uint,
    next_label: uint,
    ident_temps: TreeMap<NodeId, Temp>,
}

struct Temp(uint);

enum Label {
    NumLabel(u32),
    NameLabel(Name),
}

enum Dest {
    Local(Temp, uint),
}

enum Src {
    Ref(Dest),
    Imm(u32),
}

enum Cond {
    Always,
    Cmp(Src, Cond, Src),
}

impl Dest {
    fn size(&self) -> uint {
        match *self {
            Local(_, s) => s
        }
    }
}

pub enum Instr {
    BinOp(Dest, BinOpNode, Src, Src),
    Call(Dest, Label, Vec<Src>),
    Store(Dest, Src, Width),
    Load(Dest, Src, Width),
    Label(Label),
    Jump(Cond, Label, Label),
    Return(Src),
    Phi(Dest, Vec<Src>, Vec<Label>),
    Nop,
}

pub struct Sub {
    body: Vec<Instr>,
    args: Vec<Dest>,
    min_temp: Temp,
    max_temp: Temp,
}

impl IRConverter {
    fn new_temp(&mut self) {
        let t = Temp(self.next_temp);
        self.next_temp += 1;
        t
    }

    fn new_dest(&mut self, size: uint) {
        Local(self.new_temp(), size)
    }

    fn temp_for(&mut self, ident: &Ident) {
        match self.ident_temps.find(&ident.id) {
            Some(t) => t,
            None => {
                let t = self.new_temp();
                self.ident_temps.insert(ident.id, t);
                t
            }
        }
    }
}

impl Visitor for IRConverter {
}
