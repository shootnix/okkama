"""Executable ownership transition system. No reference counting, no GC library.

Every identity is held by exactly one bag. Bags belong to lexical scopes,
explicit packets, pending exits, task inputs/frames/caches, or protocol storage.
Values and their stable dependencies are deliberately separate from those bags.
"""
from dataclasses import dataclass, field
from collections import Counter
import shlex

from ir import Instruction, ModelError, validate


def require(condition, rule, message):
    if not condition:
        raise ModelError(rule, message)


@dataclass
class Value:
    ids: frozenset = frozenset()
    deps: frozenset = frozenset()
    fields: dict = field(default_factory=dict)
    items: tuple = ()
    futures: frozenset = frozenset()
    views: frozenset = frozenset()
    kind: str = 'plain'
    scalar: object = None


@dataclass
class Identity:
    serial: int
    label: str
    holder: str
    epoch: int = 0
    state: str = 'live'
    provenance: str = 'unbound-result'
    destroy: tuple = ()
    async_destroy: bool = False
    value: object = None
    log_begin: object = None
    log_end: object = None


@dataclass
class Failure:
    kind: str
    name: str
    secondaries: list = field(default_factory=list)
    cause: object = None
    roots: frozenset = frozenset()
    cleaned: bool = False
    cleanup_origin: bool = False
    holder: object = None
    cache: object = None


@dataclass
class Scope:
    parent: object
    phase: str = 'open'
    defers: list = field(default_factory=list)
    staged: dict = field(default_factory=dict)
    pending: object = None
    failure: object = None
    call: object = None
    async_ok: bool = False
    cancelled: bool = False
    shield: bool = False
    current: object = None
    suspended: bool = False
    schedule: object = None
    services_done: bool = False
    destroy_started: bool = False


@dataclass
class Future:
    owner: str
    args: tuple
    mode: str
    result: str
    temporary: bool = False
    state: str = 'cold'
    outcome: object = None
    failure: object = None
    waiters: set = field(default_factory=set)
    starts: int = 0
    body: tuple = ()
    continuation: tuple = ()
    task_scope: object = None
    cancel_requested: bool = False


class Machine:
    def __init__(self):
        self.values = {}
        self.identities = {}
        self.bags = {}
        self.bag_scope = {}
        self.holder_keys = {}
        self.packets = set()
        self.consumed = set()
        self.scopes = {}
        self.stack = []
        self.bindings = {}
        self.must_use = {}
        self.builders = {}
        self.futures = {}
        self.unique = {}
        self.stores = {}
        self.groups = {}
        self.stream_ready = {}
        self.summaries = {}
        self.failures = {}
        self.frames = {}
        self.events = []
        self.destroy_trace = []
        self.log = ''
        self.checkpoints = []
        self.metrics = Counter()
        self.profile = None
        self.special_self = None
        self.edge = None
        self.block = None
        self.block_definitions = set()
        self.last_failure = None
        self.snapshots = {}
        self.service_schedule = []

    @property
    def current(self):
        require(bool(self.stack), 'B4', 'operation needs an open scope')
        return self.stack[-1]

    def event(self, event_name, **data):
        self.events.append({'event': event_name, **data})

    def bag(self, name, owner):
        require(name not in self.bags and name not in self.consumed, 'B2', f'duplicate token {name}')
        self.bags[name] = set()
        self.bag_scope[name] = owner
        self.holder_keys[name] = name
        return name

    def move_ids(self, ids, destination, protocol=False):
        require(destination in self.bags, 'B2', f'unknown destination {destination}')
        for ident in sorted(ids):
            obj = self.identities[ident]
            require(obj.state == 'live', 'B10', 'adoption/reentry of non-live identity')
            old = obj.holder
            if old == destination:
                continue
            require(protocol or not old.startswith(('cache:', 'inputs:', 'frame:', 'saved:')),
                    'B9', 'task/cache packet requires its protocol transition')
            require(protocol or not old.startswith(('store:', 'carrier:')),
                    'B7', 'storage/carrier packet requires its protocol transition')
            self.bags[old].remove(ident)
            self.bags[destination].add(ident)
            obj.holder = destination
            obj.epoch += 1
            self.metrics['transfers'] += 1
            self.event('move', id=ident, serial=obj.serial, source=old, holder=destination, epoch=obj.epoch)

    def rehome_bag(self, holder, owner):
        require(owner in self.scopes and self.scopes[owner].phase == 'open', 'B7', 'closed rehome target')
        if self.bag_scope[holder] != owner:
            self.bag_scope[holder] = owner
            for ident in sorted(self.bags[holder]):
                self.identities[ident].epoch += 1
                self.event('rehome', id=ident, holder=holder, owner=owner,
                           epoch=self.identities[ident].epoch)

    def retention(self, value):
        return set(value.ids)

    def provenance(self, ids, state):
        for ident in ids:
            self.identities[ident].provenance = state

    def token(self, name):
        require(name in self.packets and name in self.bags, 'B2', f'unknown/consumed packet {name}')
        return set(self.bags[name])

    def consume(self, name):
        self.token(name)
        require(not self.bags[name], 'B2', 'consuming packet would lose obligations')
        del self.bags[name]
        del self.bag_scope[name]
        self.packets.remove(name)
        self.consumed.add(name)

    def packet(self, name, ids=(), owner=None):
        self.bag(name, owner if owner is not None else self.current)
        self.packets.add(name)
        self.move_ids(ids, name)

    def put(self, name, value):
        require(name not in self.values, 'B1', f'value {name} is already defined (SSA)')
        self.values[name] = value
        self.block_definitions.add(name)

    def value(self, name, live=True):
        if name.startswith('&'):
            require(name[1:] in self.bindings, 'B1', f'unknown binding {name}')
            name = self.bindings[name[1:]][1]
        require(name in self.values, 'B1', f'{name} is not a value')
        value = self.values[name]
        require(value.ids <= value.deps, 'B1', 'managed path lacks stable deps')
        if live:
            for ident in value.deps:
                obj = self.identities[ident]
                require(obj.state == 'live' or (obj.state == 'destroying' and ident == self.special_self),
                        'B3', f'dead dependency {ident}')
                owner = self.bag_scope.get(obj.holder)
                require(owner not in self.frames, 'B9', 'access while ownership is saved in frame')
            for future in value.futures | value.views:
                require(self.futures[future].state != 'consumed', 'B9', 'access to consumed cache')
                require(self.scopes[self.futures[future].owner].phase != 'closed', 'B3', 'closed cache owner')
        return value

    def aggregate(self, names, kind='array', keys=None):
        vals = [self.value(n) for n in names]
        self.metrics['visited_nodes'] += len(vals) + 1
        self.metrics['visited_edges'] += len(vals)
        return Value(frozenset().union(*(v.ids for v in vals)),
                     frozenset().union(*(v.deps for v in vals)),
                     dict(zip(keys, names)) if keys else {}, tuple(names),
                     frozenset().union(*(v.futures for v in vals)),
                     frozenset().union(*(v.views for v in vals)), kind)

    def begin(self, name, async_ok=None, call=None):
        require(name not in self.scopes, 'B4', f'duplicate scope {name}')
        parent = self.stack[-1] if self.stack else None
        if async_ok is None:
            async_ok = bool(parent and self.scopes[parent].async_ok)
        self.scopes[name] = Scope(parent, async_ok=async_ok, call=call)
        self.bag('scope:' + name, name)
        self.stack.append(name)
        self.event('scope.begin', scope=name)

    def create(self, name, label, fields=(), async_destroy=False):
        s = self.current
        require(self.scopes[s].phase in ('open', 'deferring'), 'B4', 'creation in closing scope')
        require(s not in self.frames, 'B9', 'creation while execution is saved in frame')
        require(not async_destroy or self.scopes[s].async_ok, 'B11', 'async-owned destroy in sync context')
        base = self.aggregate([f.split('=', 1)[1] for f in fields], 'object',
                              [f.split('=', 1)[0] for f in fields])
        ident = len(self.identities) + 1
        obj = Identity(ident, label, 'scope:' + s, async_destroy=async_destroy)
        self.identities[ident] = obj
        self.bags[obj.holder].add(ident)
        self.put(name, Value(base.ids | {ident}, base.deps | {ident}, base.fields, base.items,
                             base.futures, base.views, 'object'))
        obj.value = self.values[name]
        self.event('create', id=ident, serial=ident, holder=obj.holder, label=label)

    def owned(self, scope, value):
        return {i for i in self.retention(value) if self.bag_scope[self.identities[i].holder] == scope}

    def transferable(self, ids):
        for ident in ids:
            holder = self.identities[ident].holder
            require(not holder.startswith(('cache:', 'inputs:', 'frame:', 'saved:')),
                    'B9', 'task/cache packet requires its protocol transition')
            require(not holder.startswith(('store:', 'carrier:', 'pending:')),
                    'B7', 'holder requires its protocol transition')

    def fresh(self, value):
        return {i for i in self.retention(value) if self.identities[i].provenance in
                ('unbound-result', 'temporary-await-result')}

    def ancestor(self, ancestor, child):
        while child is not None:
            if ancestor == child:
                return True
            child = self.scopes[child].parent
        return False

    def escape_deps(self, value, moving, target):
        for ident in value.deps - moving:
            holder = self.bag_scope[self.identities[ident].holder]
            require(self.ancestor(holder, target), 'B3', f'escape would outlive dependency {ident}')
        for f in value.futures | value.views:
            require(self.ancestor(self.futures[f].owner, target), 'B9', 'cache.rehome required before escape')

    def prepare(self, scope, name):
        s = self.scopes[scope]
        require(s.phase in ('open', 'deferring'), 'B10', 'pending result prepared after cleanup began')
        if s.failure:
            return  # return from defer cannot replace an active failure
        value = self.value(name)
        selected = self.owned(scope, value)
        if s.parent is not None:
            self.escape_deps(value, selected, s.parent)
        if s.call:
            summary, input_ids = s.call
            require(not (selected & input_ids) or summary == 'take', 'B5', 'borrow summary retains fresh input')
        bag = 'pending:' + scope
        if s.pending:
            self.move_ids(self.bags[bag] - selected, 'scope:' + scope)
        else:
            self.bag(bag, scope)
        self.move_ids(selected, bag)
        s.pending = name
        self.event('exit.prepare', scope=scope, value=name, packet=sorted(self.bags[bag]))

    def abort(self, scope):
        s = self.scopes[scope]
        if s.pending:
            self.move_ids(set(self.bags['pending:' + scope]), 'scope:' + scope)
            s.pending = None
            self.event('exit.abort', scope=scope, boundary=('abort' if s.destroy_started else 'original-plan'))

    def failure(self, scope, failure, cleanup=False):
        s = self.scopes[scope]
        failure.cleanup_origin |= cleanup
        if not s.failure:
            s.failure = failure
            self.abort(scope)
        elif failure is not s.failure:
            self.link_failure(s.failure, failure)
        self.event('failure', scope=scope, kind=failure.kind, name=failure.name, cleanup=cleanup)

    def roots(self):
        # Resource roots, not a simulator of Python's collector. Bags are owning
        # roots, including retired entries that no source-level value reaches.
        result = set().union(*self.bags.values()) if self.bags else set()
        for f in self.failures.values():
            result |= self.failure_roots(f)
        return result

    def failure_roots(self, failure):
        return set().union(*(node.roots for node in self.failure_graph(failure)))

    def failure_graph(self, failure, seen=None):
        seen = set() if seen is None else seen
        if id(failure) in seen:
            return []
        seen.add(id(failure))
        nodes = [failure]
        for child in ([failure.cause] if failure.cause else []) + failure.secondaries:
            nodes += self.failure_graph(child, seen)
        return nodes

    def carrier_move(self, failure, target, release=False, rehome=False):
        nodes = self.failure_graph(failure)
        require(not failure.cache, 'B9', 'failure-cache requires cache protocol')
        require(target in self.scopes and self.scopes[target].phase == 'open', 'B7', 'closed carrier target')
        roots = set().union(*(n.roots for n in nodes if not n.cache))
        for node in nodes:
            if node.cache:
                require(self.ancestor(self.futures[node.cache].owner, target),
                        'B9', 'cache.rehome required for borrowed failure graph')
        for ident in roots:
            require(self.identities[ident].state == 'live', 'B3', 'dead carrier dependency')
            owner = self.bag_scope[self.identities[ident].holder]
            require(self.ancestor(target, owner), 'B3', 'carrier target must enclose owner')
        holders = {n.holder for n in nodes if n.holder and not n.cache}
        require(not roots or all(self.identities[i].holder.startswith('carrier:') for i in roots),
                'B7', 'carrier no longer holds packet')
        if rehome:
            for holder in holders:
                self.rehome_bag(holder, target)
        else:
            dest = 'scope:' + target if release else 'carrier:' + target + ':' + str(len(self.events))
            if dest not in self.bags:
                self.bag(dest, target)
            self.move_ids(roots, dest, protocol=True)
            for n in nodes:
                if not n.cache:
                    n.holder = None if release else dest
        self.event('carrier.' + ('rehome' if rehome else 'release' if release else 'transfer'),
                   owner=target, roots=sorted(roots))

    def link_failure(self, failure, child, cause=False):
        require(not any(n is failure for n in self.failure_graph(child)), 'B7', 'cyclic failure graph')
        nodes = self.failure_graph(failure) + self.failure_graph(child)
        owners = {self.bag_scope[self.identities[i].holder]
                  for n in nodes if not n.cache for i in n.roots}
        require(len(owners) <= 1, 'B7', 'carrier graph requires common owner before linking')
        owner = next(iter(owners), self.current)
        for node in nodes:
            if node.cache:
                require(self.ancestor(self.futures[node.cache].owner, owner),
                        'B9', 'cache.rehome required for borrowed failure graph')
        if cause:
            failure.cause = child
        else:
            failure.secondaries.append(child)

    def start_close(self, scope, failure=None):
        require(self.current == scope, 'B4', 'scope stack is not nested')
        s = self.scopes[scope]
        require(s.phase == 'open', 'B4', 'scope closed more than once')
        require(scope not in self.frames, 'B9', 'cannot close suspended frame')
        s.phase, s.shield = 'deferring', True
        if failure:
            for node in self.failure_graph(failure):
                if node.holder and node.holder.startswith('carrier:'):
                    require(self.bag_scope[node.holder] != scope, 'B7',
                            'escaping carrier must transfer before owner cleanup')
            self.failure(scope, failure)
        for body in reversed(s.defers):
            for command in body:
                self.run_inline(command, scope)
                if shlex.split(command)[0] == 'fail':
                    break
        for name, owner in self.must_use.items():
            require(owner != scope or s.failure is not None, 'B8', f'unused successor {name}')
        for name, (owner, _, _) in list(self.builders.items()):
            if owner == scope:
                require(s.failure is not None, 'B8', 'builder requires finish or abandon')
                del self.builders[name]
        s.phase = 'service-settle'
        self.event('service.begin', scope=scope)

    def settle_services(self, scope):
        s = self.scopes[scope]
        names = [n for n, f in self.futures.items()
                 if f.owner == scope and f.state != 'consumed']
        order = s.schedule if s.schedule is not None else names
        require(len(order) == len(set(order)) and set(order) == set(names),
                'B10', 'service schedule must name each owned Future once')
        for name in order:
            future = self.futures[name]
            require(not future.waiters, 'B9', 'owner closes with outstanding waiters')
            self.service_schedule.append((scope, name))
            self.event('service.choose', scope=scope, service=name)
            if future.state == 'running':
                self.settle(name, cancel=True)
                if future.failure:
                    failures = (future.failure.secondaries if future.failure.kind == 'cancel'
                                else [future.failure])
                    for failure in failures:
                        self.failure(scope, failure, cleanup=failure.cleanup_origin)
            # Closing a cold Future never runs its body.
            for kind in ('inputs', 'frame', 'cache'):
                self.move_ids(set(self.bags[kind + ':' + name]), 'scope:' + scope, protocol=True)
            self.event('service.released', scope=scope, service=name)
        for name, store in self.stores.items():
            if store['owner'] == scope:
                store['closed'] = True
                self.move_ids(set(self.bags['store:' + name]), 'scope:' + scope, protocol=True)
                self.event('service.released', scope=scope, service='store:' + name)
        for holder in list(self.bags):
            if holder.startswith('carrier:') and self.bag_scope[holder] == scope:
                self.move_ids(set(self.bags[holder]), 'scope:' + scope, protocol=True)
                self.event('service.released', scope=scope, service=holder)
        for p in self.packets:
            require(self.bag_scope[p] != scope, 'B2', f'unconsumed packet {p}, including empty residual')
        s.phase, s.services_done = 'closing', True
        self.event('service.end', scope=scope)
        self.event('cleanup.begin', scope=scope, roots=sorted(self.roots()), shield=True)

    def remaining(self, scope):
        pending = 'pending:' + scope
        return {i for bag, ids in self.bags.items() if self.bag_scope[bag] == scope and bag != pending
                for i in ids if self.identities[i].state == 'live'}

    def close_step(self, scope):
        s = self.scopes[scope]
        if s.phase == 'service-settle':
            self.settle_services(scope)
        require(s.phase == 'closing' and not s.suspended, 'B10', 'invalid cleanup step')
        remaining = self.remaining(scope)
        if not remaining:
            if s.cancelled and not s.failure:
                self.failure(scope, Failure('cancel', 'Cancelled'))
                if self.remaining(scope):
                    return
            s.phase, s.shield = 'closed', False
            require(self.current == scope, 'B4', 'cleanup stack mismatch')
            self.stack.pop()
            if s.failure:
                s.failure.cleaned = True
            self.last_failure = s.failure
            self.event('scope.closed', scope=scope)
            return
        ident = max(remaining, key=lambda i: self.identities[i].serial)
        obj = self.identities[ident]
        s.destroy_started = True
        obj.state, s.current = 'destroying', ident
        self.event('destroy.begin', id=ident, serial=obj.serial, holder=obj.holder, roots=sorted(self.roots()))
        if obj.log_begin is not None:
            self.log += obj.log_begin
        if obj.async_destroy:
            s.suspended = True
            self.event('cleanup.suspend', scope=scope, id=ident, roots=sorted(self.roots()))
        else:
            self.finish_destroy(scope)

    def finish_destroy(self, scope):
        s = self.scopes[scope]
        ident = s.current
        require(ident is not None, 'B10', 'resume without destroying identity')
        obj = self.identities[ident]
        require(obj.state == 'destroying', 'B10', 'destroy reentry')
        if s.suspended:
            self.event('cleanup.resume', scope=scope, id=ident)
        self.log += obj.label if obj.log_end is None else obj.log_end
        self.destroy_trace.append(obj.label)
        self.event('destroy', id=ident, serial=obj.serial, holder=obj.holder, label=obj.label)
        old_self = self.special_self
        old_self_value = self.values.get('self')
        saved_values, saved_bindings = set(self.values), set(self.bindings)
        self.special_self = ident
        nested = f'@destroy{ident}'
        self.begin(nested, s.async_ok)
        self.values['self'] = obj.value
        for body in obj.destroy:
            self.run_inline(body, nested)
            if shlex.split(body)[0] == 'fail':
                break
        self.close(nested)
        if self.last_failure:
            self.failure(scope, self.last_failure, cleanup=True)
        self.special_self = old_self
        for name in set(self.values) - saved_values:
            del self.values[name]
        for name in set(self.bindings) - saved_bindings:
            del self.bindings[name]
        self.values.pop('self', None)
        if old_self_value:
            self.values['self'] = old_self_value
        self.bags[obj.holder].remove(ident)
        obj.state = 'dead'
        s.current, s.suspended = None, False

    def close(self, scope, failure=None):
        self.start_close(scope, failure)
        while self.scopes[scope].phase != 'closed':
            self.close_step(scope)
            if self.scopes[scope].suspended:
                self.finish_destroy(scope)

    def run_inline(self, text, scope, cleanup=True):
        words = shlex.split(text)
        if words[0] == 'fail':
            failure = Failure(words[1], words[2])
            while self.current != scope:
                self.close(self.current, failure)
            self.failure(scope, failure, cleanup=cleanup)
        else:
            self.execute(Instruction(words[0], tuple(words[1:])))
        self.invariants()

    def commit(self, scope, target):
        s = self.scopes[scope]
        require(s.phase == 'closed' and not s.failure and s.pending, 'B10', 'commit before successful cleanup')
        require(self.scopes[target].phase in ('open', 'deferring'), 'B4', 'closed result destination')
        value = self.value(s.pending)
        ids = set(self.bags['pending:' + scope])
        self.escape_deps(value, ids, target)
        self.move_ids(ids, 'scope:' + target)
        for ident in ids:
            self.identities[ident].provenance = 'unbound-result'
        s.pending = None
        self.event('exit.commit', scope=scope, target=target)

    def settle(self, name, cancel=False):
        f = self.futures[name]
        require(f.state in ('cold', 'running', 'settled'), 'B9', 'settle consumed cache')
        self.event('future.settle-request', future=name, cancel=bool(cancel or f.cancel_requested), state=f.state)
        if f.state == 'settled':
            return
        cancel = cancel or f.cancel_requested
        if f.state == 'cold' and cancel:
            f.state, f.failure = 'settled', Failure('cancel', 'Cancelled')
            self.future_settled(name)
            return  # cold input journal keeps its original lexical boundary
        if f.state == 'cold':
            self.start_future(name)
        for value in f.args:
            self.value(value)
        frame = f.task_scope
        self.stack.append(frame)
        if not cancel:
            for command in f.continuation:
                self.run_inline(command, frame, cleanup=False)
                if shlex.split(command)[0] == 'fail':
                    break
        self.event('task.resume', future=name)
        if cancel:
            self.close(frame, Failure('cancel', 'Cancelled'))
            f.failure = self.last_failure
        elif self.scopes[frame].failure:
            self.close(frame)
            f.failure = self.last_failure
        elif f.mode == 'fail':
            self.close(frame, Failure('typed', f.result))
            f.failure = self.last_failure
        else:
            if f.mode == 'new':
                result = '@result:' + name
                self.create(result, f.result)
            elif f.mode == 'borrow':
                result = f.result
            else:
                result = '@void:' + name
                self.put(result, Value(scalar=f.result if f.mode == 'literal' else None))
            self.prepare(frame, result)
            self.close(frame)
            if self.last_failure:
                f.failure = self.last_failure
            else:
                ids = set(self.bags['pending:' + frame])
                self.escape_deps(self.value(result), ids, f.owner)
                self.move_ids(ids, 'cache:' + name)
                self.scopes[frame].pending = None
                self.provenance(ids, 'cache-owned')
                f.outcome = result
        f.state = 'settled'
        self.future_settled(name)

    def future_settled(self, name):
        f = self.futures[name]
        self.event('future.settled', future=name, failure=f.failure.name if f.failure else None)
        for stream, ready in self.stream_ready.items():
            ready.extend(n for n in self.values[stream].items if n == name)

    def start_future(self, name):
        f = self.futures[name]
        require(f.state == 'cold', 'B9', 'future already started')
        for arg in f.args:
            self.value(arg)
        self.move_ids(set(self.bags['inputs:' + name]), 'frame:' + name, protocol=True)
        frame = '@task:' + name
        self.begin(frame, async_ok=True)
        self.scopes[frame].parent = f.owner
        f.task_scope = frame
        self.move_ids(set(self.bags['frame:' + name]), 'scope:' + frame, protocol=True)
        commands = list(f.body)
        while commands:
            command = commands.pop(0)
            if command == 'suspend':
                break
            self.run_inline(command, frame, cleanup=False)
            if shlex.split(command)[0] == 'fail':
                break
        f.continuation = tuple(commands)
        require(self.current == frame, 'B9', 'task body leaves nested scope open')
        self.stack.pop()
        self.event('task.suspend', future=name, scope=frame)
        f.state, f.starts = 'running', f.starts + 1
        self.event('future.start', future=name)

    def invariants(self, final=False):
        occurrences = Counter(i for ids in self.bags.values() for i in ids)
        for ident, obj in self.identities.items():
            require(occurrences[ident] == (0 if obj.state == 'dead' else 1), 'B2', f'nonlinear obligation {ident}')
            if obj.state != 'dead':
                require(ident in self.bags.get(obj.holder, ()), 'B2', 'holder metadata disagrees with packet')
                owner = self.bag_scope[obj.holder]
                require(self.scopes[owner].phase != 'closed' or obj.holder == 'pending:' + owner,
                        'B4', 'closed owner retains live obligation')
        for f in self.futures.values():
            require(f.starts <= 1, 'B9', 'duplicate future start')
        per_owner = Counter()
        for bag, ids in self.bags.items():
            per_owner[self.bag_scope[bag]] += len(ids)
        self.metrics['max_journal'] = max(self.metrics['max_journal'], *per_owner.values(), 0)
        if final:
            require(not self.stack and all(s.phase == 'closed' for s in self.scopes.values()),
                    'B4', 'unclosed scope')
            require(not self.packets, 'B2', 'unconsumed linear packet')
            require(not self.unique, 'B9', 'unused unique-cache capability')
            require(not any(s.pending for s in self.scopes.values()), 'B10', 'uncommitted pending exit')
            require(all(o.state == 'dead' for o in self.identities.values()), 'B2', 'leaked obligation')

    def execute(self, instruction):
        validate(instruction)
        op, a = instruction.op, list(instruction.args)
        try:
            require(not self.stack or self.current not in self.frames or op in ('frame.resume', 'gc', 'assert.state'),
                    'B9', 'execution is saved in frame')
            self._execute(op, a)
            self.invariants()
        except ModelError as error:
            if not error.line:
                error.line = instruction.line
            raise ModelError(error.rule, error.message, error.line) from error
        except (IndexError, KeyError, ValueError) as error:
            raise ModelError('IR', f'malformed {op}: {error}', instruction.line) from error

    def _execute(self, op, a):
        if op == 'profile':
            require(self.profile is None and a[0] in ('own', 'coro', 'low'), 'B11', 'supported profiles: own/coro/low')
            self.profile = a[0]
        elif op == 'scope.begin':
            self.begin(a[0], True if len(a) > 1 and a[1] == 'async' else None)
        elif op in ('new', 'new.async'):
            self.create(a[0], a[1], a[2:], op == 'new.async')
        elif op == 'plain':
            self.put(a[0], Value())
        elif op == 'deps.view':
            v = self.value(a[1])
            self.put(a[0], Value(v.deps, v.deps, futures=v.futures, views=v.views, kind='view'))
        elif op in ('array', 'closure', 'cap', 'bound', 'role', 'any'):
            self.put(a[0], self.aggregate(a[1:], op))
        elif op == 'object':
            self.put(a[0], self.aggregate([p.split('=', 1)[1] for p in a[1:]], 'object',
                                          [p.split('=', 1)[0] for p in a[1:]]))
        elif op in ('alias', 'use'):
            value = self.value(a[-1])
            if op == 'alias':
                self.put(a[0], value)
            else:
                self.must_use.pop(a[0], None)
                self.event('use', value=a[0], ids=sorted(value.deps))
        elif op == 'drop':
            self.value(a[0], live=False)
            # Discarded expression demand, not a linear consume or an alias-
            # liveness proof. Containers may still retain this immutable value.
            self.event('discard', value=a[0])
        elif op in ('get', 'slice'):
            value = self.value(a[1])
            if op == 'get':
                name = value.fields[a[2]] if value.fields else value.items[int(a[2])]
                self.put(a[0], self.value(name))
            else:
                self.put(a[0], self.aggregate(value.items[int(a[2]):int(a[3])]))
        elif op == 'concat':
            self.put(a[0], self.aggregate(self.value(a[1]).items + self.value(a[2]).items))
        elif op == 'replace':
            items = list(self.value(a[1]).items)
            items[int(a[2])] = a[3]
            self.put(a[0], self.aggregate(items))
        elif op in ('bind', 'rebind'):
            if op == 'bind':
                require(a[0] not in self.bindings, 'B1', 'duplicate binding')
                owner = self.current
            else:
                owner = self.bindings[a[0]][0]
            value = self.value(a[1])
            for ident in self.fresh(value):
                require(self.ancestor(owner, self.bag_scope[self.identities[ident].holder]), 'B3', 'invalid rebind owner')
            ids = self.fresh(value)
            self.move_ids(ids, 'scope:' + owner)
            for ident in ids:
                self.identities[ident].provenance = 'bound'
            self.bindings[a[0]] = (owner, a[1])
        elif op == 'stage':
            scope, slot, name = a
            require(self.ancestor(scope, self.current), 'B4', 'staging target must enclose source')
            value = self.value(name)
            ids = self.owned(self.current, value)
            self.move_ids(ids, 'scope:' + scope)
            self.provenance(ids, 'staged/carrier-owned')
            self.scopes[scope].staged[slot] = name
        elif op == 'staged':
            self.put(a[0], self.value(self.scopes[a[1]].staged[a[2]]))
        elif op == 'own.empty':
            self.packet(a[0])
        elif op == 'scope.split':
            scope, name, packet = a
            require(self.ancestor(scope, self.current), 'B9', 'scope holder is not accessible')
            ids = self.owned(scope, self.value(name))
            self.transferable(ids)
            self.packet(packet, ids, scope)
        elif op == 'own.move':
            ids = self.token(a[0])
            old, new = a
            self.bag(new, self.bag_scope[old])
            self.holder_keys[new] = self.holder_keys[old]
            self.packets.add(new)
            self.bags[new].update(ids)
            self.bags[old].clear()
            for ident in ids:
                self.identities[ident].holder = new
            self.consume(old)
        elif op in ('own.split', 'own.split.checked'):
            packet, name, rest, selected, deps = a[:5]
            ids, value = self.token(packet), self.value(name)
            self.packet(rest, ids - value.ids, self.bag_scope[packet])
            self.packet(selected, ids & value.ids, self.bag_scope[packet])
            self.put(deps, Value(deps=frozenset(value.deps - (ids & self.retention(value))),
                                 futures=value.futures, views=value.views, kind='external-deps'))
            self.consume(packet)
        elif op == 'own.join':
            output, *inputs = a
            require(len(set(inputs)) == len(inputs), 'B2', 'same token supplied twice')
            ids = set()
            for packet in inputs:
                part = self.token(packet)
                require(not ids & part, 'B2', 'overlapping join')
                ids |= part
            self.packet(output, ids)
            for packet in inputs:
                self.consume(packet)
        elif op == 'fresh.transfer':
            packet, target = a
            ids = self.token(packet)
            require(all(self.identities[i].provenance in ('unbound-result', 'temporary-await-result')
                        for i in ids), 'B5', 'packet lacks fresh call admission')
            require(self.scopes[target].phase == 'open', 'B4', 'closed admission target')
            self.move_ids(ids, 'scope:' + target)
            self.provenance(ids, 'bound')
            self.consume(packet)
        elif op == 'own.admit':
            packet, scope = a
            require(self.scopes[scope].phase in ('open', 'deferring'), 'B4', 'admit to closed scope')
            self.move_ids(self.token(packet), 'scope:' + scope)
            self.consume(packet)
        elif op == 'scope.transfer':
            source, target, name = a
            require(self.ancestor(source, self.current), 'B9', 'scope holder is not accessible')
            value = self.value(name)
            ids = self.owned(source, value)
            self.transferable(ids)
            require(self.scopes[target].phase == 'open', 'B4', 'transfer to closed scope')
            self.escape_deps(value, ids, target)
            self.move_ids(ids, 'scope:' + target)
        elif op == 'own.dynamic':
            require(len(a) == 5, 'B7', 'dynamic needs packet, target, reason, protocol, owner')
            packet, target, reason, protocol, owner = a
            contracts = {'any': 'opaque', 'merge': 'checked', 'closure': 'closure-entry', 'group': 'cache',
                         'atomic': 'borrow', 'cell': 'borrow', 'channel': 'message'}
            require(contracts.get(reason) == protocol and target in self.scopes, 'B7', 'unknown dynamic contract')
            require(self.bag_scope.get(packet) == owner, 'B7', 'dynamic owner ID mismatch')
            require(reason not in ('atomic', 'cell') or not self.token(packet), 'B7', 'borrow storage cannot take a packet')
            require(self.scopes[target].phase == 'open', 'B7', 'closed dynamic target')
            dest = 'dyn:' + target
            if dest not in self.bags:
                self.bag(dest, target)
            self.move_ids(self.token(packet), dest)
            self.consume(packet)
            self.metrics['dynamic:' + reason] += 1
        elif op == 'dynamic.check':
            name, holder, epoch = a if len(a) == 3 else (a[0], a[2], a[3])
            for ident in self.value(name, live=False).deps:
                obj = self.identities[ident]
                require(obj.state == 'live' and self.holder_keys[obj.holder] == self.holder_keys.get(holder, holder) and obj.epoch == int(epoch)
                        and (len(a) == 3 or self.bag_scope[obj.holder] == a[1]), 'B7', 'stale dynamic owner/epoch')
        elif op == 'dynamic.refresh':
            snapshot, name = a
            value = self.value(name)
            for ident in value.deps:
                owner = self.bag_scope[self.identities[ident].holder]
                require(self.ancestor(owner, self.current), 'B7', 'refresh lacks lifetime/access rights')
            self.snapshots[snapshot] = {i: (self.bag_scope[self.identities[i].holder],
                                           self.holder_keys[self.identities[i].holder], self.identities[i].epoch)
                                        for i in value.deps}
        elif op == 'snapshot.check':
            require(a[0] in self.snapshots, 'B7', 'unknown snapshot')
            for ident, expected in self.snapshots[a[0]].items():
                obj = self.identities[ident]
                require(obj.state == 'live' and expected == (self.bag_scope[obj.holder], self.holder_keys[obj.holder], obj.epoch),
                        'B7', 'stale dynamic owner/epoch')
        elif op == 'observe':
            opcode, name = a
            require(opcode in ('array.len', 'map.len', 'str.len', 'val.eq', 'obj.get',
                               'array.get', 'map.get', 'array.lookup', 'map.lookup'),
                    'B5', 'opcode lacks observation/admission-stays-caller summary')
            self.value(name)
            self.event('observation', opcode=opcode, value=name)
        elif op == 'service.schedule':
            scope, *names = a
            require(self.scopes[scope].phase == 'open', 'B10', 'late service schedule')
            self.scopes[scope].schedule = names
        elif op == 'service.settle':
            require(self.scopes[a[0]].phase == 'service-settle', 'B10', 'service settle out of phase')
            self.settle_services(a[0])
        elif op == 'assert.provenance':
            value, state = a
            require(all(self.identities[i].provenance == state for i in self.value(value).ids),
                    'B5', 'unexpected admission provenance')
        elif op == 'assert.cleanup-origin':
            require(self.failures[a[0]].cleanup_origin, 'B10', 'failure lacks cleanup-origin')
        elif op == 'summary':
            name, mode, abi = a
            require(mode in ('borrow', 'take') and abi in ('direct', 'role', 'closure', 'intrinsic'), 'B5', 'invalid summary')
            self.summaries[name] = (mode, abi)
        elif op == 'call.enter':
            scope, summary, *args = a
            require(summary in self.summaries, 'B5', 'call without checked summary/adapter')
            mode, _ = self.summaries[summary]
            inputs = set().union(*(self.fresh(self.value(v)) for v in args))
            self.begin(scope, call=(mode, inputs))
            self.move_ids(inputs, 'scope:' + scope)
            for ident in inputs:
                self.identities[ident].provenance = 'bound'
        elif op == 'call.exit':
            scope, result = a[:2]
            target = self.scopes[scope].parent
            if result != '-':
                self.prepare(scope, result)
            failure = self.failures[a[2]] if len(a) > 2 else None
            self.close(scope, failure)
            if result != '-' and not self.last_failure:
                self.commit(scope, target)
        elif op == 'exit.prepare':
            self.prepare(a[0], a[1])
        elif op == 'exit.commit':
            self.commit(a[0], a[1])
        elif op == 'exit.abort':
            self.abort(a[0])
        elif op in ('defer', 'defer.block'):
            require(self.scopes[a[0]].phase == 'open', 'B4', 'late defer registration')
            self.scopes[a[0]].defers.append(tuple(a[1:]) if op == 'defer.block' else (shlex.join(a[1:]),))
        elif op == 'destroy.log':
            obj = self.identities[max(self.value(a[0]).ids)]
            obj.log_begin, obj.log_end = a[1:]
        elif op == 'check.value':
            actual = self.value(a[0]).scalar
            require(actual == a[1], 'CHECK', f'value {actual!r} != {a[1]!r}')
            self.checkpoints.append(actual)
        elif op == 'on.destroy':
            ids = self.value(a[0]).ids
            self.identities[max(ids)].destroy = tuple(a[1:])
        elif op == 'failure':
            self.failures[a[0]] = Failure(a[1], a[2])
        elif op == 'failure.cause':
            self.link_failure(self.failures[a[0]], self.failures[a[1]], cause=True)
        elif op == 'failure.secondary':
            self.link_failure(self.failures[a[0]], self.failures[a[1]])
        elif op == 'failure.view':
            roots = frozenset(self.failure_roots(self.failures[a[1]]))
            caches = frozenset(n.cache for n in self.failure_graph(self.failures[a[1]]) if n.cache)
            self.put(a[0], Value(roots, roots, views=caches, kind='carrier-view'))
        elif op in ('carrier', 'carrier.attach'):
            name, packet = a[:2]
            target = a[2] if len(a) == 3 else self.scopes[self.current].parent
            require(target in self.scopes, 'B7', 'root failure needs diagnostic owner')
            require(self.scopes[target].phase == 'open' and self.ancestor(target, self.current),
                    'B3', 'carrier owner must enclose current scope')
            require(not self.failures[name].holder and not self.failures[name].cache,
                    'B7', 'carrier already attached')
            ids = self.token(packet)
            existing = self.failure_graph(self.failures[name])
            owners = {self.bag_scope[self.identities[i].holder]
                      for n in existing if not n.cache for i in n.roots}
            require(not owners or owners == {target}, 'B7', 'carrier graph requires common owner before attach')
            for node in existing:
                if node.cache:
                    require(self.ancestor(self.futures[node.cache].owner, target),
                            'B9', 'cache.rehome required for borrowed failure graph')
            dest = 'carrier:' + name
            self.bag(dest, target)
            self.move_ids(ids, dest)
            self.consume(packet)
            self.provenance(ids, 'staged/carrier-owned')
            self.failures[name].roots = frozenset(ids)
            self.failures[name].holder = dest

        elif op in ('carrier.transfer', 'carrier.rehome', 'carrier.release'):
            failure, target = self.failures[a[0]], a[1]
            if target.startswith('cache:'):
                require(op == 'carrier.transfer', 'B9', 'cache requires transfer protocol')
                name = target[6:]
                f = self.futures[name]
                require(f.state == 'cold' and not f.args and f.failure is None,
                        'B9', 'carrier cache publication needs settled inputs/frame')
                nodes = self.failure_graph(failure)
                require(not failure.cache, 'B9', 'failure-cache requires cache protocol')
                for node in nodes:
                    if node.cache:
                        require(self.ancestor(self.futures[node.cache].owner, f.owner),
                                'B9', 'cache.rehome required for borrowed failure graph')
                roots = set().union(*(n.roots for n in nodes if not n.cache))
                self.escape_deps(Value(roots, roots), roots, f.owner)
                self.move_ids(roots, target, protocol=True)
                self.provenance(roots, 'cache-owned')
                for n in nodes:
                    if not n.cache:
                        n.cache, n.holder = name, target
                f.failure, f.state = failure, 'settled'
                self.future_settled(name)
            else:
                self.carrier_move(failure, target, op == 'carrier.release', op == 'carrier.rehome')
        elif op == 'scope.end':
            self.close(a[0])
        elif op == 'scope.unwind':
            self.close(a[0], self.failures[a[1]])
        elif op == 'catch':
            f = self.last_failure
            require(f is not None and f.cleaned, 'B6', 'catch before try cleanup')
            require(f.kind == 'typed' and f.name == a[0], 'B6', 'failure is not catchable by this handler')
            self.failures[a[1]] = f
        elif op == 'assert.failure':
            f = self.last_failure
            actual = [f.name] + [x.name for x in f.secondaries] if f else []
            require(actual == a, 'CHECK', f'failure order {actual} != {a}')
        elif op == 'cleanup.begin':
            self.start_close(a[0], self.failures[a[1]] if len(a) > 1 else None)
        elif op == 'cleanup.step':
            self.close_step(a[0])
        elif op == 'cleanup.resume':
            require(self.scopes[a[0]].suspended, 'B10', 'cleanup is not suspended')
            self.finish_destroy(a[0])
        elif op == 'cancel':
            self.scopes[a[0]].cancelled = True
        elif op == 'gc':
            expected = set().union(*(self.value(n, live=False).deps for n in a))
            require(expected <= self.roots(), 'B10', 'cleanup root missing')
            self.event('gc', roots=sorted(self.roots()))
        elif op == 'destroy.direct':
            scope, name = a
            s = self.scopes[scope]
            if s.phase == 'service-settle':
                self.settle_services(scope)
            require(s.phase == 'closing' and not s.suspended, 'B10', 'direct destroy outside cleanup plan')
            ids = self.value(name).ids
            require(max(self.remaining(scope)) in ids, 'B10', 'direct cleanup violates global serial order')
            self.close_step(scope)
        elif op == 'end':
            require(len(a) > 1 and a[1] == 'boundary', 'B4', 'ASAP end is not a lexical boundary')
            self.move_ids(self.token(a[0]), 'scope:' + self.current)
            self.consume(a[0])
            self.close(self.current)
        elif op == 'succ':
            self.value(a[0])
            self.must_use[a[0]] = self.current
        elif op == 'succ.take':
            require(a[0] in self.must_use, 'B8', 'no must-use obligation')
            del self.must_use[a[0]]
        elif op == 'builder.new':
            require(a[0] not in self.builders, 'B8', 'builder buffer already exists')
            self.builders[a[0]] = (self.current, [], set())
        elif op == 'builder.push':
            old, new, name, outcome = a
            require(old in self.builders and new not in self.builders, 'B8', 'consumed/nonunique builder')
            owner, items, owned = self.builders[old]
            value = self.value(name)
            require(outcome in ('ok', 'fail'), 'B6', 'unknown builder poststate')
            if outcome == 'ok':
                ids = self.fresh(value)
                self.move_ids(ids, 'scope:' + owner)
                for ident in ids:
                    self.identities[ident].provenance = 'bound'
                self.builders[new] = (owner, items + [name], owned | ids)
                del self.builders[old]
        elif op in ('builder.finish', 'builder.abandon'):
            require(a[0] in self.builders, 'B8', 'consumed builder buffer')
            owner, items, owned = self.builders.pop(a[0])
            if op == 'builder.finish':
                self.put(a[1], self.aggregate(items))
                for ident in owned:
                    self.identities[ident].provenance = 'unbound-result'
        elif op == 'future':
            name, mode, result, lifetime, *args = a
            require(mode in ('new', 'borrow', 'void', 'fail', 'literal'), 'B9', 'unknown future body')
            require(lifetime in ('bound', 'temporary'), 'B9', 'future lifetime missing')
            ids = set().union(*(self.fresh(self.value(v)) for v in args))
            self.futures[name] = Future(self.current, tuple(args), mode, result, lifetime == 'temporary')
            for kind in ('inputs', 'frame', 'cache'):
                self.bag(kind + ':' + name, self.current)
            self.move_ids(ids, 'inputs:' + name)
            for ident in ids:
                self.identities[ident].provenance = 'bound'
            self.put(name, Value(futures=frozenset({name}), kind='future'))
        elif op == 'future.body':
            require(self.futures[a[0]].state == 'cold', 'B9', 'body after task start')
            self.futures[a[0]].body = tuple(a[1:])
        elif op == 'future.request-cancel':
            self.futures[a[0]].cancel_requested = True
            self.event('future.cancel-request', future=a[0])
        elif op in ('future.start', 'future.settle', 'future.cancel'):
            self.value(a[0])
            if op == 'future.start':
                self.start_future(a[0])
            else:
                self.settle(a[0], op == 'future.cancel')
        elif op == 'unique-cache':
            capability, name = a
            require(capability not in self.unique, 'B9', 'duplicate unique-cache capability')
            require(name in self.futures, 'B9', 'not a Future')
            refs = [v for key, v in self.values.items() if key != name and name in v.futures | v.views]
            require(not refs and not self.futures[name].waiters and name not in self.unique.values(), 'B9', 'cache is shared')
            self.unique[capability] = name
        elif op in ('await', 'await.take'):
            output, reference = a[:2]
            self.event('future.await', reference=reference, take=op == 'await.take')
            value = self.value(reference)
            require(len(value.futures) == 1 and value.kind == 'future', 'B9', 'await requires a Future')
            name = next(iter(value.futures))
            f = self.futures[name]
            if self.scopes[self.current].cancelled and f.state != 'settled':
                self.last_failure = Failure('cancel', 'Cancelled', cleaned=True)
                return
            if op == 'await.take':
                require(len(a) == 3 and self.unique.get(a[2]) == name, 'B9', 'await.take requires unique-cache')
                refs = [v for key, v in self.values.items() if key != name and name in v.futures | v.views]
                require(not refs and not f.waiters, 'B9', 'uniqueness invalidated by alias/view/waiter')
                require(f.temporary or f.owner == self.current, 'B9', 'take changes lexical cleanup boundary')
                del self.unique[a[2]]
            self.settle(name)
            if f.failure:
                if op == 'await.take':
                    ids = set(self.bags['cache:' + name])
                    self.move_ids(ids, 'scope:' + self.current, protocol=True)
                    self.provenance(ids, 'temporary-await-result' if f.temporary else 'extracted-at-existing-boundary')
                    for node in self.failure_graph(f.failure):
                        if node.cache == name:
                            node.cache, node.holder = None, 'scope:' + self.current
                self.last_failure = f.failure
                f.failure.cleaned = True
                if op == 'await.take':
                    f.state = 'consumed'
                return
            v = self.value(f.outcome)
            if op == 'await.take':
                ids = set(self.bags['cache:' + name])
                self.move_ids(ids, 'scope:' + self.current, protocol=True)
                for ident in ids:
                    self.identities[ident].provenance = ('temporary-await-result' if f.temporary
                                                         else 'extracted-at-existing-boundary')
                f.state = 'consumed'
                self.put(output, v)
            else:
                self.put(output, Value(v.ids, v.deps, v.fields, v.items, v.futures, v.views | {name}, v.kind, v.scalar))
        elif op == 'cache.rehome':
            name, target = a
            f = self.futures[name]
            require(self.ancestor(target, f.owner), 'B9', 'rehome must extend to enclosing scope')
            held = set().union(*(self.bags[kind + ':' + name] for kind in ('inputs', 'frame', 'cache')))
            if f.task_scope:
                held |= {i for i, obj in self.identities.items() if obj.state == 'live'
                         and self.ancestor(f.task_scope, self.bag_scope[obj.holder])}
            if f.state in ('cold', 'running'):
                for arg in f.args:
                    v = self.value(arg)
                    self.escape_deps(v, held & v.ids, target)
            if f.outcome:
                v = self.value(f.outcome)
                self.escape_deps(v, set(self.bags['cache:' + name]), target)
            if f.failure:
                for node in self.failure_graph(f.failure):
                    if node.cache and node.cache != name:
                        require(self.ancestor(self.futures[node.cache].owner, target),
                                'B9', 'cache.rehome required for borrowed failure graph')
            f.owner = target
            if f.task_scope:
                self.scopes[f.task_scope].parent = target
            for kind in ('inputs', 'frame', 'cache'):
                self.rehome_bag(kind + ':' + name, target)
        elif op == 'waiter':
            self.futures[a[1]].waiters.add(a[0])
            self.event('waiter.register', future=a[1], waiter=a[0])
        elif op == 'waiter.cancel':
            waiter, name = a
            f = self.futures[name]
            require(waiter in f.waiters, 'B9', 'unknown waiter')
            f.waiters.remove(waiter)
            if not f.waiters:
                self.settle(name, cancel=True)
            self.event('waiter.cancelled', future=name, waiter=waiter)
        elif op == 'group':
            require(a[0] not in self.groups, 'B9', 'duplicate group')
            self.groups[a[0]] = []
        elif op == 'group.start':
            group, name = a
            require(self.futures[name].owner == self.current, 'B9', 'group requires explicit cache.rehome')
            if name not in self.groups[group]:
                self.groups[group].append(name)
                if self.futures[name].state == 'cold':
                    self.start_future(name)
        elif op in ('group.wait', 'group.cancel'):
            for name in self.groups[a[0]]:
                if op == 'group.cancel':
                    self.futures[name].cancel_requested = True
                else:
                    self.settle(name)
        elif op == 'race.winner':
            output, winner, *losers = a
            observed = [e['future'] for e in self.events if e['event'] == 'future.settled'
                        and e['future'] in [winner, *losers]]
            require(not observed or observed[0] == winner, 'B9', 'race winner is not first settled input')
            self.settle(winner)
            f = self.futures[winner]
            outcome_failure = f.failure
            for name in dict.fromkeys(losers):
                if name == winner:
                    continue
                self.settle(name, cancel=True)
                failure = self.futures[name].failure
                failures = (failure.secondaries if failure and failure.kind == 'cancel'
                            else [failure] if failure else [])
                for failure in failures:
                    if outcome_failure is None:
                        outcome_failure = failure
                    elif failure is not outcome_failure:
                        self.link_failure(outcome_failure, failure)
            if outcome_failure:
                self.last_failure = outcome_failure
            else:
                v = self.value(f.outcome)
                self.put(output, Value(v.ids, v.deps, v.fields, v.items, v.futures,
                                       v.views | {winner}, v.kind, v.scalar))
        elif op == 'collect.all':
            output, *names = a
            for name in dict.fromkeys(names):
                if self.futures[name].state == 'cold':
                    self.start_future(name)
            seen_failure = any(self.futures[n].failure for n in names)
            for name in dict.fromkeys(names):
                self.settle(name, cancel=seen_failure)
                seen_failure |= self.futures[name].failure is not None
            order = [e['future'] for e in self.events if e['event'] == 'future.settled' and e['future'] in names]
            order += [n for n in names if n not in order]
            failed = [self.futures[n].failure for n in dict.fromkeys(order) if self.futures[n].failure]
            if failed:
                self.last_failure = failed[0]
                for name in dict.fromkeys(names):
                    secondary = self.futures[name].failure
                    if secondary and secondary is not self.last_failure and not any(
                            item is secondary for item in self.last_failure.secondaries):
                        self.last_failure.secondaries.append(secondary)
            else:
                items = [self.futures[n].outcome for n in names]
                v = self.aggregate(items)
                v.views = frozenset(names)
                self.put(output, v)
        elif op == 'completions':
            self.put(a[0], self.aggregate(a[1:], 'completions'))
            self.stream_ready[a[0]] = [n for n in a[1:] if self.futures[n].state == 'settled']
        elif op == 'completion.next':
            output, stream = a
            value = self.value(stream)
            require(value.kind == 'completions' and value.items, 'B9', 'NoFutures')
            for name in dict.fromkeys(value.items):
                if self.futures[name].state == 'cold':
                    self.start_future(name)
            ready = self.stream_ready[stream]
            if not ready:
                self.settle(value.items[0])
            name = ready.pop(0)
            self.put(output, self.value(name))
            items = list(value.items)
            items.remove(name)
            self.values[stream] = Value(items=tuple(items), futures=value.futures, kind='completions')
        elif op == 'frame.save':
            scope, frame = a
            require(self.profile == 'coro' and scope not in self.frames, 'B11', 'invalid coro frame save')
            holders = {i: obj.holder for i, obj in self.identities.items()
                       if obj.state == 'live' and self.bag_scope[obj.holder] == scope}
            self.bag('saved:' + frame, scope)
            self.move_ids(set(holders), 'saved:' + frame, protocol=True)
            self.frames[scope] = (frame, holders)
            self.event('frame.save', scope=scope, frame=frame, roots=sorted(self.roots()))
        elif op == 'frame.resume':
            scope, frame = a
            require(scope in self.frames and self.frames[scope][0] == frame, 'B9', 'frame not uniquely held')
            for ident, holder in self.frames[scope][1].items():
                self.move_ids({ident}, holder, protocol=True)
            del self.frames[scope]
        elif op == 'storage':
            name, protocol = a
            require(protocol in ('atomic', 'cell', 'channel'), 'B7', 'unknown storage protocol')
            self.stores[name] = {'protocol': protocol, 'owner': self.current, 'items': {}, 'closed': False}
            self.bag('store:' + name, self.current)
        elif op == 'storage.set':
            name, key, value = a
            store = self.stores[name]
            require(not store['closed'], 'B7', 'publication to closed storage')
            v = self.value(value)
            if store['protocol'] == 'channel':
                ids = self.fresh(v)
                self.move_ids(ids, 'store:' + name)
                for ident in ids:
                    self.identities[ident].provenance = 'bound'
            store['items'][key] = value
        elif op == 'storage.get':
            output, name, key = a
            store = self.stores[name]
            value = store['items'][key]
            v = self.value(value)
            if store['protocol'] == 'channel':
                ids = set(self.bags['store:' + name]) & v.ids
                self.move_ids(ids, 'scope:' + self.current, protocol=True)
                del store['items'][key]
            self.put(output, v)
        elif op in ('storage.remove', 'storage.clear', 'storage.close'):
            store = self.stores[a[0]]
            if op == 'storage.remove':
                store['items'].pop(a[1], None)
            elif op == 'storage.clear':
                store['items'].clear()
            else:
                store['closed'] = True
        elif op in ('once', 'worker', 'native', 'error.payload', 'descriptor.plain'):
            v = self.value(a[0])
            require(not v.deps and not v.futures and (op in ('once', 'descriptor.plain') or v.kind == 'plain'),
                    'B11', f'{op} requires an admissible plain value')
        elif op == 'emit':
            self.log += a[0]
        elif op == 'emit.label':
            self.log += self.identities[max(self.value(a[0]).ids)].label
        elif op == 'reset.log':
            self.log = ''
        elif op == 'check':
            require(self.log == a[0], 'CHECK', f'trace {self.log!r} != {a[0]!r}')
            self.checkpoints.append(self.log)
        elif op == 'assert.owner':
            require(all(self.bag_scope[self.identities[i].holder] == a[1] for i in self.value(a[0]).ids),
                    'CHECK', 'unexpected owner')
        elif op == 'assert.items':
            require(list(self.value(a[0]).items) == a[1:], 'B9', 'collector positions changed')
        elif op == 'assert.state':
            require(self.scopes[a[0]].phase == a[1], 'CHECK', 'unexpected scope phase')
        elif op == 'label':
            self.block = a[0]
            self.block_definitions.clear()
        elif op == 'unwind.use':
            require(a[0] not in self.block_definitions, 'B6', 'ordinary unwind reads block-local definition')
            self.value(a[0])
        elif op == 'edge.bind':
            name, result = a
            require(self.edge is not None and result in self.edge, 'B6', 'edge result unavailable on this successor')
            kind, payload = self.edge.pop(result)
            if kind == 'packet':
                self.packet(name, self.token(payload), self.bag_scope[payload])
                self.consume(payload)
            elif kind == 'value':
                self.put(name, payload)
            else:
                self.failures[name] = payload
        else:
            raise ModelError('IR', f'unknown instruction {op}')
