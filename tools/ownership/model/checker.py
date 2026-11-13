"""Bounded, exhaustive verification of the finite protocol IR.

Both successors of every branch and both outcomes of invoke.split are checked,
even when a particular interpreter run chooses only one. Loops are explicitly
unrolled fixtures: revisiting a PC on a path is rejected, not silently accepted.
The transition system supplies local preconditions; this module checks edges,
and Machine.invariants independently checks global conservation after each step.
"""
from copy import deepcopy
from ir import Instruction, ModelError
from machine import Machine, Failure, Value, require


def split_edge(machine, instruction, success):
    packet, value, outcome, normal, unwind = instruction.args
    machine.token(packet)
    machine.value(value)
    require(outcome in ('ok', 'fail'), 'B6', 'invoke outcome must be ok/fail')
    prefix = f'@edge:{instruction.line}:'
    if success:
        rest, selected, deps = (prefix + s for s in ('rest', 'selected', 'deps'))
        machine.execute(Instruction('own.split.checked', (packet, value, rest, selected, deps), instruction.line))
        machine.edge = {'$rest': ('packet', rest), '$selected': ('packet', selected),
                        '$deps': ('value', machine.values[deps])}
        return normal
    # Failure precedes commit: no new partition or self is published.
    saved = prefix + 'input'
    machine.execute(Instruction('own.move', (packet, saved), instruction.line))
    machine.edge = {'$packet': ('packet', saved), '$failure': ('failure', Failure('typed', 'Selection'))}
    return unwind


def walk(program, exhaustive):
    pending = [(0, Machine(), frozenset())]
    completed = []
    while pending:
        pc, machine, visited = pending.pop()
        while pc < len(program.instructions):
            instruction = program.instructions[pc]
            require(pc not in visited, 'B6', 'cyclic CFG: unroll finite protocol fixtures explicitly')
            visited = visited | {pc}
            op, a = instruction.op, instruction.args
            if op == 'jump':
                machine.edge = None
                pc = program.labels[a[0]]
                continue
            if op == 'branch':
                require(len(a) == 3 and a[0] in ('true', 'false'), 'B6', 'malformed branch')
                if exhaustive:
                    pending.append((program.labels[a[2]], deepcopy(machine), visited))
                    pc = program.labels[a[1]]
                else:
                    pc = program.labels[a[1] if a[0] == 'true' else a[2]]
                machine.edge = None
                continue
            if op == 'invoke.split':
                if exhaustive:
                    other = deepcopy(machine)
                    label = split_edge(other, instruction, False)
                    pending.append((program.labels[label], other, visited))
                    label = split_edge(machine, instruction, True)
                else:
                    label = split_edge(machine, instruction, a[2] == 'ok')
                pc = program.labels[label]
                continue
            if machine.edge and op not in ('label', 'edge.bind'):
                require(not any(kind == 'packet' for kind, _ in machine.edge.values()),
                        'B6', 'linear edge results must bind in successor parameters')
                machine.edge = None
            machine.execute(instruction)
            pc += 1
        machine.invariants(final=True)
        completed.append(machine)
    return completed


def verify(program):
    return walk(program, exhaustive=True)


def run(program):
    return walk(program, exhaustive=False)[0]
