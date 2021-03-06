use time::precise_time_ns;

use package::Package;

use mc::lexer::Lexer;
use mc::parser::Parser;
use mc::session::Interner;

use util::{IntKind, Name};
use util::IntKind::{GenericInt, SignedInt, UnsignedInt};
use util::Width::{Width32, Width16, Width8, AnyWidth};

use ir::liveness::{LivenessAnalyzer, get_liveness_times};
use ir::ast_to_intermediate::ASTToIntermediate;
use ir::constant_fold::ConstantFolder;
use ir::ssa::{ToSSA, get_times, get_param_times};

use super::{MkTarget,Target};

use target::util::NameMangler;

use mc::ast::*;
use ir::*;

use std::collections::{BTreeMap, BTreeSet};
use std::iter::FromIterator;
use std::io::Write;

pub struct IRTarget {
    verbose: bool,
}

fn is_function(global_map: &BTreeMap<Name, StaticIRItem>,
               v: &Var) -> bool {
    match global_map.get(&v.name) {
        Some(ref i) => i.is_func,
        None => false
    }
}

fn print_var(interner: &Interner,
             global_map: &BTreeMap<Name, StaticIRItem>,
             v: &Var) -> String {
    format!("{}{}",
            interner.name_to_str(&v.name),
            match v.generation {
                Some(ref g) if *g > 0 =>
                    if global_map.get(&v.name).is_none() {
                        format!("_{}", g)
                    } else {
                        format!("")
                    },
                _ => format!(""),
            }
            )
}

fn print_lvalue(interner: &Interner,
                global_map: &BTreeMap<Name, StaticIRItem>,
                lv: &LValue) -> String {
    match *lv {
        VarLValue(ref v) => print_var(interner, global_map, v),
        PtrLValue(ref v) => format!("*(int*){}", print_var(interner,
                                                           global_map, v)),
    }
}

fn print_rvalelem(interner: &Interner,
                  global_map: &BTreeMap<Name, StaticIRItem>,
                  rve: &RValueElem) -> String {
    match *rve {
        Variable(ref v) => print_var(interner, global_map, v),
        Constant(ref l) => {
            match *l {
                NumLit(n, ik) => ik.num_to_string(n),
                NullLit => format!("NULL"),
                BoolLit(true) => format!("1"),
                BoolLit(false) => format!("0"),
                StringLit(ref s) => {
                    let parts: Vec<String> = (&s[..]).bytes()
                        .map(|b: u8|format!("\\x{:02x}", b))
                        .collect();
                    format!("\"{}\"", parts.concat())
                },
            }
        }
    }
}

fn assign_vars(interner: &Interner,
               global_map: &BTreeMap<Name, StaticIRItem>,
               label: &BTreeMap<Name, usize>,
               vars: &BTreeSet<Var>) -> String {
    let mut s = "".to_string();
    for var in vars.iter() {
        let new_var = Var {
            name: var.name.clone(),
            generation: Some(*label.get(&var.name).unwrap())
        };
        match global_map.get(&var.name) {
            // We don't want to do assignments when global functions are
            // involved.
            Some(ref i) if i.is_func => {},
            _ =>
                s = s +
                    &format!("  {} = {};\n",
                            print_var(interner, global_map, &new_var),
                            print_var(interner, global_map, var))[..],
        }
    }
    s
}

impl IRTarget {
    fn convert_function(&self, interner: &Interner,
                        ops: &Vec<Op>,
                        global_map: &BTreeMap<Name, StaticIRItem>) -> String {
        // If this is the case, it's an extern. We don't want to emit it.
        if ops.len() <= 1 { return "".to_string() }

        let opinfo = LivenessAnalyzer::unanalyzed_opinfo(ops);
        let mut s = "".to_string();
        let mut vars = BTreeSet::new();
        let mut labels: BTreeMap<usize, BTreeMap<Name, usize>> = BTreeMap::new();
        // Find all variables we need to declare. This is all variables
        // that are defined anywhere, except in the very first instruction
        // (which must be a function definition instruction).
        for op in opinfo.iter().skip(1) {
            for var in op.def.iter() {
                vars.insert(var);
            }
        }
        // TODO: This is a hack, until "return" takes an option.
        for op in ops.iter() {
            match op.val {
                OpNode::Return(Variable(ref v)) => { vars.insert(v); },
                _ => {}
            }
        }

        // Find out which variables are used at each label.
        for op in ops.iter() {
            match op.val {
                OpNode::Label(ref idx, ref vars) => {
                    let mut varmap: BTreeMap<Name, usize> = BTreeMap::new();
                    for var in vars.iter() {
                        varmap.insert(var.name.clone(),
                                      var.generation.unwrap());
                    }
                    labels.insert(*idx, varmap);
                }
                _ => {}
            }
        }

        let mut in_func = false;

        // Do the actual conversion.
        for op in ops.iter() {
            match op.val {
                OpNode::Nop => {},
                _ => s = s + &format!("  // {}", op)[..]
            }
            s = s + &(match op.val {
                OpNode::BinOp(ref v, ref op, ref rv1, ref rv2, signed) => {
                    let cast = if signed {
                        "(long)"
                    } else {
                        "(unsigned long)"
                    };
                    format!("  {} = (long)(({}{}) {} ({}{}));\n",
                            print_var(interner, global_map, v),
                            cast,
                            print_rvalelem(interner, global_map, rv1),
                            op,
                            cast,
                            print_rvalelem(interner, global_map, rv2))
                },
                OpNode::UnOp(ref v, ref op, ref rv) => {
                    match *op {
                        Deref =>
                            format!("  {} = (long)({} (long*)({}));\n",
                                    print_var(interner, global_map, v),
                                    op,
                                    print_rvalelem(interner, global_map, rv)),
                        SxbOp |
                        SxhOp =>
                            format!("  {} = (long)({} ({}));\n",
                                    print_var(interner, global_map, v),
                                    if *op == SxbOp {
                                        "(int8_t)"
                                    } else {
                                        "(int16_t)"
                                    },
                                    print_rvalelem(interner, global_map, rv)),
                        _ =>
                            format!("  {} = (long)({} ({}));\n",
                                    print_var(interner, global_map, v),
                                    op,
                                    print_rvalelem(interner, global_map, rv))
                    }
                },
                OpNode::Alloca(ref v, ref size) => {
                    format!("  {} = (long)(alloca({}));\n",
                            print_var(interner, global_map, v), size)
                },
                OpNode::Call(ref v, ref fname, ref args) => {
                    let mut s = match *fname {
                        Variable(ref fnv) =>
                            if is_function(global_map, fnv) {
                                format!("  {} = (long){}(",
                                    print_var(interner, global_map, v),
                                    print_var(interner, global_map, fnv),
                                    )
                            } else {
                                format!("  {} = ((long (*)()){})(",
                                        print_var(interner, global_map, v),
                                        print_var(interner, global_map, fnv),
                                        )
                            },
                        _=> unimplemented!(),
                    };
                    let list: Vec<String> = args.iter()
                        .map(|arg| print_var(interner, global_map, arg)
                             ).collect();
                    s = s + &list.join(", ")[..];
                    s = s + ");\n";
                    s
                },
                OpNode::Load(ref l, ref r, ref size) => {
                    format!("  {} = (long)*({}*)({});\n",
                            print_var(interner, global_map, l),
                            match *size {
                                AnyWidth |
                                Width32 => "uint32_t",
                                Width16 => "uint16_t",
                                Width8 => "uint8_t",
                            },
                            print_var(interner, global_map, r))
                },
                OpNode::Store(ref l, ref r, ref size) => {
                    format!("  *({}*)({}) = {};\n",
                            match *size {
                                AnyWidth |
                                Width32 => "uint32_t",
                                Width16 => "uint16_t",
                                Width8 => "uint8_t",
                            },
                            print_var(interner, global_map, l),
                            print_var(interner, global_map, r))
                },
                OpNode::Nop => format!(""),
                OpNode::Label(ref l, _) => {
                    // TODO: correct assignments of variables in labels and
                    // gotos.
                    format!("LABEL{}:\n", l)
                },
                OpNode::Goto(ref l, ref vars) => {
                    format!("{}  goto LABEL{};\n",
                            assign_vars(interner,
                                        global_map,
                                        labels.get(l).unwrap(),
                                        vars),
                            l)
                },
                OpNode::CondGoto(ref negated, ref rve, ref l, ref vars) => {
                    format!("  if ({}({})) {{\n  {}\n  goto LABEL{}; }}\n",
                            if *negated { "!" } else { "" },
                            print_rvalelem(interner, global_map, rve),
                            assign_vars(interner,
                                        global_map,
                                        labels.get(l).unwrap(),
                                        vars),
                            l)
                },
                OpNode::Return(ref rve) => {
                    format!("  return {};\n", print_rvalelem(interner,
                                                             global_map,
                                                             rve))
                },
                OpNode::Func(ref name, ref args, _) => {
                    let mapped_args: Vec<String> = args.iter()
                        .map(|arg| format!("long {}", print_var(interner,
                                                                global_map,
                                                                arg)))
                        .collect();
                    let mut s = format!("");
                    for var in vars.iter() {
                        if global_map.get(&var.name).is_none() {
                            s = s + &format!("  long {};\n",
                                         print_var(interner,
                                                   global_map,
                                                   *var))[..];
                        }
                    }

                    let closer = if in_func { "}" } else { "" };

                    in_func = true;

                    format!("{}\nlong {}({}) {{\n{}",
                            closer,
                            interner.name_to_str(name),
                            mapped_args.join(", "),
                            s)
                },
                OpNode::AsmOp(..) => panic!("Inline ASM not supported in IR-C target."),
            }[..]);
        }
        s + "}\n"
    }
}

impl MkTarget for IRTarget {
    fn new(args: &Vec<(String, Option<String>)>) -> Box<IRTarget> {
        let mut verbose = false;
        for arg in args.iter() {
            if arg.0 == "verbose".to_string() {
                print!("Enabling verbose mode.\n");
                verbose = true;
            }
        }
        Box::new(IRTarget { verbose: verbose })
    }
}
impl Target for IRTarget {
    #[allow(unused_must_use)]
    fn compile(&self, p: Package, f: &mut Write) {
        let Package {
            module,
            session,
            mut typemap,
        } = p;

        let mangler = NameMangler::new(session, &module, true, false);
        let mut session = mangler.session;
        let mut sourcemap = BTreeMap::<NodeId, NodeId>::new();

        let (mut result, staticitems) = {
            let mut converter = ASTToIntermediate::new(&mut session,
                                                       &mut typemap,
                                                       &mangler.names,
                                                       &mut sourcemap);

            if self.verbose {
                print!("{:?}\n", module);
            }

            converter.convert_module(&module)
        };

        if self.verbose {
            for res in result.iter() {
                print!("{:?}\n\n", res);
            }
        }

        // I wish that this was actually the same as the ccross one, but it differs slightly.
        // We could probably get this merged.
        writeln!(f, "{}", "#pragma GCC diagnostic ignore \"-Wunused-but-set-variable\"");
        writeln!(f, "{}", "#include <stdint.h>");
        writeln!(f, "{}", "#include <stdlib.h>");
        writeln!(f, "{}", "typedef unsigned int uint_t;");
        writeln!(f, "{}", "typedef int int_t;");

        writeln!(f, "{}", "#ifndef MB_FREESTANDING");
        writeln!(f, "{}", "#include <stdio.h>");
        writeln!(f, "{}", "#include <string.h>");

        writeln!(f, "{}", "long print_int(long x) { printf(\"%d\\n\", (int)x); return x; }");
        writeln!(f, "{}", "long print_char(long x) { printf(\"%c\", (int)x); return x; }");
        writeln!(f, "{}", "long rt_memcpy(long dest, long src, long n) { return (long)memcpy((void*)dest, (void*)src, n); }");
        writeln!(f, "{}", "extern void abort();");
        writeln!(f, "{}", "long rt_abort() { abort(); return 0; }");
        writeln!(f, "{}", "long rt_malloc(long size) { return (long)malloc(size); }");
        writeln!(f, "{}", "#else");
        writeln!(f, "{}", "long print_int(long x) { return x; }");
        writeln!(f, "{}", "long print_char(long x) { return x; }");
        writeln!(f, "{}", "long rt_memcpy(long dest, long src, long n) { char *pdest = (char*)dest; char *psrc = (char*)src; while (n--) *pdest++ = *psrc++; return dest; }");
        writeln!(f, "{}", "#endif");

        // Handle alloca
        writeln!(f, "{}", "#ifndef alloca");
        writeln!(f, "{}", "#define alloca(size) __builtin_alloca(size)");
        writeln!(f, "{}", "#endif");

        let global_map = ASTToIntermediate::allocate_globals(staticitems);
        let global_initializer = {
            let mut converter = ASTToIntermediate::new(&mut session,
                                                       &mut typemap,
                                                       &mangler.names,
                                                       &mut sourcemap);

            converter.convert_globals(&global_map)
        };
        result.push(global_initializer);

        // Print function prototypes.
        for insts in result.iter() {
            for inst in insts.iter() {
                match inst.val {
                    OpNode::Func(ref name, _, ref abi) => {
                        write!(f, "{}long {}();\n",
                               match *abi {
                                   Some(_) => format!("extern "),
                                   _ => "".to_string()
                               },
                               session.interner.name_to_str(name));
                    },
                    _ => {}
                }
            }
        }


        // Print global definitions.
        for (_, item) in global_map.iter() {
            if !item.is_func {
                if item.is_ref {
                    writeln!(f, "char {}[{}];", item.name, item.size);
                } else {
                    writeln!(f, "long {};", item.name);
                }
            }
        }

        let mut ssa_time: u64 = 0;
        let mut fold_time: u64 = 0;
        let mut convert_time: u64 = 0;

        for insts in result.iter_mut() {
            let start = precise_time_ns();
            ToSSA::to_ssa(insts, self.verbose);
            let end = precise_time_ns();
            ssa_time += end-start;
            if self.verbose {
                for inst in insts.iter() {
                    write!(f, "{}", inst);
                }
            }
            let start = precise_time_ns();
            ConstantFolder::fold(insts, &global_map, self.verbose);
            let end = precise_time_ns();
            fold_time += end-start;
            if self.verbose {
                for inst in insts.iter() {
                    write!(f, "{}", inst);
                }
                for a in LivenessAnalyzer::analyze(insts).iter() {
                    write!(f, "{:?}\n", a);
                }
            }
            let start = precise_time_ns();
            write!(f, "{}\n", self.convert_function(&*session.interner, insts, &global_map));
            let end = precise_time_ns();
            convert_time += end-start;
        }

        writeln!(f, "{}", "int main(int argc, char **argv) { _INIT_GLOBALS(); return (int)((long (*)())__main)((long)argc, (long)argv); }");

        writeln!(f, "// ssa:{} fold:{} convert:{}",
                 ssa_time as f32 / 1000000000f32,
                 fold_time as f32 / 1000000000f32,
                 convert_time as f32 / 1000000000f32);
        let (label_time, gen_time, min_time) = get_times();
        writeln!(f, "// label:{}, gen:{}, min:{}",
                 label_time as f32 / 1000000000f32,
                 gen_time as f32 / 1000000000f32,
                 min_time as f32 / 1000000000f32);
        let (opinfo_time, param_time_1, param_time_2) = get_param_times();
        writeln!(f, "// opinfo:{}, param1:{}, param2:{}",
                 opinfo_time as f32 / 1000000000f32,
                 param_time_1 as f32 / 1000000000f32,
                 param_time_2 as f32 / 1000000000f32);
        let (seed_time, propagate_time) = get_liveness_times();
        writeln!(f, "// seed:{}, propagate:{}",
                 seed_time as f32 / 1000000000f32,
                 propagate_time as f32 / 1000000000f32);
    }
}
