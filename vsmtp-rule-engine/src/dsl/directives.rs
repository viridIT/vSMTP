use crate::modules::EngineResult;
use vsmtp_common::status::Status;

/// a set of directives, filtered by smtp stage.
pub type Directives = std::collections::BTreeMap<String, Vec<Box<dyn Directive + Send + Sync>>>;

/// a directive rhai code and that can be executed, return a status.
pub trait Directive {
    fn directive_type(&self) -> &'static str;
    fn execute(&self, engine: &rhai::Engine, ast: &rhai::AST) -> EngineResult<Status>;
    fn name(&self) -> &str;
}

/// a rule, that returns an evaluated Status.
pub struct Rule {
    pub name: String,
    pub pointer: rhai::FnPtr,
}

impl Directive for Rule {
    fn directive_type(&self) -> &'static str {
        "rule"
    }

    fn execute(&self, engine: &rhai::Engine, ast: &rhai::AST) -> EngineResult<Status> {
        engine.call_fn(&mut rhai::Scope::new(), ast, self.pointer.fn_name(), ())
        // self.pointer.call(engine, ast, ())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// an action, that alway return the status Next.
pub struct Action {
    pub name: String,
    pub pointer: rhai::FnPtr,
}

impl Directive for Action {
    fn directive_type(&self) -> &'static str {
        "action"
    }

    fn execute(&self, engine: &rhai::Engine, ast: &rhai::AST) -> EngineResult<Status> {
        println!("{ast:#?}");
        engine.call_fn(&mut rhai::Scope::new(), ast, self.pointer.fn_name(), ())?;
        // let _: () = self.pointer.call(engine, ast, ())?;

        Ok(Status::Next)
    }

    fn name(&self) -> &str {
        &self.name
    }
}
