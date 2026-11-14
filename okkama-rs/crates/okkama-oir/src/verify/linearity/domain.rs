use crate::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) const ABSENT: u8 = 1;
pub(super) const AVAILABLE: u8 = 2;
pub(super) const CONSUMED: u8 = 4;
pub(super) const FRESH: u8 = 1 | 32;

/// Origins are static families, including creations inside loops. A journal
/// stores a family once, rather than enumerating its dynamic incarnations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Origin {
    Input(ValueId),
    Instruction(InstId, u32),
    Invoke(BlockId, u32),
    External(ValueId),
    RetiredInstruction(InstId, u32),
    RetiredInvoke(BlockId, u32),
    Projection(ValueId, ProjectionKind),
    RetiredProjection(ValueId, ProjectionKind),
    ViewInstruction(InstId, u16),
    ViewInvoke(BlockId, u16),
    RetiredViewInstruction(InstId, u16),
    RetiredViewInvoke(BlockId, u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ProjectionKind {
    Field(DeclId, u32),
    Path { fields: [(DeclId, u32); 4], len: u8 },
    Index(ValueId),
    Slice(ValueId, ValueId),
    Map(ValueId),
    MapIndex(ValueId, bool),
    Keys,
    Values,
    Remove(ValueId),
}
type Fields = BTreeMap<(Origin, DeclId, u32), (BTreeSet<Origin>, Option<Origin>)>;

/// A finite partition cell. Opposite predicates prove disjointness, including
/// when the contents of an ABI input are not statically enumerable.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Filter {
    pub keys: BTreeSet<Origin>,
    pub fresh: bool,
    pub possible: BTreeSet<Origin>,
    pub labels: BTreeSet<ValueId>,
    pub stale: bool,
    pub state: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Piece {
    pub origin: Origin,
    pub predicates: BTreeMap<Filter, bool>,
    pub provenance: u8,
    pub owner: Option<ScopeId>,
    pub journal: bool,
    pub family: bool,
}
impl Piece {
    pub fn new(origin: Origin, provenance: AdmissionProvenance) -> Self {
        Self {
            origin,
            predicates: BTreeMap::new(),
            provenance: provenance_bit(provenance),
            owner: None,
            journal: false,
            family: false,
        }
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        self.origin == other.origin && self.journal == other.journal && !self.predicates.iter().any(|(key, value)| {
            !key.stale && other.predicates.get(key).is_some_and(|other| value != other)
        })
    }
}
pub(super) fn provenance_bit(provenance: AdmissionProvenance) -> u8 {
    match provenance {
        AdmissionProvenance::UnboundResult => 1,
        AdmissionProvenance::Bound => 2,
        AdmissionProvenance::StagedOrCarrierOwned => 4,
        AdmissionProvenance::CacheOwned => 8,
        AdmissionProvenance::ExtractedAtExistingBoundary => 16,
        AdmissionProvenance::TemporaryAwaitResult => 32,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Token {
    pub pieces: BTreeSet<Piece>,
    pub scope: Option<ScopeId>,
    pub reserve: BTreeSet<ScopeId>,
    pub sources: BTreeSet<ValueId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Flow {
    pub availability: Vec<u8>,
    pub tokens: Vec<Token>,
    pub journals: Vec<Token>,
    pub refs: Vec<BTreeSet<Origin>>,
    pub full_deps: Vec<BTreeSet<Origin>>,
    pub lifetimes: Vec<u8>,
    pub identities: Vec<Option<Origin>>,
    pub dead: BTreeSet<Origin>,
    pub provenance: BTreeMap<Origin, u8>,
    pub scopes: BTreeMap<ScopeId, u8>,
    pub bindings: BTreeSet<BindingId>,
    pub must_use: Vec<u8>,
    pub entry_accepted: bool,
    pub ended: BTreeSet<ScopeId>,
    pub closed_inputs: BTreeSet<Piece>,
    pub failures: Vec<u8>,
    pub active_failure: u8,
    pub fields: Fields,
    pub aliases: Vec<BTreeSet<ValueId>>,
    pub contained: Vec<BTreeSet<ValueId>>,
    pub views: BTreeMap<Origin, BTreeSet<Origin>>,
    pub witnesses: Vec<bool>,
}
impl Flow {
    pub fn new(values: usize) -> Self {
        Self {
            availability: vec![ABSENT; values], tokens: vec![Token::default(); values],
            journals: vec![Token::default(); values],
            refs: vec![BTreeSet::new(); values], full_deps: vec![BTreeSet::new(); values],
            lifetimes: vec![1; values],
            identities: vec![None; values],
            dead: BTreeSet::new(),
            provenance: BTreeMap::new(),
            scopes: BTreeMap::new(), bindings: BTreeSet::new(), must_use: vec![0; values], entry_accepted: true,
            ended: BTreeSet::new(),
            closed_inputs: BTreeSet::new(),
            failures: vec![0; values],
            active_failure: 0,
            fields: BTreeMap::new(),
            aliases: vec![BTreeSet::new(); values],
            contained: vec![BTreeSet::new(); values],
            views: BTreeMap::new(),
            witnesses: vec![true; values],
        }
    }
    /// Only availability is joined independently. All relational components
    /// stay together, so a phi cannot pair one edge's owner with another's packet.
    pub fn same_relations(&self, other: &Self) -> bool {
        self.tokens == other.tokens && self.journals == other.journals
            && self.refs == other.refs && self.full_deps == other.full_deps
            && self.lifetimes == other.lifetimes && self.identities == other.identities
            && self.dead == other.dead
            && self.provenance == other.provenance && self.scopes == other.scopes
            && self.bindings == other.bindings && self.must_use == other.must_use
            && self.entry_accepted == other.entry_accepted && self.ended == other.ended
            && self.closed_inputs == other.closed_inputs && self.failures == other.failures
            && self.active_failure == other.active_failure && self.fields == other.fields
            && self.aliases == other.aliases && self.contained == other.contained && self.views == other.views
    }
    pub fn join_masks(&mut self, other: &Self) -> bool {
        let mut changed = false;
        for (mask, incoming) in self.availability.iter_mut().zip(&other.availability) {
            let next = *mask | incoming;
            changed |= *mask != next;
            *mask = next;
        }
        changed
    }
    pub fn normalize_token(&self, mut token: Token) -> Token {
        token.pieces = token.pieces.into_iter().map(|mut piece| {
            piece.predicates = piece.predicates.into_iter().map(|(mut filter, selected)| {
                filter.labels = filter.labels.iter().flat_map(|value| {
                    self.aliases[value.as_u32() as usize].iter().copied()
                }).collect();
                let current = filter.keys.iter().all(|key| !matches!(key, Origin::RetiredInstruction(..)
                    | Origin::RetiredInvoke(..) | Origin::RetiredProjection(..)
                    | Origin::RetiredViewInstruction(..) | Origin::RetiredViewInvoke(..)));
                if current {
                    for (index, refs) in self.refs.iter().enumerate() {
                        if self.witnesses[index] && !refs.is_empty() && refs.is_subset(&filter.keys) {
                            filter.labels.extend(&self.contained[index]);
                        }
                    }
                }
                (filter, selected)
            }).collect();
            piece
        }).collect();
        token
    }
    pub fn remap_aliases(&mut self, classes: &BTreeMap<BTreeSet<ValueId>, BTreeSet<ValueId>>) {
        let old = self.aliases.clone();
        let rewrite = |piece: &Piece| {
            let mut piece = piece.clone();
            piece.predicates = piece.predicates.into_iter().map(|(mut filter, selected)| {
                let mut labels = BTreeSet::new();
                for label in &filter.labels {
                    if let Some(members) = classes.get(&old[label.as_u32() as usize]) {
                        labels.extend(members);
                        filter.stale |= members.is_empty();
                    }
                }
                filter.labels = labels;
                (filter, selected)
            }).collect();
            piece
        };
        for token in self.tokens.iter_mut().chain(&mut self.journals) {
            token.pieces = token.pieces.iter().map(rewrite).collect();
        }
        self.closed_inputs = self.closed_inputs.iter().map(rewrite).collect();
        for contained in &mut self.contained {
            *contained = contained.iter().flat_map(|label| {
                classes.get(&old[label.as_u32() as usize]).into_iter().flatten().copied()
            }).collect();
        }
        self.aliases.fill(BTreeSet::new());
        for members in classes.values() {
            for member in members {
                self.aliases[member.as_u32() as usize] = members.clone();
            }
        }
    }
    pub fn add_contained(&mut self, roots: &BTreeSet<ValueId>, child: &BTreeSet<ValueId>) {
        for contained in &mut self.contained {
            if !contained.is_disjoint(roots) {
                contained.extend(child);
            }
        }
        let add = |piece: &Piece| {
            let mut piece = piece.clone();
            piece.predicates = piece.predicates.into_iter().map(|(mut filter, selected)| {
                if !filter.labels.is_disjoint(roots) {
                    filter.labels.extend(child);
                }
                (filter, selected)
            }).collect();
            piece
        };
        for token in self.tokens.iter_mut().chain(&mut self.journals) {
            token.pieces = token.pieces.iter().map(add).collect();
        }
        self.closed_inputs = self.closed_inputs.iter().map(add).collect();
    }
    pub fn replace_aliases(&mut self, old: &BTreeSet<ValueId>, new: &BTreeSet<ValueId>) {
        let rewrite = |piece: &Piece| {
            let mut piece = piece.clone();
            piece.predicates = piece.predicates.into_iter().map(|(mut filter, selected)| {
                if !filter.labels.is_disjoint(old) {
                    filter.labels = filter.labels.difference(old).copied().chain(new.iter().copied()).collect();
                    filter.stale |= new.is_empty();
                }
                (filter, selected)
            }).collect();
            piece
        };
        for token in self.tokens.iter_mut().chain(&mut self.journals) {
            token.pieces = token.pieces.iter().map(rewrite).collect();
        }
        self.closed_inputs = self.closed_inputs.iter().map(rewrite).collect();
        for contained in &mut self.contained {
            if !contained.is_disjoint(old) {
                *contained = contained.difference(old).copied().chain(new.iter().copied()).collect();
            }
        }
        for alias in old.union(new) {
            self.aliases[alias.as_u32() as usize] = new.clone();
        }
    }
    pub fn projection_keys(&self) -> BTreeSet<Origin> {
        let mut keys = BTreeSet::new();
        for refs in self.refs.iter().chain(&self.full_deps).chain(self.views.values()) {
            keys.extend(refs.iter().filter(|key| matches!(key, Origin::Projection(..))).copied());
        }
        keys.extend(self.views.keys().filter(|key| matches!(key, Origin::Projection(..))).copied());
        for token in self.tokens.iter().chain(&self.journals) {
            for piece in &token.pieces {
                for filter in piece.predicates.keys() {
                    keys.extend(filter.keys.iter().chain(&filter.possible)
                        .filter(|key| matches!(key, Origin::Projection(..))).copied());
                }
            }
        }
        keys
    }
    pub fn retire_origin(&mut self, origin: Origin) {
        let retired = match origin {
            Origin::Instruction(inst, slot) => Origin::RetiredInstruction(inst, slot),
            Origin::Invoke(block, slot) => Origin::RetiredInvoke(block, slot),
            Origin::Projection(value, kind) => Origin::RetiredProjection(value, kind),
            Origin::ViewInstruction(inst, slot) => Origin::RetiredViewInstruction(inst, slot),
            Origin::ViewInvoke(block, slot) => Origin::RetiredViewInvoke(block, slot),
            _ => return,
        };
        for key in self.projection_keys() {
            if let Origin::Projection(value, _) = key {
                self.views.entry(key).or_insert_with(|| self.refs[value.as_u32() as usize].clone());
            }
        }
        let identities = self.identities.clone();
        let rename = |key: Origin| {
            if key == origin {
                return retired;
            }
            if let Origin::Projection(value, kind) = key
                && identities.get(value.as_u32() as usize) == Some(&Some(origin)) {
                return Origin::RetiredProjection(value, kind);
            }
            key
        };
        for refs in self.refs.iter_mut().chain(&mut self.full_deps) {
            *refs = refs.iter().copied().map(rename).collect();
        }
        for identity in &mut self.identities {
            *identity = identity.map(rename);
        }
        for token in self.tokens.iter_mut().chain(&mut self.journals) {
            token.pieces = token.pieces.iter().cloned().map(|mut piece| {
                let changed = piece.origin != rename(piece.origin);
                piece.origin = rename(piece.origin);
                piece.family |= changed;
                piece.predicates = piece.predicates.into_iter().map(|(mut filter, selected)| {
                    filter.keys = filter.keys.into_iter().map(rename).collect();
                    filter.possible = filter.possible.into_iter().map(rename).collect();
                    (filter, selected)
                }).collect();
                piece
            }).collect();
        }
        self.closed_inputs = self.closed_inputs.iter().cloned().map(|mut piece| {
            piece.origin = rename(piece.origin);
            piece.predicates = piece.predicates.into_iter().map(|(mut filter, selected)| {
                filter.keys = filter.keys.into_iter().map(rename).collect();
                filter.possible = filter.possible.into_iter().map(rename).collect();
                (filter, selected)
            }).collect();
            piece
        }).collect();
        let mut views: BTreeMap<Origin, BTreeSet<Origin>> = BTreeMap::new();
        for (key, refs) in &self.views {
            views.entry(rename(*key)).or_default().extend(refs.iter().copied().map(rename));
        }
        self.views = views;
        let mut fields: Fields = BTreeMap::new();
        for ((key, declaration, index), (refs, identity)) in &self.fields {
            let entry = fields.entry((rename(*key), *declaration, *index))
                .or_insert_with(|| (BTreeSet::new(), identity.map(rename)));
            entry.0.extend(refs.iter().copied().map(rename));
            if entry.1 != identity.map(rename) {
                entry.1 = None;
            }
        }
        self.fields = fields;
        self.dead = self.dead.iter().copied().map(rename).collect();
        if self.dead.remove(&origin) {
            self.dead.insert(retired);
        }
        if let Some(provenance) = self.provenance.remove(&origin) {
            self.provenance.entry(retired).and_modify(|old| *old |= provenance).or_insert(provenance);
        }
    }
    pub fn retire(&mut self, value: ValueId) {
        let index = value.as_u32() as usize;
        self.availability[index] = CONSUMED;
        self.tokens[index] = Token::default();
        self.journals[index] = Token::default();
    }
}

pub(super) fn failure_bit(kind: FailureKind) -> u8 {
    match kind {
        FailureKind::Thrown => 1,
        FailureKind::Program => 2,
        FailureKind::Panic => 4,
        FailureKind::Native => 8,
        FailureKind::Cancelled => 16,
    }
}

pub(super) fn creation(origin: Origin, site: CreationSiteId) -> Origin {
    match origin {
        Origin::Instruction(inst, _) => Origin::Instruction(inst, site.as_u32()),
        Origin::Invoke(block, _) => Origin::Invoke(block, site.as_u32()),
        _ => origin,
    }
}

pub(super) fn view_origin(origin: Origin, slot: usize) -> Origin {
    match origin {
        Origin::Instruction(inst, _) => Origin::ViewInstruction(inst, slot as u16),
        Origin::Invoke(block, _) => Origin::ViewInvoke(block, slot as u16),
        _ => origin,
    }
}
