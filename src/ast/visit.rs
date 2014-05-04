use ast::*;

pub trait Visitor {
    fn visit_item(&mut self, item: &Item) { walk_item(self, item) }
    fn visit_type(&mut self, t: &Type) { walk_type(self, t) }
    fn visit_func_arg(&mut self, arg: &FuncArg) { walk_func_arg(self, arg) }
    fn visit_block(&mut self, block: &Block) { walk_block(self, block) }
    fn visit_stmt(&mut self, stmt: &Stmt) { walk_stmt(self, stmt) }
    fn visit_expr(&mut self, expr: &Expr) { walk_expr(self, expr) }
    fn visit_lit(&mut self, lit: &Lit) { walk_lit(self, lit) }
    fn visit_ident(&mut self, ident: &Ident) { walk_ident(self, ident) }
    fn visit_module(&mut self, module: &Module) { walk_module(self, module) }
}

pub fn walk_item<T: Visitor>(visitor: &mut T, item: &Item) {
    match item.val {
        FuncItem(ref id, ref args, ref t, ref def, ref tps) => {
            visitor.visit_ident(id);
            for arg in args.iter() { visitor.visit_func_arg(arg); }
            visitor.visit_type(t);
            visitor.visit_block(def);
            for id in tps.iter() { visitor.visit_ident(id); }
        },
        StructItem(ref id, ref fields, ref tps) => {
            visitor.visit_ident(id);
            for &(_, ref fieldtype) in fields.iter() {
                visitor.visit_type(fieldtype);
            }
            for id in tps.iter() { visitor.visit_ident(id); }
        },
        EnumItem(ref id, ref fields, ref tps) => {
            visitor.visit_ident(id);
            for ref field in fields.iter() {
                for fieldarg in field.val.args.iter() {
                    visitor.visit_type(fieldarg);
                }
            }
            for id in tps.iter() { visitor.visit_ident(id); }
        }
    }
}

pub fn walk_type<T: Visitor>(visitor: &mut T, t: &Type) {
    match t.val {
        PtrType(ref p) => {
            visitor.visit_type(*p);
        }
        NamedType(ref id) => {
            visitor.visit_ident(id);
        }
        FuncType(ref d, ref r) => {
            for a in d.iter() { visitor.visit_type(a); }
            visitor.visit_type(*r);
        }
        ArrayType(ref a, _) => {
            visitor.visit_type(*a);
        }
        TupleType(ref ts) => {
            for t in ts.iter() { visitor.visit_type(t); }
        }
        BoolType | UnitType | IntType(..) => {}
    }
}

pub fn walk_func_arg<T: Visitor>(visitor: &mut T, arg: &FuncArg) {
    visitor.visit_ident(&arg.ident);
    visitor.visit_type(&arg.argtype);
}

pub fn walk_block<T: Visitor>(visitor: &mut T, block: &Block) {
    for item in block.items.iter() { visitor.visit_item(item); }
    for stmt in block.stmts.iter() { visitor.visit_stmt(stmt); }
    for expr in block.expr.iter()  { visitor.visit_expr(expr); }
}

pub fn walk_stmt<T: Visitor>(visitor: &mut T, stmt: &Stmt) {
    match stmt.val {
        LetStmt(ref id, ref t, ref e) => {
            visitor.visit_ident(id);
            for t in t.iter() { visitor.visit_type(t); }
            for e in e.iter() { visitor.visit_expr(e); }
        }
        ExprStmt(ref e) => {
            visitor.visit_expr(e);
        }
        SemiStmt(ref e) => {
            visitor.visit_expr(e);
        }
        DeconstructTupleStmt(ref ids, ref e) => {
            for ident in ids.iter() { visitor.visit_ident(ident); }
            visitor.visit_expr(e);
        }
    }
}

pub fn walk_expr<T: Visitor>(visitor: &mut T, expr: &Expr) {
    match expr.val {
        LitExpr(ref l) => {
            visitor.visit_lit(l);
        }
        TupleExpr(ref es) => {
            for e in es.iter() { visitor.visit_expr(e); }
        }
        IdentExpr(ref id) => {
            visitor.visit_ident(id);
        }
        BinOpExpr(_, ref l, ref r) => {
            visitor.visit_expr(*l);
            visitor.visit_expr(*r);
        }
        UnOpExpr(_, ref e) => {
            visitor.visit_expr(*e);
        }
        IndexExpr(ref a, ref i) => {
            visitor.visit_expr(*a);
            visitor.visit_expr(*i);
        }
        DotExpr(ref e, _) => {
            visitor.visit_expr(*e);
        }
        ArrowExpr(ref e, _) => {
            visitor.visit_expr(*e);
        }
        AssignExpr(ref lv, ref rv) => {
            visitor.visit_expr(*lv);
            visitor.visit_expr(*rv);
        }
        CallExpr(ref f, ref args) => {
            visitor.visit_expr(*f);
            for arg in args.iter() { visitor.visit_expr(arg); }
        }
        CastExpr(ref e, ref t) => {
            visitor.visit_expr(*e);
            visitor.visit_type(t);
        }
        IfExpr(ref c, ref tb, ref fb) => {
            visitor.visit_expr(*c);
            visitor.visit_block(*tb);
            visitor.visit_block(*fb);
        }
        BlockExpr(ref b) => {
            visitor.visit_block(*b);
        }
        ReturnExpr(ref e) => {
            visitor.visit_expr(*e);
        }
        WhileExpr(ref e, ref b) => {
            visitor.visit_expr(*e);
            visitor.visit_block(*b);
        }
        ForExpr(ref e1, ref e2, ref e3, ref b) => {
            visitor.visit_expr(*e1);
            visitor.visit_expr(*e2);
            visitor.visit_expr(*e3);
            visitor.visit_block(*b);
        }
        UnitExpr => {}
        MatchExpr(ref e, ref items) => {
            visitor.visit_expr(*e);
            for item in items.iter() {
                for var in item.val.vars.iter() {
                    visitor.visit_ident(var);
                }
                visitor.visit_expr(&item.val.body);
            }
        }
    }
}

pub fn walk_lit<T: Visitor>(_: &T, lit: &Lit) {
    match lit.val {
        NumLit(..) | StringLit(..) | BoolLit(..) => {}
    }
}

pub fn walk_ident<T: Visitor>(visitor: &mut T, ident: &Ident) {
    for tps in ident.tps.iter() {
        for tp in tps.iter() { visitor.visit_type(tp); }
    }
}

pub fn walk_module<T: Visitor>(visitor: &mut T, module: &Module) {
    for item in module.items.iter() { visitor.visit_item(item); }
}

