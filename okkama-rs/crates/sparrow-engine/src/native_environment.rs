//! Environment snapshots and legacy mutation are local to the VM heap. No
//! process-global set_var, thread-local VM singleton, or unsafe mutation.
use crate::{
    Error,
    native::{Outcome, ProviderError},
    value::*,
};
use okkama_runtime_abi::{Native, NativeFailure};
use std::collections::BTreeMap;

fn host_snapshot(
    values: impl Iterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
) -> BTreeMap<String, String> {
    values
        // This marker belongs to the launcher/CLI handshake, not the program's
        // environment or an ordinary subprocess launched by that program.
        .filter(|(key, _)| key != "SPARROW_INTERNAL_WORKER")
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect()
}
pub(crate) fn handles(n: Native) -> bool {
    n.symbol().starts_with("runtime/environment@1::")
}
fn failed(n: Native, code: &str, message: &str) -> ProviderError {
    ProviderError::Native(NativeFailure {
        domain: "core.environment".into(),
        code: code.into(),
        operation: n.symbol().into(),
        message: message.into(),
    })
}
fn name(n: Native, s: &str, normalize: bool) -> Result<String, ProviderError> {
    if s.is_empty() {
        return Err(failed(
            n,
            "invalid_input",
            "Environment name must not be empty",
        ));
    }
    if s.contains('\0') {
        return Err(failed(
            n,
            "invalid_input",
            "Environment name contains a zero byte",
        ));
    }
    if s.contains('=') {
        return Err(failed(
            n,
            "invalid_input",
            "Environment name contains \"=\"",
        ));
    }
    Ok(if normalize && cfg!(windows) {
        s.to_uppercase()
    } else {
        s.into()
    })
}
fn value(n: Native, s: &str) -> Result<String, ProviderError> {
    if s.contains('\0') {
        return Err(failed(
            n,
            "invalid_input",
            "Environment value contains a zero byte",
        ));
    }
    Ok(s.into())
}
impl Heap {
    pub(crate) fn environment(&mut self) -> &mut BTreeMap<String, String> {
        self.environment
            .get_or_insert_with(|| host_snapshot(std::env::vars_os()))
    }
    fn environment_map(
        &self,
        n: Native,
        slot: Slot,
    ) -> Result<BTreeMap<String, String>, ProviderError> {
        if slot.tag != MAP {
            return Err(Error("expected environment map".into()).into());
        }
        let mut out = BTreeMap::new();
        for &(key, content) in self.maps.get(slot.bits)?.values() {
            let name = name(n, self.text(key)?, true)?;
            let content = value(n, self.text(content)?)?;
            if out.insert(name, content).is_some() {
                return Err(failed(
                    n,
                    "invalid_input",
                    "Environment contains names that collide on this platform",
                ));
            }
        }
        Ok(out)
    }
    fn environment_publish(&mut self, values: BTreeMap<String, String>) -> Result<Slot, Error> {
        let mut entries = Vec::with_capacity(values.len());
        for (key, value) in values {
            entries.push((self.allocate_text(key)?, self.allocate_text(value)?));
        }
        self.map_new(entries.into_iter())
    }
    pub(crate) fn native_environment(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        match self.environment_execute(n, a) {
            Ok(v) => Ok(Outcome::Value(v)),
            Err(ProviderError::Engine(e)) => Err(e),
            Err(ProviderError::Native(e)) => {
                let payload = self.native_failure_new(e)?;
                self.failure_new(crate::failure::FailureKind::Native(payload))
                    .map(Outcome::Failure)
            }
        }
    }
    fn environment_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        match n {
            EnvironmentEnvironmentSnapshot => {
                let mut out = BTreeMap::new();
                for (key, content) in self.environment().iter() {
                    let key = name(n, key, true)?;
                    let content = value(n, content)?;
                    if out.insert(key, content).is_some() {
                        return Err(failed(
                            n,
                            "invalid_data",
                            "Process environment contains colliding variable names",
                        ));
                    }
                }
                Ok(self.environment_publish(out)?)
            }
            EnvironmentEnvironmentNormalize
            | EnvironmentEnvironmentAdd
            | EnvironmentEnvironmentDiff => {
                let mut out = self.environment_map(n, a[0])?;
                if n == EnvironmentEnvironmentAdd {
                    out.insert(
                        name(n, self.text(a[1])?, true)?,
                        value(n, self.text(a[2])?)?,
                    );
                }
                if n == EnvironmentEnvironmentDiff {
                    out.remove(&name(n, self.text(a[1])?, true)?);
                }
                Ok(self.environment_publish(out)?)
            }
            EnvironmentEnvironmentName => {
                Ok(self.allocate_text(name(n, self.text(a[0])?, true)?)?)
            }
            EnvironmentEnvironmentHome => {
                let values = self.environment_map(n, a[0])?;
                let home = if cfg!(windows) {
                    values
                        .get("USERPROFILE")
                        .filter(|s| !s.is_empty())
                        .cloned()
                        .or_else(|| {
                            Some(format!(
                                "{}{}",
                                values.get("HOMEDRIVE")?,
                                values.get("HOMEPATH")?
                            ))
                        })
                } else {
                    values.get("HOME").cloned()
                };
                Ok(self
                    .allocate_text(home.filter(|s| !s.is_empty()).ok_or_else(|| {
                        failed(n, "not_found", "Home directory is unavailable")
                    })?)?)
            }
            EnvironmentEnvironmentPathSeparator => {
                Ok(self.allocate_text(if cfg!(windows) { ";" } else { ":" }.into())?)
            }
            EnvironmentEnvironmentGet
            | EnvironmentEnvironmentSet
            | EnvironmentEnvironmentRemove => {
                let bytes = self.native_bytes(a[0])?;
                let key = name(n, &String::from_utf8_lossy(&bytes), false)?;
                match n {
                    EnvironmentEnvironmentGet => {
                        let bytes = self
                            .environment()
                            .get(&key)
                            .ok_or_else(|| {
                                failed(n, "not_found", "Environment variable was not found")
                            })?
                            .as_bytes()
                            .to_vec();
                        Ok(self.native_byte_array(bytes)?)
                    }
                    EnvironmentEnvironmentSet => {
                        let raw = self.native_bytes(a[1])?;
                        let content = value(n, &String::from_utf8_lossy(&raw))?;
                        self.environment().insert(key, content);
                        Ok(Slot::boolean(true))
                    }
                    _ => Ok(Slot::boolean(self.environment().remove(&key).is_some())),
                }
            }
            EnvironmentEnvironmentAll => {
                let values = self.environment().clone();
                let mut all = Vec::with_capacity(values.len());
                for (key, value) in values {
                    let key = self.native_byte_array(key.into_bytes())?;
                    let value = self.native_byte_array(value.into_bytes())?;
                    all.push(self.allocate_array(vec![key, value])?);
                }
                Ok(self.allocate_array(all)?)
            }
            _ => Err(Error("unsupported environment native".into()).into()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_snapshot_excludes_worker_launch_marker() {
        let values = host_snapshot(
            [
                ("SPARROW_INTERNAL_WORKER".into(), "1".into()),
                ("USER_SETTING".into(), "kept".into()),
            ]
            .into_iter(),
        );
        assert_eq!(values.len(), 1);
        assert_eq!(values.get("USER_SETTING").map(String::as_str), Some("kept"));
    }
    fn call(h: &mut Heap, n: Native, a: &[Slot]) -> Slot {
        match h.native_environment(n, a).unwrap() {
            Outcome::Value(v) => v,
            Outcome::Failure(_) => panic!("unexpected environment failure"),
        }
    }
    #[test]
    fn mutation_is_heap_local_snapshots_are_immutable_and_host_is_unchanged() {
        let mut first = Heap::new(&[], &[]);
        let mut second = Heap::new(&[], &[]);
        first.environment = Some(BTreeMap::new());
        second.environment = Some(BTreeMap::new());
        let key = format!("OKKAMA_NATIVE_TEST_{}", std::process::id());
        let host_before = std::env::var_os(&key);
        let name = first.native_byte_array(key.as_bytes().to_vec()).unwrap();
        let text = first.native_byte_array(b"saved".to_vec()).unwrap();
        call(&mut first, Native::EnvironmentEnvironmentSet, &[name, text]);
        let snapshot = call(&mut first, Native::EnvironmentEnvironmentSnapshot, &[]);
        call(&mut first, Native::EnvironmentEnvironmentRemove, &[name]);
        assert!(first.environment().is_empty());
        assert!(second.environment().is_empty());
        let key_slot = first.allocate_text(key.clone()).unwrap();
        let saved = first.map_lookup(snapshot, key_slot).unwrap().unwrap();
        assert_eq!(first.text(saved).unwrap(), "saved");
        assert_eq!(std::env::var_os(&key), host_before);
        first.collect([snapshot].into_iter()).unwrap();
        assert_eq!(first.maps.get(snapshot.bits).unwrap().len(), 1);
    }
    #[test]
    fn invalid_names_values_and_missing_home_are_typed_errors() {
        let mut h = Heap::new(&[], &[]);
        for invalid in ["", "bad=name", "zero\0byte"] {
            let s = h.allocate_text(invalid.into()).unwrap();
            assert!(matches!(
                h.native_environment(Native::EnvironmentEnvironmentName, &[s])
                    .unwrap(),
                Outcome::Failure(_)
            ));
        }
        let empty = h.map_new(std::iter::empty()).unwrap();
        assert!(matches!(
            h.native_environment(Native::EnvironmentEnvironmentHome, &[empty])
                .unwrap(),
            Outcome::Failure(_)
        ));
        let key = h.native_byte_array(b"valid".to_vec()).unwrap();
        let invalid = h.native_byte_array(b"zero\0byte".to_vec()).unwrap();
        assert!(matches!(
            h.native_environment(Native::EnvironmentEnvironmentSet, &[key, invalid])
                .unwrap(),
            Outcome::Failure(_)
        ));
    }
}
