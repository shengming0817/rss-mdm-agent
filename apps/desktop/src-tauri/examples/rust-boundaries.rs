// ref: syn src/lib.rs (parse_file), src/gen/visit.rs@2.0.119
// Medium source guard: parsed paths/imports, never a sandbox or macro expansion proof.
use proc_macro2::{TokenStream, TokenTree};
use std::{collections::BTreeMap, io};
use syn::visit::{self, Visit};

struct Guard {
    fixture: bool,
    ipc: bool,
    composition: bool,
    main: bool,
    forbidden: bool,
}
impl Guard {
    fn path(&mut self, parts: &[String]) {
        if parts.iter().any(|p| p == "prmonitor_lib") {
            self.forbidden = true;
        }
        self.forbidden |= !self.composition && parts.windows(2).any(|p| p == ["Command", "new"]);
        self.forbidden |= self.fixture
            && parts.first().is_some_and(|p| {
                [
                    "tokio",
                    "execution_app",
                    "execution_sqlite",
                    "execution_mcp",
                ]
                .contains(&p.as_str())
            });
        if self.fixture && parts.len() >= 2 {
            self.forbidden |= match parts[0].as_str() {
                "std" | "core" => {
                    ["fs", "net", "process", "thread", "ffi", "os"].contains(&parts[1].as_str())
                }
                "tauri" => ["async_runtime", "path", "process"].contains(&parts[1].as_str()),
                _ => false,
            };
        }
        self.forbidden |= !self.ipc && parts == ["tauri", "command"];
    }
    fn imports(&mut self, prefix: &mut Vec<String>, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(p) => {
                prefix.push(p.ident.to_string());
                self.path(prefix);
                self.imports(prefix, &p.tree);
                prefix.pop();
            }
            syn::UseTree::Group(g) => {
                for tree in &g.items {
                    self.imports(prefix, tree);
                }
            }
            syn::UseTree::Name(n) => {
                prefix.push(n.ident.to_string());
                self.path(prefix);
                if self.fixture
                    && prefix.len() == 2
                    && prefix[1] == "self"
                    && ["std", "core", "tauri"].contains(&prefix[0].as_str())
                {
                    self.forbidden = true;
                }
                prefix.pop();
            }
            syn::UseTree::Rename(n) => {
                prefix.push(n.ident.to_string());
                self.path(prefix);
                if self.fixture
                    && ["std", "core", "tauri"].contains(&prefix[0].as_str())
                    && (prefix.len() == 1 || prefix.last().is_some_and(|p| p == "self"))
                {
                    self.forbidden = true;
                }
                prefix.pop();
            }
            syn::UseTree::Glob(_) => {
                if self.fixture
                    && prefix
                        .first()
                        .is_some_and(|p| ["std", "core", "tauri"].contains(&p.as_str()))
                {
                    self.forbidden = true;
                }
            }
        }
    }
    fn tokens(&mut self, tokens: TokenStream) {
        // Macro bodies are token trees; comments and literals cannot masquerade as paths.
        let block = TokenTree::Group(proc_macro2::Group::new(
            proc_macro2::Delimiter::Brace,
            tokens.clone(),
        ));
        if let Ok(block) = syn::parse2::<syn::Block>(std::iter::once(block).collect()) {
            self.visit_block(&block);
            return;
        }
        let tokens: Vec<_> = tokens.into_iter().collect();
        for (i, token) in tokens.iter().enumerate() {
            if let TokenTree::Group(group) = token {
                self.tokens(group.stream());
            }
            if let TokenTree::Ident(first) = token {
                let mut parts = vec![first.to_string()];
                let mut next = i + 1;
                while next + 2 < tokens.len()
                    && matches!(&tokens[next], TokenTree::Punct(p) if p.as_char() == ':')
                    && matches!(&tokens[next + 1], TokenTree::Punct(p) if p.as_char() == ':')
                {
                    match &tokens[next + 2] {
                        TokenTree::Ident(name) => parts.push(name.to_string()),
                        TokenTree::Group(group)
                            if group.delimiter() == proc_macro2::Delimiter::Brace =>
                        {
                            if let Ok(tree) = syn::parse2::<syn::UseTree>(
                                std::iter::once(TokenTree::Group(group.clone()))
                                    .collect::<TokenStream>(),
                            ) {
                                self.imports(&mut parts, &tree);
                            } else if self.fixture
                                && ["std", "core", "tauri"].contains(&parts[0].as_str())
                            {
                                // A root grouped import inside a macro is outside this guard's resolution.
                                self.forbidden = true;
                            }
                            break;
                        }
                        _ => break,
                    }
                    next += 3;
                }
                self.path(&parts);
            }
        }
    }
}
impl<'ast> Visit<'ast> for Guard {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.path(
            &path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>(),
        );
        visit::visit_path(self, path);
    }
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.imports(&mut Vec::new(), &item.tree);
    }
    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        if self.fixture && ["std", "core", "tauri"].contains(&item.ident.to_string().as_str()) {
            self.forbidden = true;
        }
    }
    fn visit_item_foreign_mod(&mut self, item: &'ast syn::ItemForeignMod) {
        self.forbidden |= self.fixture;
        visit::visit_item_foreign_mod(self, item);
    }
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.to_string();
        self.forbidden |= name == "plugin"
            || (!self.ipc && name == "invoke_handler")
            || (name == "manage"
                && !(self.main
                    && matches!(&*call.receiver, syn::Expr::Path(path) if path.path.is_ident("app"))))
            || (self.fixture
                && ["path", "shell", "spawn", "spawn_blocking"].contains(&name.as_str()));
        visit::visit_expr_method_call(self, call);
    }
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.tokens(mac.tokens.clone());
        visit::visit_macro(self, mac);
    }
}
fn main() {
    let sources: BTreeMap<String, String> =
        serde_json::from_reader(io::stdin()).expect("source map");
    let mut errors = Vec::new();
    for (file, source) in sources {
        let mut guard = Guard {
            fixture: file.contains("/self_service/"),
            ipc: file.ends_with("/composition/ipc.rs"),
            composition: file.contains("/composition/"),
            main: file.ends_with("/src/main.rs"),
            forbidden: false,
        };
        match syn::parse_file(&source) {
            Ok(ast) => guard.visit_file(&ast),
            Err(_) => {
                errors.push(format!("{file}: invalid Rust syntax"));
                continue;
            }
        }
        if guard.forbidden {
            errors.push(format!("{file}: unexpected host capability; fixture service cannot access host I/O or background execution"));
        }
    }
    println!("{}", serde_json::to_string(&errors).unwrap());
}
