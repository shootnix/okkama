//! Bounded semantic fixture executor, independent of the lowering algorithm.
//! Serial inputs and expected traces come from Champ G1 §6.5, not from Low plans.
use super::tests::{body, checked};
use crate::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Failure {
    kind: FailureKind,
    error: &'static str,
    cleanup_origin: bool,
    secondaries: Vec<&'static str>,
}
impl Failure {
    fn new(error: &'static str) -> Self {
        Self { kind: FailureKind::Panic, error, cleanup_origin: false, secondaries: vec![] }
    }
}
#[derive(Clone, Debug)]
struct Activation {
    remaining: Vec<usize>,
    reserve: Option<Vec<usize>>,
    current: Option<usize>,
    failure: Option<Failure>,
    needs_record: bool,
    settled: bool,
}
impl Activation {
    fn failure(&mut self, failure: Failure) {
        if let Some(primary) = &mut self.failure {
            primary.secondaries.push(failure.error);
        } else {
            self.failure = Some(failure);
        }
        if let Some(reserve) = self.reserve.take() {
            self.remaining.extend(reserve);
        }
        self.needs_record = false;
    }
}
#[derive(Clone, Debug)]
enum Value {
    Unit,
    Identity(usize),
    Packet(Vec<usize>),
    State(Activation),
    Key(Option<usize>),
    Bool(bool),
    Failure(Failure),
    Failures(Option<Failure>),
    Exit(Option<Vec<usize>>),
}
#[derive(Default)]
struct Executor {
    values: BTreeMap<ValueId, Value>,
    // name, creation serial, optional destroy failure
    identities: Vec<(&'static str, u64, Option<&'static str>)>,
    dead: Vec<usize>,
    trace: Vec<&'static str>,
    terminal: Option<Failure>,
    risky_failure: bool,
    defer_failure: bool,
    creation_failure: bool,
    created: usize,
}
impl Executor {
    fn destroy_current(&mut self, mut state: Activation) -> Vec<Value> {
        let identity = state.current.take().unwrap();
        assert!(!self.dead.contains(&identity));
        let (name, _, error) = self.identities[identity];
        self.trace.push(name);
        self.dead.push(identity);
        let failure = error.map(|error| {
            let mut failure = Failure::new(error);
            failure.cleanup_origin = true;
            failure
        });
        state.needs_record = failure.is_some();
        vec![Value::State(state), Value::Failures(failure)]
    }
    fn packet(&mut self, value: ValueId) -> Vec<usize> {
        let Value::Packet(packet) = self.values.remove(&value).unwrap() else {
            panic!("expected packet");
        };
        packet
    }
    fn state(&mut self, value: ValueId) -> Activation {
        let Value::State(state) = self.values.remove(&value).unwrap() else {
            panic!("expected cleanup state");
        };
        state
    }
    fn failure_view(&self, value: ValueId) -> Failure {
        let Value::Failure(failure) = &self.values[&value] else {
            panic!("expected failure view");
        };
        failure.clone()
    }
    fn edge(&mut self, body: &Body, edge: &InvokeEdge, produced: &[Value]) -> BlockId {
        let args: Vec<_> = edge.args.iter().map(|arg| match arg {
            EdgeArg::Value(value) => self.values.remove(value).unwrap(),
            EdgeArg::Produced(result) => produced[usize::from(result.slot)].clone(),
        }).collect();
        for (param, value) in body.blocks.get(edge.block).unwrap().params.iter().zip(args) {
            self.values.insert(*param, value);
        }
        edge.block
    }
    fn target(&mut self, body: &Body, target: &Target) -> BlockId {
        let edge = InvokeEdge {
            block: target.block, args: target.args.iter().copied().map(EdgeArg::Value).collect(),
        };
        self.edge(body, &edge, &[])
    }
    fn run(&mut self, module: &Module) {
        let body = body(module);
        let mut block = body.entry;
        for _ in 0..1000 {
            let data = body.blocks.get(block).unwrap();
            for instruction in &data.insts {
                let inst = body.insts.get(*instruction).unwrap();
                let result = match &inst.op {
                    Operation::Const(_) | Operation::Core(CoreOp::TypeOf { .. }) => vec![Value::Unit],
                    Operation::Exit(ExitOp::Prepare { exit }) => {
                        vec![Value::Packet(self.packet(exit.packet.unwrap()))]
                    }
                    Operation::Ownership(OwnershipOp::ScopeBegin { .. } | OwnershipOp::Empty {}) => {
                        vec![Value::Packet(vec![])]
                    }
                    Operation::Ownership(OwnershipOp::Admit { state, packet }
                        | OwnershipOp::Accept { state, packet }) => {
                        let mut values = self.packet(*state);
                        values.extend(self.packet(*packet));
                        vec![Value::Packet(values)]
                    }
                    Operation::Core(CoreOp::FailSecondary { primary, cleanup }) => {
                        let mut failure = self.failure_view(*primary);
                        failure.secondaries.push(self.failure_view(*cleanup).error);
                        vec![Value::Failure(failure)]
                    }
                    Operation::Cleanup(CleanupOp::Begin { state, pending, failure, .. }) => {
                        let remaining = self.packet(*state);
                        let reserve = pending.map(|value| self.packet(value));
                        let failure = failure.map(|value| self.failure_view(value));
                        let mut state = Activation {
                            remaining, reserve, current: None, failure: None, needs_record: false, settled: false,
                        };
                        if let Some(failure) = failure {
                            state.failure(failure);
                        }
                        vec![Value::State(state)]
                    }
                    Operation::Cleanup(CleanupOp::SettleEmpty { cleanup, schedule }) => {
                        assert!(body.cleanup.schedules.get(*schedule).unwrap().holders.is_empty());
                        let mut state = self.state(*cleanup);
                        state.settled = true;
                        vec![Value::State(state)]
                    }
                    Operation::Cleanup(CleanupOp::Plan { cleanup, plan }) => {
                        let state = self.state(*cleanup);
                        assert!(state.settled);
                        let plan = body.cleanup.plans.get(*plan).unwrap();
                        let close = body.cleanup.closes.get(plan.close).unwrap();
                        assert_eq!(close.reserve,
                            plan.sources.contains(&MergeSource::WholeReserveOnFailure));
                        vec![Value::State(state)]
                    }
                    Operation::Cleanup(CleanupOp::Next { cleanup }) => {
                        let mut state = self.state(*cleanup);
                        assert!(!state.needs_record && state.current.is_none());
                        let next = state.remaining.iter().enumerate()
                            .max_by_key(|(_, identity)| self.identities[**identity].1).map(|(index, _)| index);
                        state.current = next.map(|index| state.remaining.remove(index));
                        let selected = state.current;
                        vec![Value::State(state), Value::Key(selected), Value::Bool(selected.is_some())]
                    }
                    Operation::Cleanup(CleanupOp::Destroy { cleanup, key }) => {
                        let mut state = self.state(*cleanup);
                        let Value::Key(selected) = self.values[key] else {
                            panic!("expected selected key");
                        };
                        let identity = state.current.take().unwrap();
                        assert_eq!(selected, Some(identity));
                        state.current = Some(identity);
                        self.destroy_current(state)
                    }
                    Operation::Cleanup(CleanupOp::DestroyDirect { cleanup, identity, .. }) => {
                        let mut state = self.state(*cleanup);
                        assert!(!state.needs_record && state.current.is_none());
                        let Value::Identity(identity) = self.values[identity] else {
                            panic!("expected direct identity");
                        };
                        let position = state.remaining.iter().position(|entry| *entry == identity).unwrap();
                        state.current = Some(state.remaining.remove(position));
                        self.destroy_current(state)
                    }
                    Operation::Cleanup(CleanupOp::RecordFailure { cleanup, failure }) => {
                        let mut state = self.state(*cleanup);
                        assert!(state.current.is_none() && state.needs_record);
                        state.failure(self.failure_view(*failure));
                        vec![Value::State(state)]
                    }
                    Operation::Cleanup(CleanupOp::Finish { cleanup }) => {
                        let state = self.state(*cleanup);
                        assert!(state.remaining.is_empty() && state.current.is_none() && !state.needs_record);
                        assert!(state.failure.is_none() || state.reserve.is_none());
                        vec![Value::Unit, Value::Failures(state.failure), Value::Exit(state.reserve)]
                    }
                    op => panic!("unsupported test instruction {}", op.opcode()),
                };
                for (value, result) in inst.results.iter().zip(result) {
                    self.values.insert(*value, result);
                }
            }
            block = match &data.term.kind {
                TermKind::CleanupNext { cleanup, selected, done } => {
                    let mut state = self.state(*cleanup);
                    assert!(!state.needs_record && state.current.is_none());
                    let next = state.remaining.iter().enumerate()
                        .max_by_key(|(_, identity)| self.identities[**identity].1).map(|(index, _)| index);
                    state.current = next.map(|index| state.remaining.remove(index));
                    if let Some(key) = state.current {
                        self.edge(body, selected, &[Value::State(state), Value::Key(Some(key))])
                    } else {
                        self.edge(body, done, &[Value::State(state)])
                    }
                }
                TermKind::Br(target) => self.target(body, target),
                TermKind::Cond { condition, yes, no } => {
                    let Value::Bool(condition) = self.values[condition] else {
                        panic!("expected condition");
                    };
                    self.target(body, if condition { yes } else { no })
                }
                TermKind::FailureDispatch { failure, none, some } => {
                    let Value::Failures(failure) = self.values[failure].clone() else {
                        panic!("expected captured failure");
                    };
                    if let Some(failure) = failure {
                        self.edge(body, some, &[Value::Failure(failure)])
                    } else {
                        self.target(body, none)
                    }
                }
                TermKind::ExitDispatch { failure, exit, success, pending, failed, .. } => {
                    let Value::Failures(failure) = self.values[failure].clone() else {
                        panic!("expected terminal failures");
                    };
                    let Value::Exit(reserve) = self.values.remove(exit).unwrap() else {
                        panic!("expected linear exit state");
                    };
                    if let Some(failure) = failure {
                        assert!(reserve.is_none());
                        self.edge(body, failed.as_ref().unwrap(), &[Value::Unit, Value::Failure(failure)])
                    } else if let Some(reserve) = reserve {
                        self.edge(body, pending.as_ref().unwrap(), &[Value::Unit, Value::Packet(reserve)])
                    } else {
                        self.edge(body, success.as_ref().unwrap(), &[Value::Unit])
                    }
                }
                TermKind::InvokeOwn { op, normal, .. }
                    if matches!(op.as_ref(), Operation::Core(CoreOp::ObjNew { .. })) => {
                    let Operation::Core(CoreOp::ObjNew { packet, .. }) = op.as_ref() else {
                        unreachable!();
                    };
                    let mut packet = self.packet(packet.unwrap());
                    let identity = self.created;
                    self.created += 1;
                    packet.push(identity);
                    self.edge(body, normal, &[Value::Identity(identity), Value::Packet(packet)])
                }
                TermKind::InvokeOwn { op, normal, unwind } => {
                    let Operation::Call(CallOp::Direct { function, call }) = op.as_ref() else {
                        panic!("expected call");
                    };
                    if let Some(packet) = call.packet {
                        assert!(self.packet(packet).is_empty());
                    }
                    let name = &module.functions.get(*function).unwrap().symbol;
                    let failed = if name == "new" { self.creation_failure } else { self.risky_failure };
                    if name == "old" {
                        let identity = self.values[&call.args[0].value].clone();
                        self.edge(body, normal, &[identity, Value::Packet(vec![]), Value::Unit])
                    } else if failed {
                        self.edge(body, unwind, &[Value::Failure(Failure::new("body"))])
                    } else if name == "new" {
                        self.edge(body, normal, &[Value::Unit, Value::Packet(vec![0]), Value::Unit])
                    } else {
                        self.edge(body, normal, &[Value::Packet(vec![]), Value::Unit])
                    }
                }
                TermKind::InvokeDefer { call, continue_edge, unwind, .. } => {
                    assert!(self.packet(call.packet.unwrap()).is_empty());
                    self.trace.push("d");
                    if self.defer_failure {
                        self.edge(body, unwind.as_ref().unwrap(), &[Value::Failure(Failure::new("defer"))])
                    } else {
                        self.edge(body, continue_edge, &[])
                    }
                }
                TermKind::Ret { values } => {
                    if let Some(value) = values.first()
                        && let Value::Failure(failure) = &self.values[value] {
                        self.terminal = Some(failure.clone());
                    }
                    return;
                }
                TermKind::Raise(value) => {
                    self.terminal = Some(self.failure_view(*value));
                    return;
                }
                term => panic!("unsupported test terminator {term:?}"),
            };
        }
        panic!("executor did not terminate within the fixture budget");
    }
}
fn reserve_case(source: &str, identities: Vec<(&'static str, u64, Option<&'static str>)>) -> Executor {
    let module = checked(source);
    let params = &body(&module).blocks.get(body(&module).entry).unwrap().params;
    let mut executor = Executor { identities, ..Executor::default() };
    let ordinary: Vec<_> = (0..executor.identities.len()).filter(|identity| *identity != 1).collect();
    executor.values.insert(params[0], Value::Packet(ordinary));
    executor.values.insert(params[1], Value::Packet(vec![1]));
    if let Some(active) = params.get(2) {
        executor.values.insert(*active, Value::Failure(Failure::new("defer")));
    }
    executor.run(&module);
    executor
}
#[test]
fn whole_reserve_before_destroy_and_after_dead_current() {
    let identities = vec![("A", 1, None), ("R", 2, None)];
    let before = reserve_case(include_str!("fixtures/unwind-reserve.own.oir"), identities);
    assert_eq!(before.trace, ["R", "A"]);
    assert_eq!(before.terminal.unwrap().error, "defer");
    let after = reserve_case(include_str!("fixtures/end-reserve.own.oir"),
        vec![("A", 1, Some("A failed")), ("R", 2, None)]);
    assert_eq!(after.trace, ["A", "R"]);
    let failure = after.terminal.unwrap();
    assert_eq!(failure.kind, FailureKind::Panic);
    assert!(failure.cleanup_origin);
    let suffix = reserve_case(include_str!("fixtures/end-reserve.own.oir"),
        vec![("A", 1, None), ("R", 2, Some("R failed")), ("C", 3, Some("C failed"))]);
    assert_eq!(suffix.trace, ["C", "R", "A"]);
    let failure = suffix.terminal.unwrap();
    assert_eq!(failure.error, "C failed");
    assert_eq!(failure.secondaries, ["R failed"]);
}
#[test]
fn risky_paths_run_exactly_the_reached_defer_prefix() {
    let module = checked(include_str!("fixtures/spec16-4.own.oir"));
    for (creation_failure, risky_failure, defer_failure, expected) in [
        (false, false, false, vec!["d", "R"]),
        (false, true, false, vec!["d", "R"]),
        (false, true, true, vec!["d", "R"]),
        (true, false, false, vec![]),
    ] {
        let mut executor = Executor {
            identities: vec![("R", 1, None)], creation_failure, risky_failure, defer_failure,
            ..Executor::default()
        };
        let param = body(&module).blocks.get(body(&module).entry).unwrap().params[0];
        executor.values.insert(param, Value::Unit);
        executor.run(&module);
        assert_eq!(executor.trace, expected);
        if risky_failure && defer_failure {
            let failure = executor.terminal.unwrap();
            assert_eq!(failure.error, "body");
            assert_eq!(failure.secondaries, ["defer"]);
        }
    }
}

#[test]
fn direct_failure_continues_suffix_once_and_old_result_uses_serial_merge() {
    for source in [
        include_str!("fixtures/local-pair.own.oir"),
        include_str!("fixtures/return-old.own.oir"),
    ] {
        let module = checked(source);
        let param = body(&module).blocks.get(body(&module).entry).unwrap().params[0];
        let mut executor = Executor {
            identities: vec![("A", 1, Some("A failed")), ("B", 2, Some("B failed"))],
            ..Executor::default()
        };
        executor.values.insert(param, Value::Unit);
        executor.run(&module);
        assert_eq!(executor.trace, ["B", "A"]);
        assert_eq!(executor.dead, [1, 0]);
        let failure = executor.terminal.unwrap();
        assert_eq!(failure.error, "B failed");
        assert_eq!(failure.secondaries, ["A failed"]);
    }
}

#[test]
fn finite_local_triplet_uses_creation_order() {
    let module = checked(&super::direct_tests::triplet_source());
    let param = body(&module).blocks.get(body(&module).entry).unwrap().params[0];
    let mut executor = Executor {
        identities: vec![("a", 1, None), ("b", 2, None), ("h", 3, None)],
        ..Executor::default()
    };
    executor.values.insert(param, Value::Unit);
    executor.run(&module);
    assert_eq!(executor.trace, ["h", "b", "a"]);
    assert_eq!(executor.dead, [2, 1, 0]);
}
