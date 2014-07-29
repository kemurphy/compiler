use ir::*;
use ir::liveness::LivenessAnalyzer;
use std::collections::{TreeMap, TreeSet};
use util::Name;
use mc::ast::AddrOf;
use codegen::*;
use mas::ast::Reg;

pub struct ConflictAnalyzer;

impl ConflictAnalyzer {
    /// Analyze a block of IR code, and return a map of conflicting registers
    /// as well as how many times each variable is used.
    /// Also return a list of variables that must be in memory rather than
    /// in registers, because they are referenced.
    /// Assumes the IR is already in SSA form.
    pub fn conflicts(ops: &Vec<Op>) -> (TreeMap<Var, TreeSet<Var>>,
                                        TreeMap<Var, u32>,
                                        TreeMap<Var, RegisterColor>,
                                        // This is a set of Names, because
                                        // we spill *every* generation of
                                        // the variable.
                                        TreeSet<Name>) {
        let opinfo = LivenessAnalyzer::analyze(ops);
        let mut conflict_map = TreeMap::<Var, TreeSet<Var>>::new();
        let mut counts = TreeMap::<Var, u32>::new();
        let mut referenced_vars = TreeSet::<Name>::new();
        let mut must_colors = TreeMap::new();

        for op in ops.iter() {
            match *op {
                UnOp(_, AddrOf, ref rve) => {
                    match *rve {
                        Variable(ref v) => { referenced_vars.insert(v.name); },
                        _ => fail!("Should have a variable here."),
                    }
                },
                Call(ref v, _, ref args) => {
                    for (i, arg) in args.iter().enumerate()
                        .take(num_param_regs)
                    {
                        must_colors.insert(*arg,
                                           RegColor(Reg { index: i as u8 }));
                    }
                    must_colors.insert(*v, RegColor(Reg { index: 0 as u8 }));
                },
                Func(_, ref args) => {                    
                    for (i, arg) in args.iter().enumerate()
                        .take(num_param_regs)
                    {
                        must_colors.insert(*arg,
                                           RegColor(Reg { index: i as u8 }));
                    }
                    for i in range(num_param_regs, args.len()) {
                        must_colors.insert(args[i],
                                           StackColor((i as int) - 1 -
                                                      (args.len() as int)));
                    }
                }
                _ => {}
            }
        }

        for info in opinfo.iter() {
            for var1 in info.live.iter() {
                for var2 in info.live.iter() {
                    if var1 != var2 {
                        if !conflict_map.contains_key(var1) {
                            conflict_map.insert(*var1, TreeSet::<Var>::new());
                        }
                        if !conflict_map.contains_key(var2) {
                            conflict_map.insert(*var2, TreeSet::<Var>::new());
                        }

                        conflict_map.find_mut(var1).unwrap().insert(*var2);
                        conflict_map.find_mut(var2).unwrap().insert(*var1);
                    }
                }
            }

            for used_var in info.used.iter().chain(info.def.iter()) {
                let new_count = counts.find(used_var).unwrap_or(&0) + 1;
                counts.insert(*used_var, new_count);
            }
        }

        (conflict_map, counts, must_colors, referenced_vars)
    }
}
