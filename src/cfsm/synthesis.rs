use crate::abstraction::protocol::Communication;
use crate::abstraction::sv_info::{BoolExpression, Channel, ModuleInfo, ModuleInstance};
use crate::cfsm::env::Environment;
use crate::cfsm::fsm::{AnonymousCFSM, BlankNode, EdgeInfo, CFSM, FSM};
use crate::error::{Action, DanglingReceiving, DanglingSending, LiveLock, VerilockError};
use petgraph::graph::{EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet, VecDeque};
use z3::Solver;

type LocalStep = (ModuleInstance, NodeIndex, EdgeIndex);

#[derive(Debug, PartialEq, Clone)]
enum SynthesisStep {
    Jump(Jump),
    External(External),
    Match(Match),
}

#[derive(Debug, PartialEq, Clone)]
struct Jump {
    instance: ModuleInstance,
    source_id: NodeIndex,
    edge_id: EdgeIndex,
}

#[derive(Debug, PartialEq, Clone)]
struct External {
    instance: ModuleInstance,
    source_id: NodeIndex,
    edge_id: EdgeIndex,
}

#[derive(Debug, PartialEq, Clone)]
struct Match {
    send_instance: ModuleInstance,
    send_source: NodeIndex,
    send_edge: EdgeIndex,
    recv_instance: ModuleInstance,
    recv_source: NodeIndex,
    recv_edge: EdgeIndex,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
struct GlobalConfiguration {
    node: BlankNode,
    env: Environment,
}

// module instance -> local CFSM node index
type LocalConfigurations = HashMap<ModuleInstance, NodeIndex>;

pub type Group = HashMap<ModuleInstance, CFSM>;

#[derive(Debug, PartialEq, Clone)]
struct SynthesisState {
    local_configurations: LocalConfigurations,
    current_env: Environment,
    error_trace: Vec<Action>,
}

pub fn synthesize(
    group: Group,
    parent: ModuleInfo,
    solver: &Solver,
) -> Result<CFSM, VerilockError> {
    let mut local_nodes_to_global_node = HashMap::<Vec<NodeIndex>, BlankNode>::new();
    let initial_local_nodes = initial_nodes(&group);
    let initial_global_node =
        retrieve_or_construct_node(&mut local_nodes_to_global_node, &initial_local_nodes);
    let empty_env = Environment::new();
    let initial_global_config = GlobalConfiguration {
        node: initial_global_node,
        env: empty_env.clone(),
    };
    let mut visited_global_configs = HashSet::new();
    visited_global_configs.insert(initial_global_config);
    let local_configurations = group
        .iter()
        .map(|(instance, cfsm)| (instance.clone(), cfsm.initial))
        .collect::<HashMap<ModuleInstance, NodeIndex>>();
    start_synthesizing_fsm(
        SynthesisState {
            local_configurations,
            current_env: empty_env,
            error_trace: Vec::new(),
        },
        &mut local_nodes_to_global_node,
        &group,
        solver,
    )
    .map(|anonymous_fsm| CFSM {
        module: parent,
        initial: anonymous_fsm.initial,
        finals: anonymous_fsm.finals,
        fsm: anonymous_fsm.fsm,
    })
}

fn start_synthesizing_fsm(
    initial_synthesis_state: SynthesisState,
    local_nodes_to_global_node: &mut HashMap<Vec<NodeIndex>, BlankNode>,
    group: &Group,
    solver: &Solver,
) -> Result<AnonymousCFSM, VerilockError> {
    let mut used_edges = HashSet::<(ModuleInstance, EdgeIndex)>::new();
    let mut fsm = FSM::new();
    let mut initial: Option<NodeIndex> = None;
    let mut initial_node: Option<BlankNode> = None;
    let mut visited_global_configs = HashSet::new();
    let mut synthesis_queue = VecDeque::new();
    visited_global_configs.insert(synthesis_state_to_config(
        &initial_synthesis_state,
        local_nodes_to_global_node,
    ));
    synthesis_queue.push_back(initial_synthesis_state);
    let mut node_cache = HashMap::<BlankNode, NodeIndex>::new();
    while let Some(synthesis_state) = synthesis_queue.pop_front() {
        let SynthesisState {
            local_configurations,
            current_env,
            error_trace,
        } = synthesis_state;
        let source_node = retrieve_or_construct_node(
            local_nodes_to_global_node,
            &local_configurations.values().cloned().collect(),
        );
        let source_id = find_index_by_weight_or_insert_node(&mut fsm, &mut node_cache, source_node.clone());
        if initial.is_none() {
            initial = Some(source_id);
            initial_node = Some(source_node);
        }
        let synthesis_steps = generate_all_possible_synthesis_steps(
            &local_configurations,
            &current_env,
            solver,
            group,
            &error_trace,
        )?;
        for step in synthesis_steps {
            record_used_edges(&mut used_edges, &step);
            let next_configurations =
                next_local_configurations(group, &local_configurations, &step);
            let next_node = retrieve_or_construct_node(
                local_nodes_to_global_node,
                &next_configurations.values().cloned().collect(),
            );
            let target_id = find_index_by_weight_or_insert_node(&mut fsm, &mut node_cache, next_node);
            let edge = step_to_edge_info(group, &step);
            let next_env = modify_environment_by_edge(&edge, &current_env);
            fsm.add_edge(source_id, target_id, edge);
            let next_error_trace = record_error_trace(&error_trace, &step, group);
            let next_synthesis_state = SynthesisState {
                local_configurations: next_configurations,
                current_env: next_env,
                error_trace: next_error_trace,
            };
            let next_global_config =
                synthesis_state_to_config(&next_synthesis_state, local_nodes_to_global_node);
            // there are two situations that next synthesis state will not be explored:
            // 1. if the state has been visited;
            // 2. if all the CFSMs in the group are back to their initial states.
            if !visited_global_configs.contains(&next_global_config)
                && !return_to_initial_state(&initial_node, &next_global_config.node)
            {
                visited_global_configs.insert(next_global_config);
                synthesis_queue.push_back(next_synthesis_state);
            }
        }
    }
    if let Some(instance) = check_live_locked(group, &used_edges) {
        return Err(VerilockError::LiveLock(LiveLock { module: instance }));
    }
    Ok(AnonymousCFSM {
        initial: initial.expect("missing initial node when synthesizing CFSMs"),
        finals: HashSet::new(),
        fsm,
    })
}

fn return_to_initial_state(initial: &Option<BlankNode>, next_global_node: &BlankNode) -> bool {
    initial.is_some() && initial.as_ref().unwrap() == next_global_node
}

fn find_index_by_weight_or_insert_node(
    fsm: &mut FSM,
    node_cache: &mut HashMap<BlankNode, NodeIndex>,
    node_weight: BlankNode
) -> NodeIndex {
    if node_cache.contains_key(&node_weight) {
        node_cache.get(&node_weight).unwrap().clone()
    } else {
        let id = fsm.add_node(node_weight.clone());
        node_cache.insert(node_weight, id);
        id
    }
}

fn check_live_locked(
    group: &Group,
    used: &HashSet<(ModuleInstance, EdgeIndex)>,
) -> Option<ModuleInstance> {
    for (instance, cfsm) in group {
        let edges: HashSet<(ModuleInstance, EdgeIndex)> = cfsm
            .fsm
            .edge_indices()
            .into_iter()
            .map(|e| (instance.clone(), e))
            .collect();
        // if a CFSM is not empty and its edges are not used in the synthesis CFSM
        if !edges.is_empty() && edges.intersection(used).count() == 0 {
            return Some(instance.clone());
        }
    }
    None
}

fn record_error_trace(old_trace: &Vec<Action>, step: &SynthesisStep, group: &Group) -> Vec<Action> {
    let mut trace = old_trace.clone();
    match step {
        SynthesisStep::Jump(j) => {
            trace.push(construct_action_description(
                &j.instance,
                j.edge_id.clone(),
                &group,
            ));
        }
        SynthesisStep::External(e) => {
            trace.push(construct_action_description(
                &e.instance,
                e.edge_id.clone(),
                &group,
            ));
        }
        SynthesisStep::Match(m) => {
            trace.push(construct_action_description(
                &m.send_instance,
                m.send_edge.clone(),
                &group,
            ));
            trace.push(construct_action_description(
                &m.recv_instance,
                m.recv_edge.clone(),
                &group,
            ));
        }
    };
    trace
}

fn synthesis_state_to_config(
    synthesis_state: &SynthesisState,
    local_nodes_to_global_node: &mut HashMap<Vec<NodeIndex>, BlankNode>,
) -> GlobalConfiguration {
    GlobalConfiguration {
        node: retrieve_or_construct_node(
            local_nodes_to_global_node,
            &synthesis_state
                .local_configurations
                .values()
                .cloned()
                .collect(),
        ),
        env: synthesis_state.current_env.clone(),
    }
}

fn modify_environment_by_edge(edge_info: &EdgeInfo, current_env: &Environment) -> Environment {
    let mut next_env = current_env.clone();
    if let Some(g) = &edge_info.guard {
        next_env = next_env.extend(g);
    }
    for u in &edge_info.updates {
        next_env = next_env.update(u);
    }
    next_env
}

fn step_to_edge_info(group: &Group, step: &SynthesisStep) -> EdgeInfo {
    match step {
        SynthesisStep::Jump(j) => group
            .get(&j.instance)
            .unwrap()
            .fsm
            .edge_weight(j.edge_id)
            .unwrap()
            .clone(),
        SynthesisStep::External(e) => group
            .get(&e.instance)
            .unwrap()
            .fsm
            .edge_weight(e.edge_id)
            .unwrap()
            .clone(),
        SynthesisStep::Match(m) => {
            let s_edge = group
                .get(&m.send_instance)
                .unwrap()
                .fsm
                .edge_weight(m.send_edge)
                .unwrap();
            let r_edge = group
                .get(&m.recv_instance)
                .unwrap()
                .fsm
                .edge_weight(m.recv_edge)
                .unwrap();
            let merged_guard = merge_guard(s_edge.guard.clone(), r_edge.guard.clone());
            let mut merged_updates = s_edge.updates.clone();
            merged_updates.extend(r_edge.updates.clone());
            EdgeInfo {
                communication: None,
                guard: merged_guard,
                updates: merged_updates,
            }
        }
    }
}

fn merge_guard(
    s_guard: Option<BoolExpression>,
    r_guard: Option<BoolExpression>,
) -> Option<BoolExpression> {
    if s_guard.is_some() && r_guard.is_some() {
        let s_guard = s_guard.unwrap();
        let r_guard = r_guard.unwrap();
        Some(BoolExpression::And(Box::new(s_guard), Box::new(r_guard)))
    } else if s_guard.is_some() {
        s_guard
    } else {
        r_guard
    }
}

fn record_used_edges(used_edges: &mut HashSet<(ModuleInstance, EdgeIndex)>, step: &SynthesisStep) {
    match step {
        SynthesisStep::Jump(j) => {
            used_edges.insert((j.instance.clone(), j.edge_id));
        }
        SynthesisStep::External(e) => {
            used_edges.insert((e.instance.clone(), e.edge_id));
        }
        SynthesisStep::Match(m) => {
            used_edges.insert((m.send_instance.clone(), m.send_edge));
            used_edges.insert((m.recv_instance.clone(), m.recv_edge));
        }
    }
}

fn next_local_configurations(
    group: &Group,
    current: &LocalConfigurations,
    step: &SynthesisStep,
) -> LocalConfigurations {
    let mut next = current.clone();
    match step {
        SynthesisStep::Jump(j) => {
            next.insert(
                j.instance.clone(),
                retrieve_next_node(group, &j.instance, j.edge_id),
            );
        }
        SynthesisStep::External(e) => {
            next.insert(
                e.instance.clone(),
                retrieve_next_node(group, &e.instance, e.edge_id),
            );
        }
        SynthesisStep::Match(m) => {
            next.insert(
                m.send_instance.clone(),
                retrieve_next_node(group, &m.send_instance, m.send_edge),
            );
            next.insert(
                m.recv_instance.clone(),
                retrieve_next_node(group, &m.recv_instance, m.recv_edge),
            );
        }
    }
    next
}

fn retrieve_next_node(group: &Group, instance: &ModuleInstance, edge_id: EdgeIndex) -> NodeIndex {
    let (_, t) = &group
        .get(instance)
        .unwrap()
        .fsm
        .edge_endpoints(edge_id)
        .unwrap();
    t.clone()
}

fn generate_all_possible_synthesis_steps(
    local_configurations: &LocalConfigurations,
    current_env: &Environment,
    solver: &Solver,
    group: &Group,
    error_trace: &Vec<Action>,
) -> Result<Vec<SynthesisStep>, VerilockError> {
    let mut synthesis_steps = Vec::new();
    let (jumps, externals, sendings, receivings) =
        all_possible_local_steps(local_configurations, group, current_env, solver);
    for (cfsm_name, source_id, edge_id) in jumps
    {
        synthesis_steps.push(SynthesisStep::Jump(Jump {
            instance: cfsm_name,
            source_id,
            edge_id,
        }))
    }
    for (cfsm_name, source_id, edge_id) in externals
    {
        synthesis_steps.push(SynthesisStep::External(External {
            instance: cfsm_name,
            source_id,
            edge_id,
        }))
    }
    for (s_name, s_source_id, s_edge_id) in sendings.iter() {
        for (r_name, r_source_id, r_edge_id) in receivings.iter() {
            let s_channel = retrieve_channel_from_map(s_name, s_edge_id.clone(), group);
            let r_channel = retrieve_channel_from_map(r_name, r_edge_id.clone(), group);
            // from two different cfsms and through the same channel
            if s_name != r_name && s_channel == r_channel {
                synthesis_steps.push(SynthesisStep::Match(Match {
                    send_instance: s_name.clone(),
                    send_source: s_source_id.clone(),
                    send_edge: s_edge_id.clone(),
                    recv_instance: r_name.clone(),
                    recv_source: r_source_id.clone(),
                    recv_edge: r_edge_id.clone(),
                }));
            }
        }
    }
    if synthesis_steps.is_empty() {
        for (name, _, edge_id) in sendings.iter() {
            return Err(VerilockError::DanglingSending(DanglingSending {
                trace: error_trace.clone(),
                dangling: construct_action_description(name, edge_id.clone(), group),
            }));
        }
        for (name, _, edge_id) in receivings.iter() {
            return Err(VerilockError::DanglingReceiving(DanglingReceiving {
                trace: error_trace.clone(),
                dangling: construct_action_description(name, edge_id.clone(), group),
            }));
        }
    }
    Ok(synthesis_steps)
}

fn retrieve_channel_from_map(
    instance: &ModuleInstance,
    edge_id: EdgeIndex,
    group: &Group,
) -> Channel {
    group
        .get(instance)
        .expect("CFSM not found")
        .fsm
        .edge_weight(edge_id)
        .expect("edge not found")
        .communication
        .as_ref()
        .expect("communication not found")
        .channel()
}

fn construct_action_description(
    instance: &ModuleInstance,
    edge_id: EdgeIndex,
    group: &Group,
) -> Action {
    Action {
        subject: instance.clone(),
        action: group
            .get(instance)
            .expect("CFSM not found")
            .fsm
            .edge_weight(edge_id)
            .expect("edge not found")
            .describe(),
    }
}

fn all_possible_local_steps(
    local_configurations: &LocalConfigurations,
    group:&Group,
    env: &Environment,
    solver: &Solver,
) -> (Vec<LocalStep>, Vec<LocalStep>, Vec<LocalStep>, Vec<LocalStep>) {
    let mut jumps = Vec::new();
    let mut externals = Vec::new();
    let mut internal_sendings = Vec::new();
    let mut internal_receivings = Vec::new();
    for (cfsm_name, node_index) in local_configurations {
        let cfsm = group.get(cfsm_name).unwrap();
        for edge_ref in cfsm.fsm.edges(*node_index) {
            let edge = edge_ref.weight();
            let satisfiable = if edge.guard.is_none() && edge.updates.is_empty() {
                true
            } else {
                let mut extended_env = if let Some(g) = &edge.guard {
                    env.extend(g)
                } else {
                    env.clone()
                };
                for u in &edge.updates {
                    extended_env = extended_env.update(u);
                }
                match extended_env.satisfiable(solver) {
                    Ok(sat) => sat,
                    Err(e) => {
                        e.report();
                        false
                    }
                }
            };
            if satisfiable {
                let edge_id = edge_ref.id();
                if let Some(c) = &edge.communication {
                    if c.is_external() {
                        externals.push((cfsm_name.clone(), *node_index, edge_id));
                    } else {
                        if matches!(c, Communication::Send(_)) {
                            internal_sendings.push((cfsm_name.clone(), *node_index, edge_id));
                        } else {
                            internal_receivings.push((cfsm_name.clone(), *node_index, edge_id));
                        }
                    }
                } else {
                    jumps.push((cfsm_name.clone(), *node_index, edge_id));
                }
            }
        }
    }
    (jumps, externals, internal_sendings, internal_receivings)
}

fn initial_nodes(group: &Group) -> Vec<NodeIndex> {
    let mut initial_nodes: Vec<NodeIndex> = group.iter().map(|(_, cfsm)| cfsm.initial).collect();
    initial_nodes.sort();
    initial_nodes
}

fn retrieve_or_construct_node(
    node_map: &mut HashMap<Vec<NodeIndex>, BlankNode>,
    nodes: &Vec<NodeIndex>,
) -> BlankNode {
    if node_map.contains_key(nodes) {
        node_map.get(nodes).unwrap().clone()
    } else {
        let new_node = BlankNode::new();
        node_map.insert(nodes.clone(), new_node.clone());
        new_node
    }
}
