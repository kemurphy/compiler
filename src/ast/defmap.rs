use ast::visit::*;
use ast::*;
use collections::TreeMap;

// in case we ever want generic bounds
use ParamType = ast::UnitType;

#[deriving(Show)]
pub enum Def {
    ModDef(Vec<DefId>),
    TypeDef(TypeNode),
    FuncDef(Vec<DefId>, TypeNode, Vec<DefId>),
    FuncArgDef(TypeNode),
    StructDef(Vec<DefId>, Vec<DefId>),
    FieldDef(TypeNode),
    EnumDef(Vec<DefId>, Vec<DefId>),
    VariantDef(Vec<TypeNode>),
    LetDef(Option<TypeNode>),
}

pub struct DefMap {
    table: TreeMap<DefId, Def>,
}

impl DefMap {
    pub fn new() -> DefMap {
        DefMap {
            table: TreeMap::new(),
        }
    }

    pub fn find<'a>(&'a self, id: &DefId) -> Option<&'a Def> {
        self.table.find(id)
    }
}

/* TODO intern strings so clone is cheaper for NamedTypes */

impl Visitor for DefMap {
    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt.val {
            LetStmt(ref ident, ref t, ref e) => {
                self.table.insert(ident.id, LetDef(t.as_ref().map(|t| t.val.clone())));

                for e in e.iter() {
                    self.visit_expr(e);
                }
            }
            _ => { walk_stmt(self, stmt); }
        }

    }

    fn visit_item(&mut self, item: &Item) {
        match item.val {
            FuncItem(ref ident, ref args, ref t, ref def, ref tps) => {
                let arg_def_ids = args.iter().map(|arg| {
                    self.table.insert(arg.ident.id, FuncArgDef(arg.argtype.val.clone()));
                    arg.ident.id
                }).collect();

                let tp_def_ids = tps.iter().map(|tp| {
                    self.table.insert(tp.id, TypeDef(ParamType));
                    tp.id
                }).collect();

                self.table.insert(ident.id, FuncDef(arg_def_ids, t.val.clone(), tp_def_ids));

                self.visit_block(def);
            },
            StructItem(ref ident, ref fields, ref tps) => {
                let field_def_ids = fields.iter().map(|field| {
                    self.table.insert(field.ident.id, FieldDef(field.fldtype.val.clone()));
                    field.ident.id
                }).collect();

                let tp_def_ids = tps.iter().map(|tp| {
                    self.table.insert(tp.id, TypeDef(ParamType));
                    tp.id
                }).collect();

                self.table.insert(ident.id, StructDef(field_def_ids, tp_def_ids));
            },
            EnumItem(ref ident, ref variants, ref tps) => {
                let variant_def_ids = variants.iter().map(|variant| {
                    let args = variant.args.iter().map(|arg| arg.val.clone()).collect();
                    self.table.insert(variant.ident.id, VariantDef(args));
                    variant.ident.id
                }).collect();

                let tp_def_ids = tps.iter().map(|tp| {
                    self.table.insert(tp.id, TypeDef(ParamType));
                    tp.id
                }).collect();

                self.table.insert(ident.id, EnumDef(variant_def_ids, tp_def_ids));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DefMap;
    use ast::DefId;
    use ast::visit::Visitor;
    use parser::new_from_string;
    use collections::TreeMap;

    #[test]
    fn compare_canonicalized() {
        let mut parser = new_from_string("fn wot<T>(t: T) { let u = t; }"
                                         .to_owned());
        let tree = parser.parse_module();
        let mut defmap = DefMap::new();
        defmap.visit_module(&tree);

        assert_eq!(format!("{}", defmap.find(&DefId(0))),
                   "Some(FuncDef([DefId(2)], (), [DefId(1)]))".to_owned());
        assert_eq!(format!("{}", defmap.find(&DefId(4))),
                   "Some(LetDef(None))".to_owned());
    }
}
