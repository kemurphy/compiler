use package::Package;

use mc::lexer::Lexer;
use mc::parser::Parser;
use mc::session::Interner;

use super::Target;

use ir::liveness::LivenessAnalyzer;
use ir::ast_to_intermediate::ASTToIntermediate;
use ir::constant_fold::ConstantFolder;
use ir::ssa::ToSSA;
use ir::conflicts::ConflictAnalyzer;

use codegen::register_color::RegisterColorer;
use codegen::num_usable_vars;
use codegen::IrToAsm;

pub struct AsmTarget;

impl Target for AsmTarget {
    fn new(_args: Vec<String>) -> AsmTarget {
        AsmTarget
    }

    fn compile(&self, p: Package) {
        let Package {
            module:  module,
            session: mut session,
            typemap: mut typemap,
        } = p;

        let mut result =
        {
            let mut converter = ASTToIntermediate::new(&mut session,
                                                       &mut typemap);

            let mut result = vec!();

            for item in module.val.items.iter() {
                print!("{}\n", item);
                let (insts, _) = converter.convert_item(item);
                print!("{}\n\n", insts);
                result.push(insts);
            }
            result
        };

        for insts in result.mut_iter() {
            ToSSA::to_ssa(insts);
            ConstantFolder::fold(insts);
            for a in LivenessAnalyzer::analyze(insts).iter() {
                print!("{}\n", a);
            }
            print!("{}\n", insts);
            let (conflict_map, counts) = ConflictAnalyzer::conflicts(insts);
            print!("conflicts: {}\ncounts: {}\n", conflict_map, counts);
            print!("{}\n",
                   RegisterColorer::color(conflict_map, counts,
                                          num_usable_vars as uint));

            for inst in IrToAsm::ir_to_asm(insts).iter() {
                print!("   {}\n", inst);
            }
        }
    }

}