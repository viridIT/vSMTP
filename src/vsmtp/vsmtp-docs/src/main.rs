use rhai::packages::Package;
use vsmtp_rule_engine::{modules::StandardVSLPackage, rule_engine::RuleEngine, SharedObject};

const MODULE_SYNTAX: &str = "# Module:";

fn generate_variable_documentation_from_module(title: &str, module: &rhai::Module) -> String {
    let (var_count, _, _) = module.count();

    let mut variables_doc = Vec::with_capacity(var_count);

    for (name, value) in module.iter_var() {
        variables_doc.push(format!(
            "|`{}`|{}|",
            name,
            if value.is::<SharedObject>() {
                format!("{:?}", *value.clone_cast::<SharedObject>())
            } else {
                format!("{:?}", value)
            }
        ));
    }

    format!(
        "# {}\n|name|value|\n| - | - |\n{}\n",
        title,
        variables_doc.join("\n")
    )
}

fn generate_function_documentation_from_module(title: &str, module: &rhai::Module) -> String {
    let mut functions_doc: std::collections::HashMap<&str, Vec<_>> =
        std::collections::HashMap::new();

    for (_, _, _, _, metadata) in module.iter_script_fn_info() {
        let comments = &metadata
            .comments
            .iter()
            .map(|comment| format!("{}\n", &comment[3..]))
            .collect::<String>();

        let module = metadata
            .comments
            .iter()
            .find_map(|line| {
                line.find(MODULE_SYNTAX)
                    .map(|index| &line[index + MODULE_SYNTAX.len()..])
            })
            .unwrap_or("other");

        functions_doc.entry(module).or_default().push(format!(
            "<details><summary>{}({})</summary><br/>{}</details>",
            metadata.name,
            metadata.params.join(", "),
            &comments
        ));
    }

    format!("# {}\n{}\n", title, {
        let mut sorted = functions_doc.iter().collect::<Vec<_>>();

        sorted.sort_by_key(|a| a.0);

        sorted
            .into_iter()
            .fold(String::default(), |acc, functions| {
                let module = functions.0;
                let mut functions = functions.1.clone();
                format!("{}\n# {}\n{}\n\n", acc, module, {
                    functions.sort();
                    functions.join("\n")
                })
            })
    })
}

// TODO: find a way to incorporate native functions metadata and documentation.
//         - use docs.rs to get into native functions ? => not user friendly
//         - wrap 'sys' api into rhai functions ?       => might be cumbersome.

fn main() {
    let mut engine = RuleEngine::new_compiler();
    let vsl_native_module = StandardVSLPackage::new().as_shared_module();

    engine.register_static_module("sys", vsl_native_module);
    let vsl_rhai_module = RuleEngine::compile_api(&engine).expect("failed to compile vsl's api");

    let mut docs = generate_function_documentation_from_module(
        "Rhai Functions documentation",
        &vsl_rhai_module,
    );
    docs += "\n";
    docs += &generate_variable_documentation_from_module(
        "Rhai Variables documentation",
        &vsl_rhai_module,
    );

    let mut args = std::env::args();
    args.next().unwrap();

    let path = args
        .next()
        .expect("please specify a path to the generated Markdown documentation");

    // TODO: replace by path by args.
    std::fs::write(path, docs.as_bytes()).expect("failed to write docs");
}
