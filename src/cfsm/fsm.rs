use crate::abstraction::protocol::{
    Always, Block, Communication, Connect, Extension, ForkJoin, Loop, MultiArmedIfElse, Protocol,
    Update,
};
use crate::abstraction::sv_info::{
    merge_with_and, BoolExpression, Channel, ModuleInfo, ModuleInstance, Var,
};
use crate::cfsm::optimization::linearize_fork_join;
use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter, Result};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct BlankNode {
    id: usize,
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

impl BlankNode {
    pub fn new() -> BlankNode {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        BlankNode { id }
    }
}

impl Display for BlankNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.id)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct EdgeInfo {
    pub communication: Option<Communication>,
    pub guard: Option<BoolExpression>,
    pub updates: Vec<Update>,
}

impl EdgeInfo {
    pub fn is_jump(&self) -> bool {
        self.communication.is_none() && self.guard.is_none() && self.updates.is_empty()
    }

    pub fn describe(&self) -> String {
        let mut description = String::new();
        if let Some(g) = &self.guard {
            description.push_str(&format!("[{}]", g));
        }
        if let Some(c) = &self.communication {
            description.push_str(&format!("{}", c));
        }
        if !&self.updates.is_empty() {
            let updates = &self
                .updates
                .iter()
                .map(|u| u.to_string())
                .collect::<Vec<String>>()
                .join(", ");
            description.push_str(&format!("{{{}}}\n", updates));
        }
        description
    }

    pub fn rebind_channel(&self, map: &HashMap<Var, Channel>) -> EdgeInfo {
        let EdgeInfo {
            communication,
            guard,
            updates,
        } = self;
        let communication = match communication {
            None => None,
            Some(c) => Some(c.rebind_channel(map)),
        };
        EdgeInfo {
            communication,
            guard: guard.clone(),
            updates: updates.clone(),
        }
    }
}

impl Display for EdgeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.describe())
    }
}

fn jump() -> EdgeInfo {
    EdgeInfo {
        communication: None,
        guard: None,
        updates: vec![],
    }
}

pub type FSM = DiGraph<BlankNode, EdgeInfo>;

// an edge without substantial information
pub fn find_pure_connection_edge(fsm: &FSM) -> Option<EdgeIndex> {
    fsm.edge_indices().find(|eid| {
        let (s, t) = fsm.edge_endpoints(*eid).unwrap();
        let source_out_degree = fsm.edges_directed(s, petgraph::Direction::Outgoing).count();
        let target_in_degree = fsm.edges_directed(t, petgraph::Direction::Incoming).count();
        fsm[*eid].is_jump() && (source_out_degree == 1 || target_in_degree == 1)
    })
}

#[derive(Debug, Clone)]
pub struct CFSM {
    pub module: ModuleInfo,
    pub initial: NodeIndex,
    pub finals: HashSet<NodeIndex>,
    pub fsm: FSM,
}

#[derive(Debug, Clone)]
pub struct AnonymousCFSM {
    pub initial: NodeIndex,
    pub finals: HashSet<NodeIndex>,
    pub fsm: FSM,
}

impl AnonymousCFSM {
    // simplify one edge every time
    fn simplify(mut self) -> Self {
        let AnonymousCFSM {
            initial,
            finals: _,
            fsm,
        } = &self;
        if let Some(to_simplify) = find_pure_connection_edge(fsm) {
            let (s, t) = fsm.edge_endpoints(to_simplify).unwrap();
            let (substituted, substitution) = if s == *initial { (t, s) } else { (s, t) };
            let to_update = fsm
                .edge_indices()
                .filter_map(|e_id| {
                    if e_id == to_simplify {
                        None
                    } else {
                        let (s, t) = fsm.edge_endpoints(e_id).unwrap();
                        if s == substituted || t == substituted {
                            let info = fsm.edge_weight(e_id).unwrap().clone();
                            Some((e_id, s, t, info))
                        } else {
                            None
                        }
                    }
                })
                .collect::<Vec<(EdgeIndex, NodeIndex, NodeIndex, EdgeInfo)>>();

            // update the affected edges
            for (_, s_id, t_id, info) in to_update.as_slice() {
                let new_s_id = if *s_id == substituted {
                    substitution
                } else {
                    *s_id
                };
                let new_t_id = if *t_id == substituted {
                    substitution
                } else {
                    *t_id
                };
                self.fsm.add_edge(new_s_id, new_t_id, info.clone());
            }
            // remove the updated edges after the update, otherwise the edge indices will be invalid
            for (e_id, _, _, _) in to_update {
                self.fsm.remove_edge(e_id);
            }
            // remove the pure connection edge
            self.fsm.remove_edge(to_simplify);
            // remove substituted node
            self.fsm.remove_node(substituted);
            self.simplify()
        } else {
            self.finals.clear();
            self
        }
    }
}

fn add_old_fsm_to_new_fsm(
    old_fsm: &FSM,
    new_fsm: &mut FSM,
    old_new_map: &mut HashMap<NodeIndex, NodeIndex>,
) {
    for nid in old_fsm.node_indices() {
        let new_nid = new_fsm.add_node(old_fsm[nid].clone());
        old_new_map.insert(nid, new_nid);
    }
    for eid in old_fsm.edge_indices() {
        let (s, t) = old_fsm.edge_endpoints(eid).unwrap();
        new_fsm.add_edge(old_new_map[&s], old_new_map[&t], old_fsm[eid].clone());
    }
}

fn link(a: AnonymousCFSM, b: AnonymousCFSM) -> AnonymousCFSM {
    let mut linked = FSM::new();
    let mut old_new_map_a: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    add_old_fsm_to_new_fsm(&a.fsm, &mut linked, &mut old_new_map_a);
    let mut old_new_map_b: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    add_old_fsm_to_new_fsm(&b.fsm, &mut linked, &mut old_new_map_b);
    let a_initial = old_new_map_a[&a.initial];
    let b_initial = old_new_map_b[&b.initial];
    for a_final in a.finals {
        linked.add_edge(old_new_map_a[&a_final], b_initial, jump());
    }
    let b_finals = b
        .finals
        .iter()
        .map(|f| old_new_map_b[f])
        .collect::<HashSet<NodeIndex>>();
    AnonymousCFSM {
        initial: a_initial,
        finals: b_finals,
        fsm: linked,
    }
}

fn sequence_cfsm(fsms: Vec<AnonymousCFSM>) -> AnonymousCFSM {
    match fsms.len() {
        0 => from_unit(),
        1 => fsms[0].clone(),
        _ => fsms[1..]
            .iter()
            .cloned()
            .fold(fsms[0].clone(), |acc, fsm| link(acc, fsm)),
    }
}

pub fn construct_cfsm_from_module_instance(
    info: &ModuleInfo,
    instance: &ModuleInstance,
    protocol: Protocol,
    connections: &Vec<Connect>,
) -> CFSM {
    let AnonymousCFSM {
        initial,
        finals,
        fsm,
    } = construct_cfsm_from_protocol(protocol, instance, connections).simplify();
    CFSM {
        module: info.clone(),
        initial,
        finals,
        fsm,
    }
}

fn construct_cfsm_from_protocol(
    protocol: Protocol,
    instance: &ModuleInstance,
    connections: &Vec<Connect>,
) -> AnonymousCFSM {
    match protocol {
        Protocol::Unit => from_unit(),
        Protocol::Always(a) => from_always(*a, instance, connections),
        Protocol::Block(b) => from_block(*b, instance, connections),
        Protocol::Communication(c) => from_communication(c),
        Protocol::Extension(e) => from_extension(e),
        Protocol::ForkJoin(fj) => from_fork_join(
            linearize_fork_join(instance, &fj, connections),
            instance,
            connections,
        ),
        Protocol::MultiArmsIfElse(maie) => from_multi_arms_if_else(*maie, instance, connections),
        Protocol::Update(u) => from_update(u),
        Protocol::Loop(l) => from_loop(*l, instance, connections),
    }
}

fn from_unit() -> AnonymousCFSM {
    let s = BlankNode::new();
    let t = BlankNode::new();
    let mut fsm = FSM::new();
    let sid = fsm.add_node(s);
    let tid = fsm.add_node(t);
    fsm.add_edge(sid, tid, jump());
    AnonymousCFSM {
        initial: sid,
        finals: HashSet::from([tid]),
        fsm,
    }
}

fn from_always(a: Always, instance: &ModuleInstance, connections: &Vec<Connect>) -> AnonymousCFSM {
    let mut cfsm = sequence_cfsm(
        a.block
            .into_iter()
            .map(|p| construct_cfsm_from_protocol(p, instance, connections))
            .collect(),
    );
    for f in cfsm.finals.clone() {
        cfsm.fsm.add_edge(f, cfsm.initial, jump());
    }
    cfsm
}

fn from_block(b: Block, instance: &ModuleInstance, connections: &Vec<Connect>) -> AnonymousCFSM {
    sequence_cfsm(
        b.protocols
            .into_iter()
            .map(|p| construct_cfsm_from_protocol(p, instance, connections))
            .collect(),
    )
}

fn from_communication(c: Communication) -> AnonymousCFSM {
    let s = BlankNode::new();
    let t = BlankNode::new();
    let mut fsm = FSM::new();
    let sid = fsm.add_node(s);
    let tid = fsm.add_node(t);
    fsm.add_edge(
        sid,
        tid,
        EdgeInfo {
            communication: Some(c),
            guard: None,
            updates: vec![],
        },
    );
    AnonymousCFSM {
        initial: sid,
        finals: HashSet::from([tid]),
        fsm,
    }
}

fn from_extension(e: Extension) -> AnonymousCFSM {
    let s = BlankNode::new();
    let t = BlankNode::new();
    let mut fsm = FSM::new();
    let sid = fsm.add_node(s);
    let tid = fsm.add_node(t);
    fsm.add_edge(
        sid,
        tid,
        EdgeInfo {
            communication: None,
            guard: Some(e),
            updates: vec![],
        },
    );
    AnonymousCFSM {
        initial: sid,
        finals: HashSet::from([tid]),
        fsm,
    }
}

fn from_fork_join(
    fj: ForkJoin,
    instance: &ModuleInstance,
    connections: &Vec<Connect>,
) -> AnonymousCFSM {
    let mut fsm = FSM::new();
    let initial = BlankNode::new();
    let initial_id = fsm.add_node(initial);
    let cfsms: Vec<AnonymousCFSM> = permutation_and_wrap(&fj.block)
        .into_iter()
        .map(|p| from_block(p, instance, connections))
        .collect();
    let mut finals = HashSet::new();
    for cfsm in cfsms {
        let new_final_ = BlankNode::new();
        let new_final_id = fsm.add_node(new_final_);
        finals.insert(new_final_id);
        let mut map = HashMap::<NodeIndex, NodeIndex>::new();
        add_old_fsm_to_new_fsm(&cfsm.fsm, &mut fsm, &mut map);
        fsm.add_edge(initial_id, map[&cfsm.initial], jump());
        cfsm.finals.iter().for_each(|f| {
            fsm.add_edge(map[f], new_final_id, jump());
        });
    }
    AnonymousCFSM {
        initial: initial_id,
        finals,
        fsm,
    }
}

fn permutation_and_wrap(protocols: &Vec<Protocol>) -> Vec<Block> {
    permutation(protocols)
        .into_iter()
        .map(|s| Block { protocols: s })
        .collect()
}

fn permutation(protocols: &Vec<Protocol>) -> Vec<Vec<Protocol>> {
    if protocols.is_empty() {
        vec![vec![]]
    } else {
        let mut permutations = Vec::new();
        for i in 0..protocols.len() {
            let mut sessions = protocols.clone();
            let first = sessions.remove(i);
            let mut sub_permutations = permutation(&sessions);
            for sub_permutation in &mut sub_permutations {
                sub_permutation.insert(0, first.clone());
            }
            permutations.append(&mut sub_permutations);
        }
        permutations
    }
}

fn from_multi_arms_if_else(
    maie: MultiArmedIfElse,
    instance: &ModuleInstance,
    connections: &Vec<Connect>,
) -> AnonymousCFSM {
    let mut fsm = FSM::new();
    let initial = BlankNode::new();
    let initial_id = fsm.add_node(initial);
    let mut finals = HashSet::<NodeIndex>::new();
    let mut else_condition = BoolExpression::True;
    for c in maie.conditionals {
        else_condition = merge_with_and(
            else_condition,
            BoolExpression::Not(Box::new(c.condition.clone())),
        );
        let cfsm = construct_cfsm_from_protocol(c.protocol, instance, connections);
        let new_final_ = BlankNode::new();
        let new_final_id = fsm.add_node(new_final_);
        finals.insert(new_final_id);
        let mut map = HashMap::<NodeIndex, NodeIndex>::new();
        add_old_fsm_to_new_fsm(&cfsm.fsm, &mut fsm, &mut map);
        fsm.add_edge(
            initial_id,
            map[&cfsm.initial],
            EdgeInfo {
                communication: None,
                guard: Some(c.condition),
                updates: vec![],
            },
        );
        cfsm.finals.iter().for_each(|f| {
            fsm.add_edge(map[f], new_final_id, jump());
        });
    }
    // deal with else block
    let cfsm = if let Some(p) = maie.else_block {
        construct_cfsm_from_protocol(p, instance, connections)
    } else {
        from_unit()
    };
    let new_final_ = BlankNode::new();
    let new_final_id = fsm.add_node(new_final_);
    finals.insert(new_final_id);
    let mut map = HashMap::<NodeIndex, NodeIndex>::new();
    add_old_fsm_to_new_fsm(&cfsm.fsm, &mut fsm, &mut map);
    fsm.add_edge(
        initial_id,
        map[&cfsm.initial],
        EdgeInfo {
            communication: None,
            guard: Some(else_condition),
            updates: vec![],
        },
    );
    cfsm.finals.iter().for_each(|f| {
        fsm.add_edge(map[f], new_final_id, jump());
    });
    AnonymousCFSM {
        initial: initial_id,
        finals,
        fsm,
    }
}

fn from_update(u: Update) -> AnonymousCFSM {
    let s = BlankNode::new();
    let t = BlankNode::new();
    let mut fsm = FSM::new();
    let sid = fsm.add_node(s);
    let tid = fsm.add_node(t);
    fsm.add_edge(
        sid,
        tid,
        EdgeInfo {
            communication: None,
            guard: None,
            updates: vec![u],
        },
    );
    AnonymousCFSM {
        initial: sid,
        finals: HashSet::from([tid]),
        fsm,
    }
}

fn from_loop(l: Loop, instance: &ModuleInstance, connections: &Vec<Connect>) -> AnonymousCFSM {
    let mut cfsm = construct_cfsm_from_protocol(l.protocol, instance, connections);
    let initial = BlankNode::new();
    let initial_id = cfsm.fsm.add_node(initial);
    let final_ = BlankNode::new();
    let final_id = cfsm.fsm.add_node(final_);
    cfsm.finals.into_iter().for_each(|f| {
        cfsm.fsm.add_edge(f, initial_id, jump());
    });
    cfsm.fsm.add_edge(
        initial_id,
        cfsm.initial,
        EdgeInfo {
            communication: None,
            guard: Some(l.condition.clone()),
            updates: vec![],
        },
    );
    cfsm.fsm.add_edge(
        initial_id,
        final_id,
        EdgeInfo {
            communication: None,
            guard: Some(BoolExpression::Not(Box::new(l.condition))),
            updates: vec![],
        },
    );
    AnonymousCFSM {
        initial: initial_id,
        finals: HashSet::from([final_id]),
        fsm: cfsm.fsm,
    }
}
