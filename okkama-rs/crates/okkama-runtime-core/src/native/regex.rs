//! `runtime/regex`: собственный движок регулярных выражений Okkama — перенос
//! `RegexNatives` JVM (порт `Okkama::Runtime::Regex::Compiler`): разбор
//! шаблона, NFA Томпсона и поиск самого левого, затем самого длинного
//! совпадения с захватами. Смещения — в символах (code points).
use super::Error;
use super::support::{
    failure, handle, int, int_map, int_value, ints, new_handle, string, string_array, text,
};
use crate::{IntValue, Value, abi::Native};
use std::cell::RefCell;
use std::collections::BTreeMap;

const DOMAIN: &str = "core.regex";
const MAX_PATTERN: usize = 4096;
const MAX_STATES: usize = 16384;
const MAX_REPETITION: i64 = 4096;
const MAX_NESTING: usize = 256;
const MAX_GROUPS: usize = 1024;
const MAX_INPUT: usize = 1_048_576;

enum CompileFailure {
    Syntax(i64),
    /// Вид предела (0 — шаблон, 1 — вложенность, 2 — группы, 3 — состояния) и предел.
    Limit(i64, i64),
}

type Compiled<T> = Result<T, CompileFailure>;

#[derive(Clone, Debug)]
enum Matcher {
    Ranges(Vec<(u32, u32)>),
    Class { negate: bool, members: Vec<Matcher> },
    Dot,
    Shorthand { code: char, negate: bool },
}

fn literal(value: u32) -> Matcher {
    Matcher::Ranges(vec![(value, value)])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Jump,
    Save,
    Split,
    AssertStart,
    AssertEnd,
    Consume,
    Match,
}

#[derive(Clone, Debug)]
struct Instruction {
    op: Op,
    out: i64,
    out1: i64,
    slot: usize,
    matcher: Option<Matcher>,
}

fn instruction(op: Op) -> Instruction {
    Instruction {
        op,
        out: -1,
        out1: -1,
        slot: 0,
        matcher: None,
    }
}

enum Node {
    Empty,
    Consume(Matcher),
    Anchor(Op),
    Capture(usize, Box<Node>),
    Concat(Vec<Node>),
    Alternate(Box<Node>, Box<Node>),
    Repeat(Box<Node>, i64, i64),
}

const META: &str = "\\.^$|()[]{}?*+";

fn meta(point: u32) -> bool {
    point < 128 && META.contains(char::from_u32(point).expect("ascii"))
}

fn identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

struct Parser {
    source: Vec<u32>,
    index: usize,
    capture_count: usize,
    group_names: BTreeMap<String, usize>,
    nesting: usize,
    verbose: bool,
}

impl Parser {
    fn invalid(&self) -> CompileFailure {
        CompileFailure::Syntax(self.index as i64)
    }

    fn at_end(&self) -> bool {
        self.index >= self.source.len()
    }

    fn peek(&self) -> Option<u32> {
        self.source.get(self.index).copied()
    }

    fn advance(&mut self) -> Compiled<u32> {
        let point = self.peek().ok_or_else(|| self.invalid())?;
        self.index += 1;
        Ok(point)
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected as u32) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn skip_ignored(&mut self) {
        if !self.verbose {
            return;
        }
        while let Some(next) = self.peek() {
            if matches!(next, 0x20 | 0x09 | 0x0a | 0x0d | 0x0c | 0x0b) {
                self.index += 1;
                continue;
            }
            if next == '#' as u32 {
                self.index += 1;
                while !self.at_end() && self.peek() != Some('\n' as u32) {
                    self.index += 1;
                }
                continue;
            }
            break;
        }
    }

    fn hex(point: u32) -> Option<u32> {
        char::from_u32(point).and_then(|c| c.to_digit(16))
    }

    fn shorthand(point: u32) -> Option<Matcher> {
        let (lower, negate) = match char::from_u32(point)? {
            'D' => ('d', true),
            'W' => ('w', true),
            'S' => ('s', true),
            c @ ('d' | 'w' | 's') => (c, false),
            _ => return None,
        };
        Some(Matcher::Shorthand {
            code: lower,
            negate,
        })
    }

    /// Экранирование: matcher и признак сокращения класса.
    fn escaped(&mut self, allow_shorthand: bool) -> Compiled<(Matcher, bool)> {
        if !self.eat('\\') {
            return Err(self.invalid());
        }
        let escaped = self.advance()?;
        match char::from_u32(escaped) {
            Some('n') => return Ok((literal('\n' as u32), false)),
            Some('r') => return Ok((literal('\r' as u32), false)),
            Some('t') => return Ok((literal('\t' as u32), false)),
            Some('x') => {
                if !self.eat('{') {
                    return Err(self.invalid());
                }
                let value_offset = self.index;
                let mut value: u32 = 0;
                let mut digits = 0;
                while !self.at_end() && self.peek() != Some('}' as u32) {
                    let digit_offset = self.index;
                    let digit = Self::hex(self.advance()?)
                        .ok_or(CompileFailure::Syntax(digit_offset as i64))?;
                    value = value * 16 + digit;
                    digits += 1;
                    if value > 0x10ffff {
                        return Err(CompileFailure::Syntax(digit_offset as i64));
                    }
                }
                if digits == 0 || !self.eat('}') {
                    return Err(self.invalid());
                }
                if (0xd800..=0xdfff).contains(&value) {
                    return Err(CompileFailure::Syntax(value_offset as i64));
                }
                return Ok((literal(value), false));
            }
            _ => {}
        }
        if allow_shorthand {
            if let Some(matcher) = Self::shorthand(escaped) {
                return Ok((matcher, true));
            }
        }
        if !(meta(escaped) || escaped == '-' as u32 || escaped == ' ' as u32 || escaped == '#' as u32)
        {
            return Err(CompileFailure::Syntax(self.index as i64 - 1));
        }
        Ok((literal(escaped), false))
    }

    fn number(&mut self) -> Compiled<i64> {
        let mut value: i64 = 0;
        let mut digits = 0;
        while let Some(point) = self.peek() {
            if !(('0' as u32)..=('9' as u32)).contains(&point) {
                break;
            }
            self.index += 1;
            value = value * 10 + i64::from(point - '0' as u32);
            digits += 1;
            if value > MAX_REPETITION {
                return Err(CompileFailure::Syntax(self.index as i64 - 1));
            }
        }
        if digits == 0 {
            return Err(self.invalid());
        }
        Ok(value)
    }

    fn class_item(&mut self) -> Compiled<(Matcher, bool)> {
        if self.peek() == Some('\\' as u32) {
            return self.escaped(true);
        }
        let value = self.advance()?;
        if value == ']' as u32 {
            return Err(self.invalid());
        }
        Ok((literal(value), false))
    }

    fn first_point(matcher: &Matcher) -> u32 {
        match matcher {
            Matcher::Ranges(ranges) => ranges[0].0,
            _ => unreachable!("class range endpoints are literals"),
        }
    }

    fn character_class(&mut self) -> Compiled<Matcher> {
        if !self.eat('[') {
            return Err(self.invalid());
        }
        let negate = self.eat('^');
        let mut members = Vec::new();
        let mut items = 0;
        while !self.at_end() && self.peek() != Some(']' as u32) {
            let (first, short) = self.class_item()?;
            items += 1;
            if !short
                && self.peek() == Some('-' as u32)
                && self.index + 1 < self.source.len()
                && self.source[self.index + 1] != ']' as u32
            {
                self.advance()?;
                let (last, short) = self.class_item()?;
                if short {
                    return Err(self.invalid());
                }
                let (from, to) = (Self::first_point(&first), Self::first_point(&last));
                if from > to {
                    return Err(CompileFailure::Syntax(self.index as i64 - 1));
                }
                members.push(Matcher::Ranges(vec![(from, to)]));
                continue;
            }
            members.push(first);
        }
        if items == 0 || !self.eat(']') {
            return Err(self.invalid());
        }
        Ok(Matcher::Class { negate, members })
    }

    fn primary(&mut self) -> Compiled<Node> {
        self.skip_ignored();
        let next = self.peek().ok_or_else(|| self.invalid())?;
        if next == '(' as u32 {
            self.advance()?;
            let mut capturing = true;
            let mut name = None;
            if self.eat('?') {
                if self.eat(':') {
                    capturing = false;
                } else if self.eat('<') {
                    let start = self.index;
                    let mut text = String::new();
                    while !self.at_end() && self.peek() != Some('>' as u32) {
                        text.push(char::from_u32(self.advance()?).expect("scalar"));
                    }
                    if text.is_empty() || !self.eat('>') {
                        return Err(CompileFailure::Syntax(start as i64));
                    }
                    if !identifier(&text) || self.group_names.contains_key(&text) {
                        return Err(CompileFailure::Syntax(start as i64));
                    }
                    name = Some(text);
                } else {
                    return Err(self.invalid());
                }
            }
            if capturing && self.capture_count >= MAX_GROUPS {
                return Err(CompileFailure::Limit(2, MAX_GROUPS as i64));
            }
            let capture_index = if capturing {
                self.capture_count += 1;
                self.capture_count
            } else {
                0
            };
            if let Some(name) = name {
                self.group_names.insert(name, capture_index);
            }
            self.nesting += 1;
            if self.nesting > MAX_NESTING {
                return Err(CompileFailure::Limit(1, MAX_NESTING as i64));
            }
            let inner = self.alternation()?;
            self.nesting -= 1;
            if !self.eat(')') {
                return Err(self.invalid());
            }
            if !capturing {
                return Ok(inner);
            }
            return Ok(Node::Capture(capture_index, Box::new(inner)));
        }
        if next == '[' as u32 {
            return Ok(Node::Consume(self.character_class()?));
        }
        if next == '\\' as u32 {
            return Ok(Node::Consume(self.escaped(true)?.0));
        }
        if next == '.' as u32 {
            self.advance()?;
            return Ok(Node::Consume(Matcher::Dot));
        }
        if next == '^' as u32 || next == '$' as u32 {
            self.advance()?;
            return Ok(Node::Anchor(if next == '^' as u32 {
                Op::AssertStart
            } else {
                Op::AssertEnd
            }));
        }
        if meta(next) {
            return Err(self.invalid());
        }
        self.advance()?;
        Ok(Node::Consume(literal(next)))
    }

    fn repetition(&mut self) -> Compiled<Node> {
        let inner = self.primary()?;
        self.skip_ignored();
        if self.at_end() {
            return Ok(inner);
        }
        let (minimum, maximum) = if self.eat('?') {
            (0, 1)
        } else if self.eat('*') {
            (0, -1)
        } else if self.eat('+') {
            (1, -1)
        } else if self.eat('{') {
            let minimum = self.number()?;
            if self.eat('}') {
                (minimum, minimum)
            } else {
                if !self.eat(',') {
                    return Err(self.invalid());
                }
                let mut maximum_offset = -1;
                let maximum = if self.peek() == Some('}' as u32) {
                    -1
                } else {
                    maximum_offset = self.index as i64;
                    self.number()?
                };
                if !self.eat('}') {
                    return Err(self.invalid());
                }
                if maximum >= 0 && maximum < minimum {
                    return Err(CompileFailure::Syntax(maximum_offset));
                }
                (minimum, maximum)
            }
        } else {
            return Ok(inner);
        };
        if let Some(next) = self.peek() {
            if ['?', '*', '+', '{'].iter().any(|c| *c as u32 == next) {
                return Err(self.invalid());
            }
        }
        Ok(Node::Repeat(Box::new(inner), minimum, maximum))
    }

    fn concatenation(&mut self) -> Compiled<Node> {
        let mut children = Vec::new();
        while !self.at_end() {
            self.skip_ignored();
            let Some(next) = self.peek() else {
                break;
            };
            if next == '|' as u32 || next == ')' as u32 {
                break;
            }
            children.push(self.repetition()?);
        }
        Ok(match children.len() {
            0 => Node::Empty,
            1 => children.pop().expect("one child"),
            _ => Node::Concat(children),
        })
    }

    fn alternation(&mut self) -> Compiled<Node> {
        let mut result = self.concatenation()?;
        self.skip_ignored();
        while self.eat('|') {
            let right = self.concatenation()?;
            result = Node::Alternate(Box::new(result), Box::new(right));
            self.skip_ignored();
        }
        Ok(result)
    }

    fn parse(&mut self) -> Compiled<Node> {
        self.skip_ignored();
        let result = self.alternation()?;
        self.skip_ignored();
        if !self.at_end() {
            return Err(self.invalid());
        }
        Ok(result)
    }
}

/// Фрагмент NFA: начало и незаполненные выходы (индекс * 2 + поле).
struct Fragment {
    start: usize,
    outs: Vec<usize>,
}

fn emit(instructions: &mut Vec<Instruction>, instruction: Instruction) -> Compiled<usize> {
    if instructions.len() >= MAX_STATES {
        return Err(CompileFailure::Limit(3, MAX_STATES as i64));
    }
    instructions.push(instruction);
    Ok(instructions.len() - 1)
}

fn patch(instructions: &mut [Instruction], outs: &[usize], target: usize) {
    for out in outs {
        let instruction = &mut instructions[out >> 1];
        if out & 1 == 0 {
            instruction.out = target as i64;
        } else {
            instruction.out1 = target as i64;
        }
    }
}

fn single(instructions: &mut Vec<Instruction>, instruction: Instruction) -> Compiled<Fragment> {
    let index = emit(instructions, instruction)?;
    Ok(Fragment {
        start: index,
        outs: vec![index * 2],
    })
}

fn concat(instructions: &mut [Instruction], left: Fragment, right: Fragment) -> Fragment {
    patch(instructions, &left.outs, right.start);
    Fragment {
        start: left.start,
        outs: right.outs,
    }
}

fn compile(node: &Node, instructions: &mut Vec<Instruction>) -> Compiled<Fragment> {
    match node {
        Node::Empty => single(instructions, instruction(Op::Jump)),
        Node::Consume(matcher) => {
            let mut consume = instruction(Op::Consume);
            consume.matcher = Some(matcher.clone());
            single(instructions, consume)
        }
        Node::Anchor(op) => single(instructions, instruction(*op)),
        Node::Capture(index, child) => {
            let fragment = compile(child, instructions)?;
            let mut open = instruction(Op::Save);
            open.slot = (index - 1) * 2;
            open.out = fragment.start as i64;
            let start = emit(instructions, open)?;
            let mut close = instruction(Op::Save);
            close.slot = (index - 1) * 2 + 1;
            let end = emit(instructions, close)?;
            patch(instructions, &fragment.outs, end);
            Ok(Fragment {
                start,
                outs: vec![end * 2],
            })
        }
        Node::Concat(children) => {
            let mut fragment = compile(&children[0], instructions)?;
            for child in &children[1..] {
                let next = compile(child, instructions)?;
                fragment = concat(instructions, fragment, next);
            }
            Ok(fragment)
        }
        Node::Alternate(left, right) => {
            let left = compile(left, instructions)?;
            let right = compile(right, instructions)?;
            let mut split = instruction(Op::Split);
            split.out = left.start as i64;
            split.out1 = right.start as i64;
            let index = emit(instructions, split)?;
            let mut outs = left.outs;
            outs.extend(right.outs);
            Ok(Fragment { start: index, outs })
        }
        Node::Repeat(child, minimum, maximum) => {
            let mut fragment: Option<Fragment> = None;
            for _ in 0..*minimum {
                let copy = compile(child, instructions)?;
                fragment = Some(match fragment {
                    None => copy,
                    Some(fragment) => concat(instructions, fragment, copy),
                });
            }
            if *maximum < 0 {
                let body = compile(child, instructions)?;
                let mut split = instruction(Op::Split);
                split.out = body.start as i64;
                let index = emit(instructions, split)?;
                patch(instructions, &body.outs, index);
                let star = Fragment {
                    start: index,
                    outs: vec![index * 2 + 1],
                };
                return Ok(match fragment {
                    None => star,
                    Some(fragment) => concat(instructions, fragment, star),
                });
            }
            for _ in 0..(maximum - minimum) {
                let body = compile(child, instructions)?;
                let mut split = instruction(Op::Split);
                split.out = body.start as i64;
                let index = emit(instructions, split)?;
                let mut outs = body.outs;
                outs.push(index * 2 + 1);
                let optional = Fragment { start: index, outs };
                fragment = Some(match fragment {
                    None => optional,
                    Some(fragment) => concat(instructions, fragment, optional),
                });
            }
            match fragment {
                Some(fragment) => Ok(fragment),
                None => single(instructions, instruction(Op::Jump)),
            }
        }
    }
}

#[derive(Debug)]
struct Program {
    instructions: Vec<Instruction>,
    start: usize,
    capture_count: usize,
    group_names: BTreeMap<String, usize>,
    ascii: bool,
    ignore_case: bool,
    multiline: bool,
    dot_all: bool,
}

/// Состояние с тегами: инструкция, захваты, начало совпадения.
struct Tagged {
    index: usize,
    captures: Vec<i64>,
    match_start: usize,
}

fn is_letter(point: u32) -> bool {
    matches!(
        crate::data::category(point),
        "Lu" | "Ll" | "Lt" | "Lm" | "Lo"
    )
}

fn is_number(point: u32) -> bool {
    matches!(crate::data::category(point), "Nd" | "Nl" | "No")
}

impl Program {
    fn matches_character(&self, matcher: &Matcher, point: u32) -> bool {
        match matcher {
            Matcher::Dot => self.dot_all || point != '\n' as u32,
            Matcher::Class { negate, members } => {
                let inside = members
                    .iter()
                    .any(|member| self.matches_character(member, point));
                *negate != inside
            }
            Matcher::Shorthand { code, negate } => {
                let inside = match code {
                    'd' => {
                        if self.ascii {
                            (('0' as u32)..=('9' as u32)).contains(&point)
                        } else {
                            crate::data::category(point) == "Nd"
                        }
                    }
                    'w' => {
                        if self.ascii {
                            char::from_u32(point)
                                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
                        } else {
                            is_letter(point) || is_number(point) || point == '_' as u32
                        }
                    }
                    _ => {
                        (9..=13).contains(&point)
                            || point == 32
                            || (!self.ascii
                                && (point == 0x85
                                    || point == 0xa0
                                    || point == 0x1680
                                    || (0x2000..=0x200a).contains(&point)
                                    || point == 0x2028
                                    || point == 0x2029
                                    || point == 0x202f
                                    || point == 0x205f
                                    || point == 0x3000))
                    }
                };
                *negate != inside
            }
            Matcher::Ranges(ranges) => {
                let candidates = self.candidates(point);
                ranges.iter().any(|(from, to)| {
                    candidates
                        .iter()
                        .any(|candidate| (*from..=*to).contains(candidate))
                })
            }
        }
    }

    fn candidates(&self, point: u32) -> Vec<u32> {
        if !self.ignore_case {
            return vec![point];
        }
        if self.ascii {
            return match char::from_u32(point) {
                Some(c) if c.is_ascii_uppercase() => vec![point, point + 32],
                Some(c) if c.is_ascii_lowercase() => vec![point, point - 32],
                _ => vec![point],
            };
        }
        let mut out = vec![point];
        let Some(c) = char::from_u32(point) else {
            return out;
        };
        let upper: String = c.to_uppercase().collect();
        let variants = [
            c.to_lowercase().collect::<String>(),
            upper.clone(),
            upper.to_lowercase(),
        ];
        for variant in variants {
            let mut chars = variant.chars();
            if let (Some(single), None) = (chars.next(), chars.next()) {
                out.push(single as u32);
            }
        }
        out
    }

    /// Замыкание по ε-переходам в порядке приоритета (обход в глубину).
    fn add_tagged(
        &self,
        states: &mut Vec<Tagged>,
        seen: &mut [bool],
        index: i64,
        captures: Vec<i64>,
        match_start: usize,
        position: usize,
        points: &[u32],
    ) {
        let mut stack = vec![(index, captures)];
        while let Some((at, tags)) = stack.pop() {
            if at < 0 || seen[at as usize] {
                continue;
            }
            let at = at as usize;
            seen[at] = true;
            let instruction = &self.instructions[at];
            match instruction.op {
                Op::Jump => stack.push((instruction.out, tags)),
                Op::Save => {
                    let mut updated = tags;
                    updated[instruction.slot] = position as i64;
                    stack.push((instruction.out, updated));
                }
                Op::Split => {
                    stack.push((instruction.out1, tags.clone()));
                    stack.push((instruction.out, tags));
                }
                Op::AssertStart => {
                    if position == 0 || (self.multiline && points[position - 1] == '\n' as u32) {
                        stack.push((instruction.out, tags));
                    }
                }
                Op::AssertEnd => {
                    if position == points.len()
                        || (self.multiline && points[position] == '\n' as u32)
                    {
                        stack.push((instruction.out, tags));
                    }
                }
                _ => states.push(Tagged {
                    index: at,
                    captures: tags,
                    match_start,
                }),
            }
        }
    }

    fn append_initial(&self, states: &mut Vec<Tagged>, seen: &mut [bool], position: usize, points: &[u32]) {
        let captures = vec![-1; self.capture_count * 2];
        self.add_tagged(states, seen, self.start as i64, captures, position, position, points);
    }

    fn best_acceptance(&self, states: &[Tagged], position: usize, best: Option<Vec<i64>>) -> Option<Vec<i64>> {
        let mut best = best;
        if let Some(state) = states
            .iter()
            .find(|state| self.instructions[state.index].op == Op::Match)
        {
            let better = match &best {
                None => true,
                Some(best) => {
                    (state.match_start as i64) < best[0]
                        || (state.match_start as i64 == best[0] && position as i64 > best[1])
                }
            };
            if better {
                let mut next = vec![state.match_start as i64, position as i64];
                next.extend(&state.captures);
                best = Some(next);
            }
        }
        best
    }

    fn can_extend(&self, states: &[Tagged], match_start: i64) -> bool {
        states.iter().any(|state| {
            state.match_start as i64 <= match_start
                && self.instructions[state.index].op == Op::Consume
        })
    }

    /// [начало, конец, захваты…] или None.
    fn best_from(&self, points: &[u32], minimum_start: usize) -> Option<Vec<i64>> {
        let length = points.len();
        if minimum_start > length {
            return None;
        }
        let mut states = Vec::new();
        let mut seen = vec![false; self.instructions.len()];
        self.append_initial(&mut states, &mut seen, minimum_start, points);
        let mut best = self.best_acceptance(&states, minimum_start, None);
        if best
            .as_ref()
            .is_some_and(|best| !self.can_extend(&states, best[0]))
        {
            return best;
        }
        for index in minimum_start..length {
            let mut next = Vec::new();
            let mut next_seen = vec![false; self.instructions.len()];
            for state in &states {
                let instruction = &self.instructions[state.index];
                if instruction.op != Op::Consume
                    || !self.matches_character(
                        instruction.matcher.as_ref().expect("consume matcher"),
                        points[index],
                    )
                {
                    continue;
                }
                self.add_tagged(
                    &mut next,
                    &mut next_seen,
                    instruction.out,
                    state.captures.clone(),
                    state.match_start,
                    index + 1,
                    points,
                );
            }
            self.append_initial(&mut next, &mut next_seen, index + 1, points);
            states = next;
            best = self.best_acceptance(&states, index + 1, best);
            if best
                .as_ref()
                .is_some_and(|best| !self.can_extend(&states, best[0]))
            {
                return best;
            }
        }
        best
    }

    /// Совпадение в [start, end) в режиме first/prefix/entire: спаны или пусто.
    fn selected_spans(
        &self,
        input: &str,
        start: &IntValue,
        end: &IntValue,
        mode: i64,
    ) -> Result<Vec<(i64, i64)>, &'static str> {
        let all: Vec<u32> = input.chars().map(u32::from).collect();
        if all.len() > MAX_INPUT {
            return Err("Regex input resource limit exceeded\n");
        }
        let length = IntValue::from_u128(all.len() as u128);
        if start.is_negative()
            || crate::integer::compare(end, start).is_lt()
            || crate::integer::compare(end, &length).is_gt()
        {
            return Err("Invalid regex search range\n");
        }
        let start = start.to_i64().expect("bounded") as usize;
        let end = end.to_i64().expect("bounded") as usize;
        let selected = &all[start..end];
        let Some(best) = self.best_from(selected, 0) else {
            return Ok(Vec::new());
        };
        if (mode == 1 && best[0] != 0)
            || (mode == 2 && (best[0] != 0 || best[1] != selected.len() as i64))
        {
            return Ok(Vec::new());
        }
        let offset = start as i64;
        let mut spans = vec![(best[0] + offset, best[1] + offset)];
        for capture in 0..self.capture_count {
            let from = best[2 + capture * 2];
            let to = best[3 + capture * 2];
            let to = if to < 0 { -1 } else { to + offset };
            spans.push((if from < 0 { -1 } else { from + offset }, to));
        }
        Ok(spans)
    }
}

fn compile_pattern(pattern: &str, flags: &[bool; 5]) -> Compiled<Program> {
    if pattern.chars().count() > MAX_PATTERN {
        return Err(CompileFailure::Limit(0, MAX_PATTERN as i64));
    }
    let mut parser = Parser {
        source: pattern.chars().map(u32::from).collect(),
        index: 0,
        capture_count: 0,
        group_names: BTreeMap::new(),
        nesting: 0,
        verbose: flags[4],
    };
    let tree = parser.parse()?;
    let mut instructions = Vec::new();
    let fragment = compile(&tree, &mut instructions)?;
    let matched = emit(&mut instructions, instruction(Op::Match))?;
    patch(&mut instructions, &fragment.outs, matched);
    Ok(Program {
        instructions,
        start: fragment.start,
        capture_count: parser.capture_count,
        group_names: parser.group_names,
        ascii: flags[0],
        ignore_case: flags[1],
        multiline: flags[2],
        dot_all: flags[3],
    })
}

#[derive(Debug)]
struct Prepared {
    program: Option<std::rc::Rc<Program>>,
    status: i64,
    offset: i64,
    kind: i64,
    limit: i64,
}

#[derive(Debug)]
struct State {
    expression: String,
    flags: [bool; 5],
    prepared: RefCell<Option<std::rc::Rc<Prepared>>>,
}

impl State {
    fn prepare(&self) -> std::rc::Rc<Prepared> {
        if let Some(prepared) = self.prepared.borrow().clone() {
            return prepared;
        }
        let prepared = std::rc::Rc::new(match compile_pattern(&self.expression, &self.flags) {
            Ok(program) => Prepared {
                program: Some(std::rc::Rc::new(program)),
                status: 0,
                offset: -1,
                kind: -1,
                limit: 0,
            },
            Err(CompileFailure::Syntax(offset)) => Prepared {
                program: None,
                status: 1,
                offset,
                kind: -1,
                limit: 0,
            },
            Err(CompileFailure::Limit(kind, limit)) => Prepared {
                program: None,
                status: 2,
                offset: -1,
                kind,
                limit,
            },
        });
        *self.prepared.borrow_mut() = Some(prepared.clone());
        prepared
    }
}

#[derive(Debug)]
struct Query {
    pattern: std::rc::Rc<State>,
    origin: String,
    start: IntValue,
    end: IntValue,
    mode: i64,
    spans: RefCell<Option<Vec<(i64, i64)>>>,
}

#[derive(Debug)]
struct Match {
    spans: Vec<(i64, i64)>,
    names: BTreeMap<String, usize>,
}

#[derive(Debug, Default)]
struct Replacement {
    template: String,
    prepared: RefCell<Option<(i64, i64, Vec<String>)>>,
}

#[derive(Debug, Default)]
struct Substitution {
    stored: RefCell<Option<(String, IntValue)>>,
}

fn invalid_input(native: Native, message: &str) -> Error {
    failure(DOMAIN, "invalid_input", native, message)
}

fn prepare_query(native: Native, query: &Query) -> Result<Vec<(i64, i64)>, Error> {
    if let Some(spans) = query.spans.borrow().clone() {
        return Ok(spans);
    }
    let prepared = query.pattern.prepare();
    let result = match prepared.status {
        2 => Err("Regex pattern resource limit exceeded\n"),
        0 => prepared
            .program
            .as_ref()
            .expect("valid pattern has a program")
            .selected_spans(&query.origin, &query.start, &query.end, query.mode),
        _ => Err("Invalid regex pattern\n"),
    };
    match result {
        Ok(spans) => {
            *query.spans.borrow_mut() = Some(spans.clone());
            Ok(spans)
        }
        Err("Invalid regex pattern\n") => Err(invalid_input(native, "Invalid regex pattern")),
        Err("Regex pattern resource limit exceeded\n" | "Regex input resource limit exceeded\n") => {
            Err(failure(DOMAIN, "resource_exhausted", native, "Regex resource limit exceeded"))
        }
        Err(_) => Err(failure(DOMAIN, "unexpected", native, "Regex provider failed")),
    }
}

fn prepare_replacement(state: &Replacement) -> (i64, i64, Vec<String>) {
    if let Some(prepared) = state.prepared.borrow().clone() {
        return prepared;
    }
    let source: Vec<char> = state.template.chars().collect();
    let mut tokens = Vec::new();
    let mut literal = String::new();
    let mut index = 0;
    let result = 'parse: {
        while index < source.len() {
            if source[index] != '$' {
                literal.push(source[index]);
                index += 1;
                continue;
            }
            let offset = index as i64;
            index += 1;
            if index >= source.len() {
                break 'parse (1, offset, Vec::new());
            }
            if source[index] == '$' {
                literal.push('$');
                index += 1;
                continue;
            }
            if !literal.is_empty() {
                tokens.push("literal".to_string());
                tokens.push(std::mem::take(&mut literal));
            }
            if source[index].is_ascii_digit() {
                let mut digits = String::new();
                while index < source.len() && source[index].is_ascii_digit() {
                    digits.push(source[index]);
                    index += 1;
                }
                tokens.push("index".into());
                tokens.push(digits);
                continue;
            }
            if source[index] == '{' {
                index += 1;
                let mut name = String::new();
                while index < source.len() && source[index] != '}' {
                    name.push(source[index]);
                    index += 1;
                }
                if index >= source.len() || !identifier(&name) {
                    break 'parse (1, offset, Vec::new());
                }
                index += 1;
                tokens.push("name".into());
                tokens.push(name);
                continue;
            }
            break 'parse (1, offset, Vec::new());
        }
        if !literal.is_empty() {
            tokens.push("literal".into());
            tokens.push(literal);
        }
        (0, -1, tokens)
    };
    *state.prepared.borrow_mut() = Some(result.clone());
    result
}

fn escape(origin: &str) -> String {
    let mut escaped = String::new();
    for c in origin.chars() {
        match c {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{0c}' | '\u{0b}' => escaped.push_str(&format!("\\x{{{:x}}}", u32::from(c))),
            c if "\\.^$|()[]{}?*+ #".contains(c) => {
                escaped.push('\\');
                escaped.push(c);
            }
            c => escaped.push(c),
        }
    }
    escaped
}

fn state(value: &Value) -> Result<std::rc::Rc<State>, Error> {
    Ok(handle::<std::rc::Rc<State>>(value)?.clone())
}

fn names_map(names: &BTreeMap<String, usize>) -> Value {
    super::support::map(
        names
            .iter()
            .map(|(name, index)| (name.clone(), int_value(*index as i64))),
    )
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let operation = native.name();
    match operation {
        "regex_state_create" => {
            let mut flags = [false; 5];
            for code in ints(&args[1])? {
                let index = code
                    .to_i64()
                    .filter(|code| (0..=4).contains(code))
                    .ok_or_else(|| invalid_input(native, "Unknown regex flag"))?;
                flags[index as usize] = true;
            }
            Ok(new_handle(std::rc::Rc::new(State {
                expression: string(&args[0])?.to_string(),
                flags,
                prepared: RefCell::new(None),
            })))
        }
        "regex_state_prepare" => {
            let prepared = state(&args[0])?.prepare();
            Ok(int_map([
                ("status", prepared.status),
                ("offset", prepared.offset),
                ("kind", prepared.kind),
                ("limit", prepared.limit),
            ]))
        }
        "regex_state_group_count" | "regex_state_group_names" => {
            let prepared = state(&args[0])?.prepare();
            let Some(program) = prepared.program.as_ref().filter(|_| prepared.status == 0) else {
                return Err(invalid_input(native, "Regex pattern is not valid"));
            };
            if operation == "regex_state_group_count" {
                Ok(int_value(program.capture_count as i64))
            } else {
                Ok(names_map(&program.group_names))
            }
        }
        "regex_query_create" => {
            let mode = int(&args[4])?
                .to_i64()
                .filter(|mode| (0..=2).contains(mode))
                .ok_or_else(|| invalid_input(native, "Unknown regex query mode"))?;
            Ok(new_handle(Query {
                pattern: state(&args[0])?,
                origin: string(&args[1])?.to_string(),
                start: int(&args[2])?.clone(),
                end: int(&args[3])?.clone(),
                mode,
                spans: RefCell::new(None),
            }))
        }
        "regex_query_found" => {
            let query = handle::<Query>(&args[0])?;
            Ok(Value::Bool(!prepare_query(native, query)?.is_empty()))
        }
        "regex_query_match" => {
            let query = handle::<Query>(&args[0])?;
            let spans = prepare_query(native, query)?;
            if spans.is_empty() {
                return Err(failure(DOMAIN, "not_found", native, "No regex match"));
            }
            let names = query
                .pattern
                .prepare()
                .program
                .as_ref()
                .expect("matched pattern has a program")
                .group_names
                .clone();
            Ok(new_handle(Match { spans, names }))
        }
        "regex_match_capture_count" => {
            Ok(int_value(handle::<Match>(&args[0])?.spans.len() as i64 - 1))
        }
        "regex_match_capture_name" | "regex_match_capture_span" => {
            let found = handle::<Match>(&args[0])?;
            let at = super::support::small(int(&args[1])?, found.spans.len())
                .ok_or_else(|| invalid_input(native, "Missing regex capture"))?;
            if operation == "regex_match_capture_name" {
                return Ok(text(
                    found
                        .names
                        .iter()
                        .find(|(_, index)| **index == at)
                        .map_or("", |(name, _)| name.as_str()),
                ));
            }
            let (start, end) = found.spans[at];
            Ok(int_map([("start", start), ("end", end)]))
        }
        "regex_match_capture_index" => {
            let found = handle::<Match>(&args[0])?;
            Ok(int_value(
                found
                    .names
                    .get(string(&args[1])?)
                    .map_or(-1, |index| *index as i64),
            ))
        }
        "regex_replacement_state" => Ok(new_handle(Replacement {
            template: string(&args[0])?.to_string(),
            prepared: RefCell::new(None),
        })),
        "regex_replacement_prepare" => {
            let (status, offset, _) = prepare_replacement(handle::<Replacement>(&args[0])?);
            Ok(int_map([("status", status), ("offset", offset)]))
        }
        "regex_replacement_tokens" => {
            let (status, _, tokens) = prepare_replacement(handle::<Replacement>(&args[0])?);
            if status != 0 {
                return Err(invalid_input(native, "Invalid regex replacement"));
            }
            Ok(string_array(tokens))
        }
        "regex_escape" => Ok(text(escape(string(&args[0])?))),
        "regex_substitution_state" => Ok(new_handle(Substitution::default())),
        "regex_substitution_ready" => Ok(Value::Bool(
            handle::<Substitution>(&args[0])?.stored.borrow().is_some(),
        )),
        "regex_substitution_store" => {
            *handle::<Substitution>(&args[0])?.stored.borrow_mut() =
                Some((string(&args[1])?.to_string(), int(&args[2])?.clone()));
            Ok(Value::Bool(true))
        }
        "regex_substitution_string" | "regex_substitution_amount" => {
            let stored = handle::<Substitution>(&args[0])?.stored.borrow().clone();
            let Some((string, amount)) = stored else {
                return Err(invalid_input(native, "Regex substitution is not ready"));
            };
            Ok(if operation == "regex_substitution_string" {
                text(string)
            } else {
                Value::Int(amount)
            })
        }
        _ => Err(Error::InvalidState(format!("{} is not a regex native", native.symbol()))),
    }
}
