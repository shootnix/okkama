use super::*;
use super::lexer::Token;

impl Reader<'_> {
    pub(super) fn profile(&self) -> Profile {
        self.profile
    }

    pub(super) fn type_depth_limit(&self) -> usize {
        self.limits.max_type_depth.min(128)
    }

    pub(super) fn arena_error(&self, error: ArenaError) -> Diagnostic {
        self.error(DiagnosticKind::Limit, error.to_string())
    }

    pub(super) fn word(&mut self) -> Result<String, Diagnostic> {
        let token = self.next()?;
        if token.quoted || token.text.is_empty()
            || token.text != "$pending" && !token.text.chars().all(|c| c.is_alphanumeric() || "_./:#-".contains(c))
        {
            return Err(self.at(&token, DiagnosticKind::Syntax, "expected identifier or path"));
        }
        Ok(token.text)
    }

    pub(super) fn location(&mut self) -> Result<Option<DebugLoc>, Diagnostic> {
        if self.is("!") {
            Ok(Some(DebugLoc::read(self)?))
        } else {
            Ok(None)
        }
    }

    pub(super) fn end_of_line(&self) -> bool {
        self.tokens.get(self.pos).is_some_and(|t| !t.quoted && matches!(t.text.as_str(), "\n" | "<eof>"))
    }

    pub(super) fn operand_keyword(&self, name: &str) -> bool {
        let mut pos = self.pos;
        if self.tokens.get(pos).is_some_and(|t| t.text == "," && !t.quoted) {
            pos += 1;
        }
        self.tokens.get(pos).is_some_and(|t| t.text == name && !t.quoted)
    }

    pub(super) fn named_reference(&self, ns: &'static str, token: &Token, local: bool)
    -> Result<u32, Diagnostic>
    {
        if self.first {
            return Ok(0);
        }
        self.names.get(&(if local {
            self.scope
        } else {
            0
        }, ns, token.text.clone())).copied()
            .ok_or_else(|| self.at(token, DiagnosticKind::UnknownName, format!("unknown {ns} {}", token.text)))
    }

    pub(super) fn anonymous(&mut self, ns: &'static str) -> Result<u32, Diagnostic> {
        self.entity()?;
        let count = self.counts.entry((self.scope, ns)).or_default();
        let id = *count;
        *count = count.checked_add(1).ok_or_else(|| Diagnostic::new(
            DiagnosticKind::Limit, Span { start: 0, end: 0 }, "entity index overflow"
        ))?;
        Ok(id)
    }

    pub(super) fn declaration_kind(&self, name: &str) -> u32 {
        self.names.get(&(0, "decl-kind", name.into())).copied().unwrap_or(0)
    }

    pub(super) fn generic_parameter(&self, name: &str) -> Option<(GenericOwner, u32)> {
        let owner = self.generic_owner?;
        let key = generic_scope(owner);
        self.names.get(&(key, "parameter", name.into())).map(|index| (owner, *index))
    }

    pub(super) fn type_arguments(&mut self) -> Result<Vec<TypeId>, Diagnostic> {
        let mut args = vec![];
        if self.eat("<") {
            while !self.is(">") {
                self.entity()?;
                args.push(TypeId::read(self)?);
                if !self.eat(",") {
                    break;
                }
            }
            self.expect(">")?;
        }
        Ok(args)
    }

    pub(super) fn type_parameters(&mut self) -> Result<Vec<TypeParameter>, Diagnostic> {
        let mut params = vec![];
        if self.eat("<") {
            while !self.is(">") {
                self.entity()?;
                let name = self.word()?;
                if self.first {
                    let owner = self.generic_owner.ok_or_else(|| self.error(DiagnosticKind::Structure,
                    "generic owner"))?;
                    if self.names.insert((generic_scope(owner), "parameter", name.clone()),
                        params.len() as u32).is_some() {
                        return Err(self.error(DiagnosticKind::DuplicateName, "duplicate type parameter"));
                    }
                }
                let mut bounds = vec![];
                if self.eat(":") {
                    loop {
                        bounds.push(TypeId::read(self)?);
                        if !self.eat("+") {
                            break;
                        }
                    }
                }
                params.push(TypeParameter { name, bounds });
                if !self.eat(",") {
                    break;
                }
            }
            self.expect(">")?;
        }
        Ok(params)
    }

    pub(super) fn results(&mut self) -> Result<Vec<TypeId>, Diagnostic> {
        if self.eat("void") {
            return Ok(vec![]);
        }
        if !self.eat("(") {
            return Ok(vec![TypeId::read(self)?]);
        }
        let mut results = vec![];
        while !self.is(")") {
            self.entity()?;
            // Result names in specification examples are descriptive, not SSA definitions.
            if self.peek().text.starts_with('%') {
                self.next()?;
                self.expect(":")?;
            }
            results.push(TypeId::read(self)?);
            self.comma()?;
        }
        self.expect(")")?;
        Ok(results)
    }

    pub(super) fn attributes(&mut self) -> Result<Vec<FunctionAttr>, Diagnostic> {
        self.expect("[")?;
        let mut attrs = vec![];
        while !self.is("]") {
            let tag = self.next()?;
            let attr = match tag.text.as_str() {
                "entry" => FunctionAttr::Entry,
                "async" => FunctionAttr::Async,
                "private" => FunctionAttr::Hidden,
                "method" => FunctionAttr::Method(DeclId::read(self)?),
                "ctor" => FunctionAttr::Ctor(DeclId::read(self)?),
                "successor" => FunctionAttr::Successor(DeclId::read(self)?),
                "destroy" => FunctionAttr::Destroy(DeclId::read(self)?),
                "default" => {
                    let token = self.next()?;
                    let (owner, member) = token.text.rsplit_once('.')
                        .ok_or_else(|| self.at(&token, DiagnosticKind::Syntax, "expected Type.field"))?;
                    let mut owner_token = token.clone();
                    owner_token.text = owner.into();
                    let object = DeclId::from_u32(self.named_reference("decl", &owner_token, false)?);
                    let field = if self.first {
                        0
                    } else {
                        member.parse().ok().or_else(|| {
                            self.names.get(&(object.as_u32() as usize, "field", member.into())).copied()
                        }).ok_or_else(|| self.at(&token, DiagnosticKind::UnknownName, "unknown default field"))?
                    };
                    FunctionAttr::Default { object, field }
                }
                "extern" => FunctionAttr::Extern(String::read(self)?),
                "intrinsic" => {
                    let name = String::read(self)?;
                    let objects = if self.eat("objects") {
                        Vec::read(self)?
                    } else {
                        vec![]
                    };
                    let helpers = if self.eat("helpers") {
                        Vec::read(self)?
                    } else {
                        vec![]
                    };
                    FunctionAttr::Intrinsic { name, objects, helpers }
                }
                "lambda" => {
                    let captures = if self.eat("captures") {
                        u32::read(self)?
                    } else {
                        0
                    };
                    let owner = if self.eat("owner") {
                        Some(DeclId::read(self)?)
                    } else {
                        None
                    };
                    FunctionAttr::Lambda { captures, owner }
                }
                _ => return Err(self.at(&tag, DiagnosticKind::Syntax, "unknown function attribute")),
            };
            attrs.push(attr);
            self.comma()?;
        }
        self.expect("]")?;
        Ok(attrs)
    }

    pub(super) fn summary_reference(&mut self, name: &str) -> Result<Option<SummaryId>, Diagnostic> {
        if !self.eat(name) {
            return Ok(None);
        }
        self.expect("(")?;
        let id = SummaryId::read(self)?;
        self.expect(")")?;
        Ok(Some(id))
    }

    pub(super) fn import(&mut self) -> Result<Import, Diagnostic> {
        let token = self.next()?;
        let path = token.text;
        let major = if self.eat("@") {
            Some(u32::read(self)?)
        }
        else if self.peek().text.starts_with('@') && !self.end_of_line() {
            let token = self.next()?;
            Some(token.text[1..].parse().map_err(|_| self.at(&token, DiagnosticKind::Literal, "import major"))?)
        } else {
            None
        };
        let alias = if self.eat("as") {
            Some(self.word()?)
        } else {
            None
        };
        let mut take = vec![];
        if self.eat("take") {
            loop {
                take.push(self.word()?);
                if !self.eat(",") {
                    break;
                }
            }
        }
        Ok(Import { path, major, alias, take })
    }

    pub(super) fn summary(&mut self) -> Result<Summary, Diagnostic> {
        let entry_kind = EntryKind::read(self)?;
        let parameters = if self.eat("parameters") {
            Vec::read(self)?
        } else {
            vec![]
        };
        let results = if self.eat("results") {
            Vec::read(self)?
        } else {
            vec![]
        };
        self.expect("contract")?;
        let contract = CallContract::read(self)?;
        Ok(Summary { entry_kind, parameters, results, contract })
    }

    pub(super) fn protocol(&mut self) -> Result<ProtocolDecl, Diagnostic> {
        let holders = if self.eat("holders") {
            Vec::read(self)?
        } else {
            vec![]
        };
        let rules = if self.eat("rules") {
            Vec::read(self)?
        } else {
            vec![]
        };
        let roots = if self.eat("roots") {
            Vec::read(self)?
        } else {
            vec![]
        };
        Ok(ProtocolDecl { holders, rules, roots })
    }

    pub(super) fn binding(&mut self) -> Result<BindingDecl, Diagnostic> {
        self.expect("binding")?;
        self.definition("binding", "b", true)?;
        let name = String::read(self)?;
        self.expect(":")?;
        let ty = TypeId::read(self)?;
        self.expect("in")?;
        let scope = ScopeId::read(self)?;
        let loc = self.location()?;
        self.line()?;
        Ok(BindingDecl { name, scope, ty, loc })
    }

    pub(super) fn boundary(&mut self) -> Result<Boundary, Diagnostic> {
        self.expect("boundary")?;
        self.definition("boundary", "B", true)?;
        let kind = BoundaryKind::read(self)?;
        self.expect("in")?;
        let scope = ScopeId::read(self)?;
        self.line()?;
        Ok(Boundary { scope, kind })
    }

    pub(super) fn declaration(&mut self, tag: &str) -> Result<Declaration, Diagnostic> {
        if tag == "native" {
            self.expect("object")?;
        }
        let owner = if tag == "cap" {
            let owner = TypeId::read(self)?;
            self.expect("::")?;
            Some(owner)
        } else {
            None
        };
        let id = self.definition("decl", "", false)?;
        let name = self.tokens[self.pos - 1].text.clone();
        let kind_code = match tag {
            "role" => 1, "enum" => 2, "variant" => 3, "native" => 4, "cap" => 5, _ => 0
        };
        if self.first {
            self.names.insert((0, "decl-kind", name.clone()), kind_code);
        }
        self.generic_owner = Some(GenericOwner::Declaration(DeclId::from_u32(id)));
        let type_params = self.type_parameters()?;
        let builtin = if self.eat("[") {
            self.expect("builtin")?;
            self.expect("]")?;
            true
        } else {
            false
        };
        let is_error = if self.eat("is") {
            self.expect("Error")?;
            true
        } else {
            false
        };
        let kind = match tag {
            "enum" => {
                self.expect("=")?;
                self.expect("(")?;
                let mut cases = vec![];
                while !self.is(")") {
                    let name = self.word()?;
                    if self.first {
                        self.names.insert((id as usize, "case", name.clone()), cases.len() as u32);
                    }
                    cases.push(name);
                    self.comma()?;
                }
                self.expect(")")?;
                DeclarationKind::Enum { cases, is_error }
            }
            "variant" => {
                self.expect("=")?;
                self.expect("(")?;
                let mut cases = vec![];
                while !self.is(")") {
                    let name = self.word()?;
                    self.expect(":")?;
                    let layout = DeclId::read(self)?;
                    cases.push(VariantCase { name, layout });
                    self.comma()?;
                }
                self.expect(")")?;
                DeclarationKind::Variant { cases, is_error }
            }
            "native" => DeclarationKind::Handle,
            "global" => {
                self.expect(":")?;
                let ty = TypeId::read(self)?;
                self.expect("=")?;
                let init = FuncId::read(self)?;
                DeclarationKind::Global { ty, init }
            }
            "role" => {
                self.expect("{")?;
                self.line()?;
                let mut methods = vec![];
                while !self.eat("}") {
                    let name = self.word()?;
                    let signature = Signature::read(self)?;
                    methods.push(RoleMethod { name, signature });
                    self.line()?;
                }
                DeclarationKind::Role { methods, builtin }
            }
            "cap" => {
                self.expect("{")?;
                self.line()?;
                let mut fields = vec![];
                while !self.eat("}") {
                    self.expect("field")?;
                    let name = self.word()?;
                    self.expect("=")?;
                    let function = FuncId::read(self)?;
                    fields.push(ComputedField { name, function });
                    self.line()?;
                }
                DeclarationKind::Cap { owner: owner.ok_or_else(|| self.error(DiagnosticKind::Structure,
                "cap owner"))?, fields }
            }
            _ => {
                self.expect("{")?;
                self.line()?;
                let mut fields = vec![];
                let mut witnesses = vec![];
                let mut destroy = None;
                while !self.eat("}") {
                    if self.eat("field") {
                        let name = self.word()?;
                        self.expect(":")?;
                        let ty = TypeId::read(self)?;
                        let default = if self.eat("=") {
                            Some(FuncId::read(self)?)
                        } else {
                            None
                        };
                        let once = self.eat("once");
                        if self.first {
                            self.names.insert((id as usize, "field", name.clone()), fields.len() as u32);
                        }
                        fields.push(Field { name, ty, default, once });
                    } else if self.eat("does") {
                        let role = TypeId::read(self)?;
                        self.expect("{")?;
                        let mut slots = vec![];
                        while !self.is("}") {
                            self.word()?;
                            self.expect("=")?;
                            slots.push(FuncId::read(self)?);
                            if !self.eat(",") {
                                break;
                            }
                        }
                        self.expect("}")?;
                        witnesses.push(Witness { role, slots });
                    } else {
                        self.expect("destroy")?;
                        destroy = Some(FuncId::read(self)?);
                    }
                    self.line()?;
                }
                DeclarationKind::Object { fields, witnesses, destroy, builtin }
            }
        };
        let loc = self.location()?;
        self.line()?;
        self.generic_owner = None;
        Ok(Declaration { name, type_params, kind, loc })
    }
}

fn generic_scope(owner: GenericOwner) -> usize {
    match owner {
        GenericOwner::Function(id) => id.as_u32() as usize * 2 + 1,
        GenericOwner::Declaration(id) => id.as_u32() as usize * 2 + 2,
    }
}

impl Writer<'_> {
    pub(super) fn word(&mut self, word: &str) -> Result<(), PrintError> {
        if word.is_empty() || word != "$pending" && !word.chars().all(|c| c.is_alphanumeric() || "_./:#-".contains(c)) {
            return Err(PrintError::new("unrepresentable identifier or module path"));
        }
        self.push(word);
        Ok(())
    }

    pub(super) fn location(&mut self, loc: Option<DebugLoc>) -> Result<(), PrintError> {
        if let Some(loc) = loc {
            self.push(" ");
            loc.write(self)?;
        }
        Ok(())
    }

    pub(super) fn type_parameters(&mut self, params: &[TypeParameter]) -> Result<(), PrintError> {
        if params.is_empty() {
            return Ok(());
        }
        self.push("<");
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            self.word(&param.name)?;
            if !param.bounds.is_empty() {
                self.push(": ");
                for (j, bound) in param.bounds.iter().enumerate() {
                    if j > 0 {
                        self.push(" + ");
                    }
                    bound.write(self)?;
                }
            }
        }
        self.push(">");
        Ok(())
    }

    pub(super) fn attributes(&mut self, attrs: &[FunctionAttr]) -> Result<(), PrintError> {
        if attrs.is_empty() {
            return Ok(());
        }
        self.push(" [");
        for (i, attr) in attrs.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            match attr {
                FunctionAttr::Entry => self.push("entry"),
                FunctionAttr::Async => self.push("async"),
                FunctionAttr::Hidden => self.push("private"),
                FunctionAttr::Method(id) | FunctionAttr::Ctor(id) | FunctionAttr::Successor(id)
                    | FunctionAttr::Destroy(id) => {
                    self.push(match attr {
                        FunctionAttr::Method(_) => "method ", FunctionAttr::Ctor(_) => "ctor ",
                        FunctionAttr::Successor(_) => "successor ", _ => "destroy ",
                    });
                    id.write(self)?;
                }
                FunctionAttr::Default { object, field } => {
                    self.push("default ");
                    object.write(self)?;
                    self.push(".");
                    let name = self.module.declarations.get(*object).ok().and_then(|decl| match &decl.kind {
                        DeclarationKind::Object { fields, .. } => fields.get(*field as usize).map(|f| f.name.as_str()),
                        _ => None,
                    });
                    if let Some(name) = name {
                        self.word(name)?;
                    } else {
                        field.write(self)?;
                    }
                }
                FunctionAttr::Extern(name) => {
                    self.push("extern ");
                    name.write(self)?;
                }
                FunctionAttr::Intrinsic { name, objects, helpers } => {
                    self.push("intrinsic ");
                    name.write(self)?;
                    if !objects.is_empty() {
                        self.push(" objects ");
                        objects.write(self)?;
                    }
                    if !helpers.is_empty() {
                        self.push(" helpers ");
                        helpers.write(self)?;
                    }
                }
                FunctionAttr::Lambda { captures, owner } => {
                    self.push("lambda");
                    if *captures != 0 {
                        self.push(" captures ");
                        captures.write(self)?;
                    }
                    if let Some(owner) = owner {
                        self.push(" owner ");
                        owner.write(self)?;
                    }
                }
            }
        }
        self.push("]");
        Ok(())
    }

    pub(super) fn summary_reference(&mut self, name: &str, id: Option<SummaryId>) -> Result<(), PrintError> {
        if let Some(id) = id {
            self.push(" ");
            self.push(name);
            self.push("(");
            id.write(self)?;
            self.push(")");
        }
        Ok(())
    }

    pub(super) fn declaration(&mut self, id: DeclId, decl: &Declaration) -> Result<(), PrintError> {
        self.generic_owner = Some(GenericOwner::Declaration(id));
        let tag = match &decl.kind {
            DeclarationKind::Object { .. } => "object ", DeclarationKind::Role {
                    ..
            } => "role ",
            DeclarationKind::Enum { .. } => "enum ", DeclarationKind::Variant {
                    ..
            } => "variant ",
            DeclarationKind::Handle => "native object ", DeclarationKind::Cap {
                    ..
            } => "cap ",
            DeclarationKind::Global { .. } => "global ",
        };
        self.push("\n");
        self.push(tag);
        if let DeclarationKind::Cap {
            owner, ..
        }
        = &decl.kind {
            owner.write(self)?;
            self.push(" :: ");
        }
        id.write(self)?;
        self.type_parameters(&decl.type_params)?;
        match &decl.kind {
            DeclarationKind::Object { fields, witnesses, destroy, builtin } => {
                if *builtin {
                    self.push(" [builtin]");
                }
                self.push(" {\n");
                for field in fields {
                    self.push("    field ");
                    self.word(&field.name)?;
                    self.push(": ");
                    field.ty.write(self)?;
                    if let Some(default) = field.default {
                        self.push(" = ");
                        default.write(self)?;
                    }
                    if field.once {
                        self.push(" once");
                    }
                    self.push("\n");
                }
                for witness in witnesses {
                    self.push("    does ");
                    witness.role.write(self)?;
                    self.push(" {");
                    for (i, slot) in witness.slots.iter().enumerate() {
                        if i > 0 {
                            self.push(", ");
                        }
                        let role = match self.module.types.get(witness.role) {
                            Ok(Type::Role { decl, .. }) => self.module.declarations.get(*decl).ok(),
                            _ => None,
                        };
                        let name = role.and_then(|decl| match &decl.kind {
                            DeclarationKind::Role { methods, .. } => methods.get(i).map(|m| m.name.as_str()),
                            _ => None,
                        });
                        if let Some(name) = name {
                            self.word(name)?;
                        } else {
                            self.push(&format!("slot{i}"));
                        }
                        self.push(" = ");
                        slot.write(self)?;
                    }
                    self.push("}\n");
                }
                if let Some(destroy) = destroy {
                    self.push("    destroy ");
                    destroy.write(self)?;
                    self.push("\n");
                }
                self.push("}");
            }
            DeclarationKind::Role { methods, builtin } => {
                if *builtin {
                    self.push(" [builtin]");
                }
                self.push(" {\n");
                for method in methods {
                    self.push("    ");
                    self.word(&method.name)?;
                    method.signature.write(self)?;
                    self.push("\n");
                }
                self.push("}");
            }
            DeclarationKind::Enum { cases, is_error } => {
                if *is_error {
                    self.push(" is Error");
                }
                self.push(" = (");
                for (i, case) in cases.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.word(case)?;
                }
                self.push(")");
            }
            DeclarationKind::Variant { cases, is_error } => {
                if *is_error {
                    self.push(" is Error");
                }
                self.push(" = (");
                for (i, case) in cases.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.word(&case.name)?;
                    self.push(": ");
                    case.layout.write(self)?;
                }
                self.push(")");
            }
            DeclarationKind::Handle => {}
            DeclarationKind::Cap { fields, .. } => {
                self.push(" {\n");
                for field in fields {
                    self.push("    field ");
                    self.word(&field.name)?;
                    self.push(" = ");
                    field.function.write(self)?;
                    self.push("\n");
                }
                self.push("}");
            }
            DeclarationKind::Global { ty, init } => {
                self.push(": ");
                ty.write(self)?;
                self.push(" = ");
                init.write(self)?;
            }
        }
        self.location(decl.loc)?;
        self.push("\n");
        self.generic_owner = None;
        Ok(())
    }
}
