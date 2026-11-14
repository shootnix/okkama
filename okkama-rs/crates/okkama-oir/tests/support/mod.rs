use okkama_oir::verify::ExternalContracts;

// The fixture files are the frozen native test ABI catalogue. Production
// verification never registers external contracts from input declarations.
pub struct FixtureContracts<'a>(pub &'a okkama_oir::Module);
impl ExternalContracts for FixtureContracts<'_> {
    fn native(&self, name: &str) -> Option<&okkama_oir::Signature> {
        self.0.functions.iter().find_map(|(_, f)| f.attrs.iter().any(|a| {
            matches!(a, okkama_oir::FunctionAttr::Extern(n) if n == name && n.starts_with("fixture."))
        }).then_some(&f.signature))
    }
    fn intrinsic(&self, _: &str) -> Option<&okkama_oir::Signature> { None }
    fn native_summary(&self, name: &str) -> Option<&okkama_oir::Summary> {
        self.0.functions.iter().find_map(|(_, f)| f.attrs.iter().any(|a| {
            matches!(a, okkama_oir::FunctionAttr::Extern(n) if n == name && n.starts_with("fixture."))
        }).then_some(f.summary).flatten()).and_then(|id| self.0.summaries.get(id).ok())
    }
}

