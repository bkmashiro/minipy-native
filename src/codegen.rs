//! Source-dependent Cranelift lowering. The only raw access boundary is here.
use crate::{
    buffer::Quote,
    ir::{self, BinOp, CompileError, Expr, Stmt},
};
use cranelift_codegen::{
    ir::{AbiParam, InstBuilder, MemFlags, Value, condcodes::IntCC, types},
    settings::{self, Configurable},
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};

#[repr(C)]
pub(crate) struct Frame {
    pub locals: [i64; ir::MAX_LOCALS],
    pub index: u64,
    initialized: u64,
    pub total: i64,
}
impl Frame {
    pub(crate) fn new(batch: i64, limit: i64) -> Self {
        let mut locals = [0; ir::MAX_LOCALS];
        locals[0] = batch;
        locals[1] = limit;
        Self {
            locals,
            index: 0,
            initialized: 0,
            total: 0,
        }
    }
}
type Entry = unsafe extern "C" fn(*mut Frame, *const Quote, u64, u64) -> i64;
/// Not Send/Sync: this first dispatcher and code ownership are thread-local.
pub struct CodeUnit {
    module: Option<JITModule>,
    entry: Entry,
    pub code_bytes: usize,
    pub clif: String,
}
impl Drop for CodeUnit {
    fn drop(&mut self) {
        // No exported function pointer or surviving task can call after owner drop.
        if let Some(module) = self.module.take() {
            unsafe {
                module.free_memory();
            }
        }
    }
}
impl CodeUnit {
    pub fn compile(source: &str) -> Result<Self, CompileError> {
        Self::compile_with_opt(source, "speed")
    }
    pub fn compile_with_opt(source: &str, opt: &str) -> Result<Self, CompileError> {
        let program = crate::frontend::parse(source)?;
        let mut flags = settings::builder();
        flags.set("opt_level", opt).map_err(error)?;
        let isa = cranelift_native::builder()
            .map_err(error)?
            .finish(settings::Flags::new(flags))
            .map_err(error)?;
        if isa.pointer_type() != types::I64 {
            return Err(CompileError("only 64-bit hosts supported".into()));
        }
        let mut module = JITModule::new(JITBuilder::with_isa(
            isa,
            cranelift_module::default_libcall_names(),
        ));
        let mut ctx = module.make_context();
        ctx.func.signature.params = vec![AbiParam::new(types::I64); 4];
        ctx.func.signature.returns.push(AbiParam::new(types::I64));
        let id = module
            .declare_function("scan", Linkage::Local, &ctx.func.signature)
            .map_err(error)?;
        let mut fc = FunctionBuilderContext::new();
        {
            let mut b = FunctionBuilder::new(&mut ctx.func, &mut fc);
            let entry = b.create_block();
            b.append_block_params_for_function_params(entry);
            b.switch_to_block(entry);
            let params = b.block_params(entry).to_vec();
            let frame = params[0];
            let rows = params[1];
            let end = params[2];
            let len = params[3];
            let vars: Vec<_> = program
                .locals
                .iter()
                .map(|_| b.declare_var(types::I64))
                .collect();
            for (i, var) in vars.iter().enumerate() {
                let v = b
                    .ins()
                    .load(types::I64, MemFlags::trusted(), frame, (i * 8) as i32);
                b.def_var(*var, v);
            }
            let ix = b.declare_var(types::I64);
            let loaded = b.ins().load(types::I64, MemFlags::trusted(), frame, 256);
            b.def_var(ix, loaded);
            let overflow = b.create_block();
            let init = b.create_block();
            let header = b.create_block();
            let initialized = b.ins().load(types::I64, MemFlags::trusted(), frame, 264);
            b.ins().brif(initialized, header, &[], init, &[]);
            b.switch_to_block(init);
            let mut emit = Emit {
                b: &mut b,
                vars: &vars,
                row: None,
                overflow,
            };
            emit.stmts(&program.init);
            let one = b.ins().iconst(types::I64, 1);
            b.ins().store(MemFlags::trusted(), one, frame, 264);
            b.ins().jump(header, &[]);
            b.switch_to_block(header);
            let i = b.use_var(ix);
            let more = b.ins().icmp(IntCC::UnsignedLessThan, i, end);
            let body = b.create_block();
            let finish = b.create_block();
            b.ins().brif(more, body, &[], finish, &[]);
            b.switch_to_block(body);
            let offset = b.ins().ishl_imm(i, 4);
            let row = b.ins().iadd(rows, offset);
            let mut emit = Emit {
                b: &mut b,
                vars: &vars,
                row: Some(row),
                overflow,
            };
            emit.stmts(&program.body);
            let i = b.use_var(ix);
            let next = b.ins().iadd_imm(i, 1);
            b.def_var(ix, next);
            b.ins().jump(header, &[]);
            b.switch_to_block(finish);
            let i = b.use_var(ix);
            b.ins().store(MemFlags::trusted(), i, frame, 256);
            for (j, var) in vars.iter().enumerate() {
                let v = b.use_var(*var);
                b.ins().store(MemFlags::trusted(), v, frame, (j * 8) as i32);
            }
            let all = b.ins().icmp(IntCC::Equal, i, len);
            let final_total = b.create_block();
            let success = b.create_block();
            b.ins().brif(all, final_total, &[], success, &[]);
            b.switch_to_block(final_total);
            let total = Emit {
                b: &mut b,
                vars: &vars,
                row: None,
                overflow,
            }
            .expr(&program.total);
            b.ins().store(MemFlags::trusted(), total, frame, 272);
            b.ins().jump(success, &[]);
            b.switch_to_block(success);
            let zero = b.ins().iconst(types::I64, 0);
            b.ins().return_(&[zero]);
            b.switch_to_block(overflow);
            let one = b.ins().iconst(types::I64, 1);
            b.ins().return_(&[one]);
            b.seal_all_blocks();
            b.finalize();
        }
        let clif = ctx.func.display().to_string();
        module.define_function(id, &mut ctx).map_err(error)?;
        let code_bytes = ctx
            .compiled_code()
            .ok_or_else(|| CompileError("missing machine code".into()))?
            .code_info()
            .total_size as usize;
        module.finalize_definitions().map_err(error)?;
        let ptr = module.get_finalized_function(id);
        // Signature and frame offsets above are fixed, audited, and layout-tested.
        let entry: Entry = unsafe { std::mem::transmute(ptr) };
        Ok(Self {
            module: Some(module),
            entry,
            code_bytes,
            clif,
        })
    }
    pub(crate) fn scan(
        &self,
        frame: &mut Frame,
        rows: &[Quote],
        end: usize,
    ) -> Result<(), crate::error::TaskError> {
        // Owner borrow proves actual allocation, alignment, immutable contents and lifetime.
        // Runtime chooses monotonic bounded end. Generated body only loads index < end.
        assert!(frame.index as usize <= end && end <= rows.len());
        let status = unsafe { (self.entry)(frame, rows.as_ptr(), end as u64, rows.len() as u64) };
        if status == 0 {
            Ok(())
        } else {
            Err(crate::error::TaskError::Overflow)
        }
    }
}
fn error(e: impl std::fmt::Display) -> CompileError {
    CompileError(e.to_string())
}
struct Emit<'a, 'b> {
    b: &'a mut FunctionBuilder<'b>,
    vars: &'a [Variable],
    row: Option<Value>,
    overflow: cranelift_codegen::ir::Block,
}
impl Emit<'_, '_> {
    fn stmts(&mut self, ss: &[Stmt]) {
        for s in ss {
            match s {
                Stmt::Set(i, e) => {
                    let v = self.expr(e);
                    self.b.def_var(self.vars[*i], v);
                }
                Stmt::If(e, yes, no) => {
                    let c = self.expr(e);
                    let y = self.b.create_block();
                    let n = self.b.create_block();
                    let join = self.b.create_block();
                    self.b.ins().brif(c, y, &[], n, &[]);
                    self.b.switch_to_block(y);
                    self.stmts(yes);
                    self.b.ins().jump(join, &[]);
                    self.b.switch_to_block(n);
                    self.stmts(no);
                    self.b.ins().jump(join, &[]);
                    self.b.switch_to_block(join);
                }
            }
        }
    }
    fn expr(&mut self, e: &Expr) -> Value {
        match e {
            Expr::Int(v) => self.b.ins().iconst(types::I64, *v),
            Expr::Bool(v) => self.b.ins().iconst(types::I64, i64::from(*v)),
            Expr::Local(i) => self.b.use_var(self.vars[*i]),
            Expr::Field(f) => self.b.ins().load(
                types::I64,
                MemFlags::trusted(),
                self.row.expect("frontend prohibits fields outside scan"),
                match f {
                    ir::Field::Price => 0,
                    ir::Field::Quantity => 8,
                },
            ),
            Expr::Not(x) => {
                let v = self.expr(x);
                let x = self.b.ins().icmp_imm(IntCC::Equal, v, 0);
                self.b.ins().uextend(types::I64, x)
            }
            Expr::Binary(op, l, r) => {
                let a = self.expr(l);
                if matches!(op, BinOp::And | BinOp::Or) {
                    let rhs = self.b.create_block();
                    let skip = self.b.create_block();
                    let join = self.b.create_block();
                    self.b.append_block_param(join, types::I64);
                    if matches!(op, BinOp::And) {
                        self.b.ins().brif(a, rhs, &[], skip, &[]);
                    } else {
                        self.b.ins().brif(a, skip, &[], rhs, &[]);
                    }
                    self.b.switch_to_block(skip);
                    self.b.ins().jump(join, &[a.into()]);
                    self.b.switch_to_block(rhs);
                    let v = self.expr(r);
                    self.b.ins().jump(join, &[v.into()]);
                    self.b.switch_to_block(join);
                    return self.b.block_params(join)[0];
                }
                let c = self.expr(r);
                if matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul) {
                    let (v, o) = match op {
                        BinOp::Add => self.b.ins().sadd_overflow(a, c),
                        BinOp::Sub => self.b.ins().ssub_overflow(a, c),
                        _ => self.b.ins().smul_overflow(a, c),
                    };
                    let next = self.b.create_block();
                    self.b.ins().brif(o, self.overflow, &[], next, &[]);
                    self.b.switch_to_block(next);
                    v
                } else {
                    let cc = match op {
                        BinOp::Eq => IntCC::Equal,
                        BinOp::Ne => IntCC::NotEqual,
                        BinOp::Lt => IntCC::SignedLessThan,
                        BinOp::Le => IntCC::SignedLessThanOrEqual,
                        BinOp::Gt => IntCC::SignedGreaterThan,
                        BinOp::Ge => IntCC::SignedGreaterThanOrEqual,
                        _ => unreachable!(),
                    };
                    let v = self.b.ins().icmp(cc, a, c);
                    self.b.ins().uextend(types::I64, v)
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout() {
        assert_eq!(std::mem::offset_of!(Frame, index), 256);
        assert_eq!(std::mem::offset_of!(Frame, initialized), 264);
        assert_eq!(std::mem::offset_of!(Frame, total), 272);
        assert_eq!(std::mem::size_of::<Frame>(), 280);
    }
}
