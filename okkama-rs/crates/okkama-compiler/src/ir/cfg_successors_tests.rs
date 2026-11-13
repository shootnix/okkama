//! Frozen pre-#469 dominator implementation, used only as a differential oracle.
use super::*;
use crate::ir::{Block, CatchType, ObjectId, ResultType, Target, TermKind, Terminator, TrapCode};
use crate::{SourceFile, Span, codegen, lowering, syntax};

fn legacy_compute(function: &Function) -> Result<Dominators, String> {
    let count = function.blocks.len() * 2;
    let node_successors = |node: usize| -> Vec<usize> {
        let block = BlockId(node / 2);
        if node.is_multiple_of(2) {
            let mut next = vec![body(block)];
            if let Some(unwind) = &function.blocks[block.0].unwind {
                next.push(start(unwind.block));
            }
            next
        } else {
            successors(function, block).into_iter().map(start).collect()
        }
    };
    let mut order = Vec::with_capacity(count);
    let mut visited = vec![false; count];
    // Итеративный DFS с явным стеком: глубина CFG не ограничена стеком потока.
    let mut stack = vec![(0usize, 0usize)];
    visited[0] = true;
    while let Some((node, next)) = stack.pop() {
        let targets = node_successors(node);
        if let Some(target) = targets.get(next) {
            stack.push((node, next + 1));
            if !visited[*target] {
                visited[*target] = true;
                stack.push((*target, 0));
            }
        } else {
            order.push(node);
        }
    }
    if let Some(node) = visited.iter().position(|seen| !seen) {
        return Err(format!("block ^b{} is unreachable", node / 2));
    }
    order.reverse(); // reverse postorder
    let mut rpo_index = vec![0; count];
    for (position, node) in order.iter().enumerate() {
        rpo_index[*node] = position;
    }
    let mut predecessors = vec![Vec::new(); count];
    for node in 0..count {
        for target in node_successors(node) {
            predecessors[target].push(node);
        }
    }
    let mut idom: Vec<Option<usize>> = vec![None; count];
    idom[0] = Some(0);
    let intersect = |idom: &[Option<usize>], mut left: usize, mut right: usize| {
        while left != right {
            while rpo_index[left] > rpo_index[right] {
                left = idom[left].expect("processed node has an idom");
            }
            while rpo_index[right] > rpo_index[left] {
                right = idom[right].expect("processed node has an idom");
            }
        }
        left
    };
    let mut changed = true;
    while changed {
        changed = false;
        for node in order.iter().skip(1) {
            let mut new_idom: Option<usize> = None;
            for predecessor in &predecessors[*node] {
                if idom[*predecessor].is_none() {
                    continue;
                }
                new_idom = Some(match new_idom {
                    None => *predecessor,
                    Some(current) => intersect(&idom, *predecessor, current),
                });
            }
            if new_idom.is_some() && idom[*node] != new_idom {
                idom[*node] = new_idom;
                changed = true;
            }
        }
    }
    let idom: Vec<usize> = idom
        .into_iter()
        .map(|parent| parent.expect("every reachable node has an idom"))
        .collect();
    let mut children = vec![Vec::new(); count];
    for (node, &parent) in idom.iter().enumerate() {
        if parent != node {
            children[parent].push(node);
        }
    }
    let (mut enter, mut exit) = (vec![0; count], vec![0; count]);
    let mut clock = 0;
    let mut walk = vec![(0usize, 0usize)];
    enter[0] = clock;
    clock += 1;
    while let Some((node, next)) = walk.pop() {
        if let Some(&child) = children[node].get(next) {
            walk.push((node, next + 1));
            enter[child] = clock;
            clock += 1;
            walk.push((child, 0));
        } else {
            exit[node] = clock;
            clock += 1;
        }
    }
    Ok(Dominators { enter, exit })
}

fn legacy_successors(function: &Function, node: usize) -> Vec<usize> {
    let block = BlockId(node / 2);
    if node.is_multiple_of(2) {
        let mut next = vec![body(block)];
        if let Some(unwind) = &function.blocks[block.0].unwind {
            next.push(start(unwind.block));
        }
        next
    } else {
        successors(function, block).into_iter().map(start).collect()
    }
}

fn legacy_rpo(function: &Function) -> Vec<usize> {
    let mut seen = vec![false; function.blocks.len() * 2];
    let mut stack = vec![(0, 0)];
    let mut order = Vec::new();
    seen[0] = true;
    while let Some((node, next)) = stack.pop() {
        if let Some(&target) = legacy_successors(function, node).get(next) {
            stack.push((node, next + 1));
            if !seen[target] {
                seen[target] = true;
                stack.push((target, 0));
            }
        } else {
            order.push(node);
        }
    }
    order.reverse();
    order
}

fn equivalent(function: &Function) {
    let edges: Vec<Vec<usize>> = (0..function.blocks.len() * 2).map(|node| {
        let actual: Vec<_> = node_successors(function, node).collect();
        assert_eq!(actual, legacy_successors(function, node), "{} node {node}", function.symbol);
        actual
    }).collect();
    if let Ok(order) = reverse_postorder(function) {
        assert_eq!(order, legacy_rpo(function), "{} production RPO", function.symbol);
    }
    let (idom, rpo) = dominators(&edges);
    for (index, node) in legacy_rpo(function).into_iter().enumerate() {
        assert_eq!(rpo[node], index, "{} RPO", function.symbol);
    }
    match (legacy_compute(function), Dominators::compute(function)) {
        (Ok(expected), Ok(actual)) => {
            assert_eq!(actual.enter, expected.enter, "{} enter", function.symbol);
            assert_eq!(actual.exit, expected.exit, "{} exit", function.symbol);
            // Independently computed immediate tree, including minimality.
            for (node, parent) in idom.into_iter().enumerate() {
                let parent = parent.expect("reachable");
                assert!(actual.dominates_node(parent, node));
                for candidate in 0..edges.len() {
                    if candidate != node && actual.dominates_node(candidate, node) {
                        assert!(actual.dominates_node(candidate, parent));
                    }
                }
            }
        }
        (Err(expected), Err(actual)) => assert_eq!(actual, expected),
        (expected, actual) => panic!("{}: {expected:?} != {actual:?}", function.symbol),
    }
}

fn graph(terms: Vec<TermKind>, unwinds: Vec<Option<usize>>) -> Function {
    Function {
        symbol: "generated".into(), params: vec![], result: ResultType::Void,
        attrs: vec![], values: vec![], span: Span::new(0, 0),
        blocks: terms.into_iter().zip(unwinds).map(|(kind, unwind)| Block {
            params: vec![], insts: vec![], unwind: unwind.map(|block| Target::new(BlockId(block))),
            term: Terminator { kind, span: Span::new(0, 0) },
        }).collect(),
    }
}

#[test]
fn every_terminator_duplicate_edges_and_unwind_order() {
    let target = |block| Target::new(BlockId(block));
    let value = ValueId(0);
    let terms = vec![
        TermKind::Br(target(1)),
        TermKind::Cond { condition: value, then_target: target(2), else_target: target(2) },
        TermKind::Catch { failure: value, ty: CatchType::Object(ObjectId(0)), handler: target(3), next: target(4) },
        TermKind::ArrayLookup { array: value, index: value, found: target(4), missing: target(5) },
        TermKind::MapLookup { map: value, key: value, found: target(5), missing: target(6) },
        TermKind::Ret(None), TermKind::Ret(Some(value)), TermKind::Panic(value),
        TermKind::Trap { code: TrapCode::Unreachable, message: String::new() },
        TermKind::Unreachable, TermKind::Throw(value), TermKind::Raise(value),
    ];
    let mut unwinds: Vec<_> = (0..terms.len()).map(|i| Some((i + 1) % terms.len())).collect();
    unwinds[1] = Some(1);
    let function = graph(terms, unwinds);
    equivalent(&function);
    assert_eq!(node_successors(&function, start(BlockId(1))).collect::<Vec<_>>(), [3, 2]);
    assert_eq!(node_successors(&function, body(BlockId(1))).collect::<Vec<_>>(), [4, 4]);
}

#[test]
fn generated_cfgs_match_the_old_dfs_and_dominator_intervals() {
    // Bounded reproducible graphs: cycles, self edges, duplicate targets,
    // high fan-in and reachable/unreachable exceptional components.
    for seed in 0..256usize {
        let n = 1 + seed % 48;
        let mut state = seed as u64 + 1;
        let mut random = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            (state >> 32) as usize
        };
        let mut terms = Vec::new();
        let mut unwinds = Vec::new();
        for block in 0..n {
            let a = Target::new(BlockId(if seed % 2 == 0 { (block + 1) % n } else { random() % n }));
            let b = Target::new(BlockId(random() % n));
            terms.push(if seed % 2 == 0 || random() % 3 != 0 {
                TermKind::Cond { condition: ValueId(0), then_target: a, else_target: b }
            } else {
                TermKind::Ret(None)
            });
            unwinds.push((random() % 3 == 0).then(|| random() % n));
        }
        equivalent(&graph(terms, unwinds));
    }
    let function = graph(
        vec![TermKind::Ret(None), TermKind::Br(Target::new(BlockId(2))), TermKind::Br(Target::new(BlockId(1)))],
        vec![None, Some(2), Some(1)],
    );
    equivalent(&function);
    assert_eq!(Dominators::compute(&function).unwrap_err(), "block ^b1 is unreachable");
}

#[test]
fn lowered_large_function_shapes_match_the_old_cfg() {
    // Same four shapes as tests/large_functions.rs, bounded for debug builds.
    let mut functions = 0;
    for n in [4, 16, 64, 256] {
        let lets = std::iter::once("let v0 = 1;".to_string())
            .chain((1..n).map(|i| format!("let v{i} = v{} + {};", i - 1, i % 7)))
            .collect::<Vec<_>>().join("\n");
        let branches = (0..n).map(|i| format!("if x == {i} {{ return {}; }}", i % 200)).collect::<Vec<_>>().join("\n");
        let loops = (0..n / 4).map(|i| format!("let s{i} = (0..3).reduce(0) -> sum, k {{ sum + k + {}; }};", i % 5)).collect::<Vec<_>>().join("\n");
        let items = (0..n).map(|i| (i % 97).to_string()).collect::<Vec<_>>().join(", ");
        for body in [
            format!("{lets} v{} % 256;", n - 1), format!("let x = 3; {branches} 0;"),
            format!("{loops} s0 % 256;"), format!("let items = [{items}]; items.size() % 256;"),
        ] {
            let source = SourceFile::new("cfg-equivalence.ok", format!("module app v1.0.0; main() is int {{ {body} }}")).unwrap();
            let program = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
            let module = lowering::lower_program(&syntax::prune::strip_test_items(program)).unwrap();
            for function in &module.functions {
                equivalent(function);
                functions += 1;
            }
            // Includes regions, liveness, allocation and the debug register check.
            codegen::sparrow::generate(&module).unwrap();
        }
    }
    assert!(functions >= 16);
}
