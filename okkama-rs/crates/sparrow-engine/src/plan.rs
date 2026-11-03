//! Labelled construction of verified intrinsic control-flow plans.
#[derive(Default)]
pub(crate) struct Plan {
    pub(crate) ops: Vec<crate::Op>,
    labels: std::collections::HashMap<&'static str, u32>,
    edges: Vec<(usize, bool, &'static str)>,
}
impl Plan {
    pub(crate) fn push(&mut self, op: crate::Op) {
        self.ops.push(op);
    }
    pub(crate) fn label(&mut self, label: &'static str) {
        self.labels.insert(label, self.ops.len() as u32);
    }
    pub(crate) fn jump(&mut self, label: &'static str) {
        self.edges.push((self.ops.len(), false, label));
        self.push(crate::Op::Jump(okkama_sparrow::Target {
            pc: 0,
            moves: vec![],
        }));
    }
    pub(crate) fn branch(&mut self, condition: usize, yes: &'static str, no: &'static str) {
        self.edges
            .extend([(self.ops.len(), false, yes), (self.ops.len(), true, no)]);
        self.push(crate::Op::Branch(
            condition,
            okkama_sparrow::Target {
                pc: 0,
                moves: vec![],
            },
            okkama_sparrow::Target {
                pc: 0,
                moves: vec![],
            },
        ));
    }
    pub(crate) fn finish(mut self) -> Vec<crate::Op> {
        for (pc, no, label) in self.edges {
            let target = match &mut self.ops[pc] {
                crate::Op::Jump(target) => target,
                crate::Op::Branch(_, yes, other) => {
                    if no {
                        other
                    } else {
                        yes
                    }
                }
                _ => unreachable!(),
            };
            target.pc = self.labels[label];
        }
        self.ops
    }
}
