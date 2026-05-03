use anyhow::Result;
use llvm_ir::Module;

pub fn emit(_module: &Module) -> Result<String> {
    Ok(String::from("# awkvm: codegen not implemented yet\n"))
}
