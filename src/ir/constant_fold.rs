// Constant folding.
use util::{Name, GenericInt};
use ir::util::subst;
use std::collections::{TreeSet, TreeMap};
use mc::ast::*;
use ir::*;
use values::{eval_binop, eval_unop};

pub struct ConstantFolder;

fn assert_signedness(l: &LitNode, signed: bool) {
    match *l {
        NumLit(_, ref kind) =>
            if !kind.is_generic() {
                assert_eq!(signed, kind.is_signed())
            },
        _ => {}
    }
}

fn fold(op: &BinOpNode, e1: &RValueElem, e2: &RValueElem, signed: bool) ->
    Option<LitNode> {
    let lit1 = match *e1 {
        Constant(ref l) => {
            assert_signedness(l, signed);
            l.clone()
        },
        _ => return None,
    };
    let lit2 = match *e2 {
        Constant(ref l) => {
            assert_signedness(l, signed);
            l.clone()
        }
        _ => return None,
    };

    Some(eval_binop(*op, lit1, lit2))
}

fn fold_unary(op: &UnOpNode, e: &RValueElem) -> Option<LitNode> {
    let lit = match *e {
        Constant(ref l) => {
            match *l {
                NumLit(..) |
                BoolLit(..) => l.clone(),
                StringLit(..) => return None,
                _ => fail!(),
            }
        }
        _ => return None,
    };

    eval_unop(*op, lit)
}

fn constant_fold_once<T>(ops: &mut Vec<Op>, vars_to_avoid: &TreeSet<Var>,
                         globals: &TreeMap<Name, T>,
                         verbose: bool) -> bool {
    // Variables to replace with constants.
    let mut changes = vec!();
    let mut immediate_changes = vec!();

    for (pos, op) in ops.iter().enumerate() {
        match *op {
            BinOp(ref v, ref op, ref v1, ref v2, signed) => {
                match fold(op, v1, v2, signed) {
                    Some(c) => {
                        changes.push((v.clone(),
                                      Constant(c.clone())));
                        immediate_changes.push((pos,
                                                UnOp(v.clone(),
                                                     Identity,
                                                     Constant(c))));
                    },
                    None => {
                        // TODO: convert multiplications and divisions by
                        // powers of two into shifts.

                        // We can't directly fold, but if we're
                        // applying the operator to an identity
                        // element, we can optimize out the operation.
                        let ident = match *op {
                            TimesOp => Some(1),
                            PlusOp => Some(0),
                            _ => None,
                        };
                        // Some operators have a right identity that is not
                        // a left identity.
                        let rhs_ident = match *op {
                            DivideOp => Some(1),
                            MinusOp => Some(0),
                            _ => None,
                        };
                        // Note: we won't match in *both* of these, because
                        // if that could have happened we instead would have
                        // folded above and never landed here at all.
                        match *v1 {
                            Constant(NumLit(x, _)) if Some(x) == ident => {
                                immediate_changes.push(
                                    (pos,
                                     UnOp(v.clone(),
                                          Identity,
                                          v2.clone())));
                            },
                            _ => {}
                        }
                        match *v2 {
                            Constant(NumLit(x, _)) if (Some(x) == ident ||
                                                       Some(x) == rhs_ident
                                                       ) => {
                                immediate_changes.push(
                                    (pos,
                                     UnOp(v.clone(),
                                          Identity,
                                          v1.clone())));
                            }
                            _ => {}
                        }
                    }
                }
            },
            UnOp(ref v, ref op, ref rv) => {
                match fold_unary(op, rv) {
                    Some(c) => {
                        changes.push((v.clone(),
                                      Constant(c.clone())));
                        if *op != Identity {
                            immediate_changes.push((pos,
                                                    UnOp(v.clone(),
                                                         Identity,
                                                         Constant(c))));
                        }
                    },
                    _ => {}
                }
            }
            // TODO: unary ops, and anything else.
            _ => {},
        }
    }

    let mut changed = false;

    // These are changes we can do unconditionally.
    for (a, b) in immediate_changes.move_iter() {
        *ops.get_mut(a) = b;
        changed = true;
    }

    for (a, b) in changes.move_iter() {
        if !vars_to_avoid.contains(&a) && !globals.find(&a.name).is_some() {
            if verbose {
                print!("Applying {}->{}\n", a, b);
            }
            subst(ops, &a, &b);
            if verbose {
                print!("{}\n", ops);
            }
            changed = true;
        }
    }

    changed
}


impl ConstantFolder {

    pub fn fold<T>(ops: &mut Vec<Op>, globals: &TreeMap<Name, T>,
                   verbose: bool) {
        // There are certain variables we are prohibited from substituting.
        // Those include any that appear in labels/gotos, as well as any
        // that is dereferenced as part of the left hand side of an assignment.
        let mut vars_to_avoid = TreeSet::<Var>::new();
        for op in ops.iter() {
            match *op {
                Label(_, ref vars) |
                Goto(_, ref vars) |
                CondGoto(_, _, _, ref vars) => {
                    for var in vars.iter() {
                        vars_to_avoid.insert(var.clone());
                    }
                },
                // We can't fold anything we take the address of.
                UnOp(_, AddrOf, ref rv) => {
                    match *rv {
                        Variable(ref v) => { vars_to_avoid.insert(v.clone()); },
                        _ => {},
                    }
                },
                Store(ref v1, ref v2, _) |
                Load(ref v1, ref v2, _) => {
                    vars_to_avoid.insert(v1.clone());
                    vars_to_avoid.insert(v2.clone());
                },
                Call(_, _, ref vars) => {
                    for var in vars.iter() {
                        vars_to_avoid.insert(var.clone());
                    }
                },
                Func(_, ref vars, is_extern) => {
                    if is_extern { return; }
                    for var in vars.iter() {
                        vars_to_avoid.insert(var.clone());
                    }
                },
                _ => {},
            }
        }
        if verbose {
            print!("avoid: {}\n", vars_to_avoid);
        }
        while constant_fold_once(ops, &vars_to_avoid, globals, verbose) {}
    }
}
