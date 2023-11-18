use crate::abstraction::protocol::{
    extract_protocol, Always, Block, Conditional, Connect, DependencyTree, ForkJoin, Loop,
    MultiArmedIfElse, Protocol, SessionComplex, TypedModule,
};
use crate::abstraction::sv_info::{Channel, ModuleInfo, ModuleInstance, Var};
use crate::cfsm::fsm::{construct_cfsm_from_module_instance, CFSM, FSM};
use crate::cfsm::synthesis::{synthesize, Group};
use crate::error::VerilockError;
use crate::parser;
use crate::task::Case;
use std::collections::{HashMap, VecDeque};
use z3::{Config, Context, Solver};

type VerificationTask = ModuleInfo;

type TaskQueue = VecDeque<VerificationTask>;

pub fn analyze(c: &Case) {
    let path = &c.path;
    let id = &c.identifier;
    let project = parser::parse_project(&path);
    let config = Config::new();
    let context = Context::new(&config);
    let solver = Solver::new(&context);
    let session_types = extract_protocol(&project, id);
    match session_types {
        Ok(t) => {
            let SessionComplex {
                dependency_forest,
                modules,
                module_instances,
                channel_instances: _,
                connections,
            } = t;
            let type_map = type_map(&modules);
            let mut error_detected = false;
            for tree in dependency_forest {
                match analyze_dependency_tree(
                    tree,
                    &type_map,
                    &module_instances,
                    &connections,
                    &solver,
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        error_detected = true;
                        e.report();
                        break;
                    }
                }
            }
            if !error_detected {
                println!("verified")
            }
        }
        Err(e) => e.report(),
    }
}

fn type_map(types: &Vec<TypedModule>) -> HashMap<String, TypedModule> {
    let mut map = HashMap::new();
    for t in types {
        map.insert(t.module.module_name.clone(), t.clone());
    }
    map
}

fn analyze_dependency_tree(
    tree: DependencyTree,
    type_map: &HashMap<String, TypedModule>,
    module_instances: &Vec<ModuleInstance>,
    connections: &Vec<Connect>,
    solver: &Solver,
) -> Result<(), VerilockError> {
    let mut queue = dependency_tree_to_task_queue(&tree);
    let mut cfsm_map = HashMap::new();
    let leaf_map = leaf_map(&tree);
    while let Some(task) = queue.pop_front() {
        let mut group = Group::new();
        // according to instantiation and dependency tree, construct communication group
        let sub_modules = retrieve_instance_in_scope(&task, module_instances);
        let connect_in_scope = retrieve_connect_in_scope(&task, connections);
        for sub_module in sub_modules {
            let cfsm = instantiate(
                &type_map[&sub_module.type_name],
                &sub_module,
                &connect_in_scope,
                leaf_map[&sub_module.type_name],
                &mut cfsm_map,
            );
            group.insert(sub_module, cfsm);
        }
        let parent = ModuleInstance::group_parent(&task.module_name);
        let parent_cfsm = instantiate(
            &type_map[&parent.type_name],
            &parent,
            &connect_in_scope,
            false,
            &mut cfsm_map,
        );
        group.insert(parent, parent_cfsm);
        match synthesize(group, cfsm_map[&task.module_name].clone().module, solver) {
            Ok(cfsm) => {
                // update the CFSM map with the synthesized CFSM
                cfsm_map.insert(task.module_name.clone(), cfsm);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn leaf_map(tree: &DependencyTree) -> HashMap<String, bool> {
    let mut map = HashMap::new();
    let root_id = tree.root_node_id().unwrap();
    for id in tree.traverse_post_order_ids(root_id).unwrap() {
        let node = tree.get(&id).unwrap();
        let children = tree.children(&id).unwrap();
        let is_leaf = children.count() == 0;
        map.insert(node.data().module_name.clone(), is_leaf);
    }
    map
}

fn instantiate(
    typed_module: &TypedModule,
    instance: &ModuleInstance,
    connections: &Vec<Connect>,
    is_leaf: bool,
    cfsm_map: &mut HashMap<String, CFSM>,
) -> CFSM {
    let channel_substitutions = channel_substitutions(&instance, &typed_module.module, connections);
    let optional_cfsm = cfsm_map.get(&instance.type_name);
    let CFSM {
        module,
        initial,
        finals,
        fsm,
    } = if !is_leaf && optional_cfsm.is_some() {
        optional_cfsm.unwrap()
    } else {
        let protocol =
            apply_channel_substitutions_on_protocol(&channel_substitutions, &typed_module.protocol);
        let cfsm = construct_cfsm_from_module_instance(
            &typed_module.module,
            instance,
            protocol,
            connections,
        );
        cfsm_map.insert(instance.type_name.clone(), cfsm);
        cfsm_map.get(&instance.type_name).unwrap()
    };
    let fsm = apply_channel_substitutions_on_fsm(&channel_substitutions, &fsm);
    CFSM {
        module: module.clone(),
        initial: initial.clone(),
        finals: finals.clone(),
        fsm,
    }
}

fn channel_substitutions(
    instance: &ModuleInstance,
    info: &ModuleInfo,
    connections: &Vec<Connect>,
) -> HashMap<Var, Channel> {
    let mut map = HashMap::new();
    info.ports.iter().enumerate().for_each(|(i, port)| {
        let r = connections
            .iter()
            .find(|c| c.module_instance == *instance && c.index == i);
        match r {
            None => None,
            Some(c) => map.insert(
                Var {
                    scope: info.module_name.clone(),
                    name: port.id.clone(),
                },
                c.channel.clone(),
            ),
        };
    });
    map
}

fn apply_channel_substitutions_on_protocol(
    channel_substitutions: &HashMap<Var, Channel>,
    protocol: &Protocol,
) -> Protocol {
    match protocol {
        Protocol::Unit => Protocol::Unit,
        Protocol::Always(a) => Protocol::Always(Box::new(apply_channel_substitutions_on_always(
            channel_substitutions,
            a,
        ))),
        Protocol::Block(b) => Protocol::Block(Box::new(apply_channel_substitutions_on_block(
            channel_substitutions,
            b,
        ))),
        Protocol::Communication(c) => {
            Protocol::Communication(c.rebind_channel(channel_substitutions))
        }
        Protocol::Extension(e) => Protocol::Extension(e.clone()),
        Protocol::ForkJoin(fj) => Protocol::ForkJoin(Box::new(
            apply_channel_substitutions_on_fork_join(channel_substitutions, fj),
        )),
        Protocol::MultiArmsIfElse(maie) => Protocol::MultiArmsIfElse(Box::new(
            apply_channel_substitutions_on_multi_arms_if_else(channel_substitutions, maie),
        )),
        Protocol::Update(u) => Protocol::Update(u.clone()),
        Protocol::Loop(l) => Protocol::Loop(Box::new(apply_channel_substitutions_on_loop(
            channel_substitutions,
            l,
        ))),
    }
}

fn apply_channel_substitutions_on_always(
    channel_substitutions: &HashMap<Var, Channel>,
    always: &Always,
) -> Always {
    Always {
        block: always
            .block
            .iter()
            .map(|p| apply_channel_substitutions_on_protocol(channel_substitutions, p))
            .collect(),
    }
}

fn apply_channel_substitutions_on_block(
    channel_substitutions: &HashMap<Var, Channel>,
    block: &Block,
) -> Block {
    Block {
        protocols: block
            .protocols
            .iter()
            .map(|p| apply_channel_substitutions_on_protocol(channel_substitutions, p))
            .collect(),
    }
}

fn apply_channel_substitutions_on_fork_join(
    channel_substitutions: &HashMap<Var, Channel>,
    fj: &ForkJoin,
) -> ForkJoin {
    ForkJoin {
        block: fj
            .block
            .iter()
            .map(|p| apply_channel_substitutions_on_protocol(channel_substitutions, p))
            .collect(),
    }
}

fn apply_channel_substitutions_on_multi_arms_if_else(
    channel_substitutions: &HashMap<Var, Channel>,
    maie: &MultiArmedIfElse,
) -> MultiArmedIfElse {
    MultiArmedIfElse {
        conditionals: maie
            .conditionals
            .iter()
            .map(|c| apply_channel_substitutions_on_conditional(channel_substitutions, c))
            .collect(),
        else_block: maie
            .else_block
            .as_ref()
            .map(|p| apply_channel_substitutions_on_protocol(channel_substitutions, &p)),
    }
}

fn apply_channel_substitutions_on_conditional(
    channel_substitutions: &HashMap<Var, Channel>,
    conditional: &Conditional,
) -> Conditional {
    Conditional {
        condition: conditional.condition.clone(),
        protocol: apply_channel_substitutions_on_protocol(
            channel_substitutions,
            &conditional.protocol,
        ),
    }
}

fn apply_channel_substitutions_on_loop(
    channel_substitutions: &HashMap<Var, Channel>,
    loop_: &Loop,
) -> Loop {
    Loop {
        condition: loop_.condition.clone(),
        protocol: apply_channel_substitutions_on_protocol(channel_substitutions, &loop_.protocol),
    }
}

fn apply_channel_substitutions_on_fsm(
    channel_substitutions: &HashMap<Var, Channel>,
    fsm: &FSM,
) -> FSM {
    fsm.map(
        |_, n| n.clone(),
        |_, edge| edge.rebind_channel(channel_substitutions),
    )
}

fn retrieve_instance_in_scope(
    scope: &ModuleInfo,
    instances: &Vec<ModuleInstance>,
) -> Vec<ModuleInstance> {
    instances
        .into_iter()
        .filter(|i| i.scope == scope.module_name)
        .cloned()
        .collect()
}

fn retrieve_connect_in_scope(scope: &ModuleInfo, connections: &Vec<Connect>) -> Vec<Connect> {
    connections
        .into_iter()
        .filter(|c| c.scope() == scope.module_name)
        .cloned()
        .collect()
}

fn dependency_tree_to_task_queue(tree: &DependencyTree) -> TaskQueue {
    let mut queue = TaskQueue::new();
    let root_id = tree.root_node_id().unwrap();
    for id in tree.traverse_post_order_ids(root_id).unwrap() {
        if let Ok(children) = tree.children(&id) {
            if children.count() > 0 {
                let parent = tree.get(&id).unwrap().data().clone();
                queue.push_back(parent);
            }
        }
    }
    queue
}
