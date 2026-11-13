"""Small line-oriented protocol IR, deliberately not the full OIR grammar."""
from dataclasses import dataclass
import shlex


# Minimum/maximum operand counts. None denotes a variadic tail, not an
# unchecked opcode. Semantic errors (e.g. missing unique-cache) have B-codes.
ARITY = {}
for names, low, high in [
    ('profile plain use drop own.empty succ succ.take builder.new builder.abandon future.start future.settle future.cancel cancel gc emit emit.label check scope.end cleanup.step cleanup.resume once worker native error.payload descriptor.plain unwind.use label', 1, 1),
    ('reset.log', 0, 0),
    ('scope.begin cleanup.begin', 1, 2),
    ('new new.async', 2, None),
    ('object array closure cap bound role any', 1, None),
    ('alias deps.view bind rebind staged own.move own.admit exit.prepare exit.commit scope.unwind catch failure.cause carrier builder.finish unique-cache await cache.rehome waiter waiter.cancel frame.save frame.resume storage storage.close storage.clear storage.remove assert.owner assert.state edge.bind', 2, 2),
    ('staged stage scope.split scope.transfer summary failure concat get dynamic.check carrier', 3, 3),
    ('failure.secondary failure.view', 2, 2),
    ('slice replace builder.push future', 4, None),
    ('own.split own.split.checked', 5, 5),
    ('own.join', 2, None),
    ('own.dynamic', 0, None),
    ('call.enter', 2, None),
    ('call.exit', 2, 3),
    ('exit.abort storage.close storage.clear', 1, 1),
    ('defer defer.block on.destroy', 2, None),
    ('assert.failure assert.items', 1, None),
    ('destroy.direct completion.next', 2, 2),
    ('end', 1, 2),
    ('await.take', 2, 3),
    ('collect.all completions', 1, None),
    ('storage.set storage.get', 3, 3),
    ('jump', 1, 1),
    ('branch', 3, 3),
    ('invoke.split', 5, 5),
]:
    for name in names.split():
        ARITY[name] = (low, high)
ARITY['gc'] = (0, None)
ARITY['slice'] = ARITY['replace'] = ARITY['builder.push'] = (4, 4)

# Draft 3 protocol transitions.
ARITY.update({
    'carrier.attach': (2, 3), 'carrier.transfer': (2, 2),
    'carrier.rehome': (2, 2), 'carrier.release': (2, 2),
    'dynamic.refresh': (2, 2), 'dynamic.check': (3, 4), 'snapshot.check': (1, 1),
    'observe': (2, 2), 'service.schedule': (1, None), 'service.settle': (1, 1),
    'assert.provenance': (2, 2), 'assert.cleanup-origin': (1, 1),
    'future.body': (2, None), 'future.request-cancel': (1, 1),
})

ARITY.update({'destroy.log': (3, 3), 'check.value': (2, 2)})

ARITY['fresh.transfer'] = (2, 2)

ARITY.update({'group': (1, 1), 'group.start': (2, 2), 'group.wait': (1, 1), 'group.cancel': (1, 1), 'race.winner': (2, None)})

def validate(instruction):
    if instruction.op not in ARITY:
        raise ModelError('IR', f'unknown instruction {instruction.op}', instruction.line)
    low, high = ARITY[instruction.op]
    size = len(instruction.args)
    if size < low or (high is not None and size > high):
        raise ModelError('IR', f'wrong operand count for {instruction.op}', instruction.line)


class ModelError(Exception):
    def __init__(self, rule, message, line=0):
        self.rule, self.message, self.line = rule, message, line
        super().__init__(f'{rule}' + (f' line {line}' if line else '') + f': {message}')


@dataclass(frozen=True)
class Instruction:
    op: str
    args: tuple
    line: int = 0

    def text(self):
        return shlex.join((self.op, *self.args))


@dataclass
class Program:
    instructions: list
    labels: dict

    def text(self):
        return '\n'.join(i.text() for i in self.instructions) + '\n'


def parse(text):
    instructions, labels = [], {}
    for number, line in enumerate(text.splitlines(), 1):
        try:
            words = shlex.split(line, comments=True)
        except ValueError as error:
            raise ModelError('IR', str(error), number) from error
        if not words:
            continue
        instruction = Instruction(words[0], tuple(words[1:]), number)
        validate(instruction)
        if instruction.op == 'label':
            if len(instruction.args) != 1 or instruction.args[0] in labels:
                raise ModelError('IR', 'duplicate or malformed label', number)
            labels[instruction.args[0]] = len(instructions)
        instructions.append(instruction)
    if not instructions or instructions[0].op != 'profile':
        raise ModelError('B11', 'explicit profile required')
    for i in instructions:
        targets = i.args[1:] if i.op == 'branch' else i.args if i.op == 'jump' else ()
        if i.op == 'invoke.split':
            targets = i.args[-2:]
        for target in targets:
            if target not in labels:
                raise ModelError('B6', f'unknown successor {target}', i.line)
    return Program(instructions, labels)
