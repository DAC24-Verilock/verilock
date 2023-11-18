use crate::abstraction::protocol::{
    Block, Communication, Connect, ForkJoin, Protocol, Receiving, Sending,
};
use crate::abstraction::sv_info::{Channel, ModuleInstance};
use std::collections::{HashMap, HashSet};

// if two communications are independent, then they can be linearize to prune state space
pub fn linearize_fork_join(
    current_instance: &ModuleInstance,
    fj: &ForkJoin,
    connections: &Vec<Connect>,
) -> ForkJoin {
    let map = channel_to_module_instance(connections);
    let mut linearized: Vec<Vec<Protocol>> = Vec::new();
    for s in &fj.block {
        if let Some(list) = linearized
            .iter_mut()
            .find(|l| all_independent(l, s, current_instance, &map))
        {
            list.push(s.clone());
        } else {
            linearized.push(vec![s.clone()]);
        }
    }
    ForkJoin {
        block: linearized
            .into_iter()
            .map(|list| Protocol::Block(Box::new(Block { protocols: list })))
            .collect(),
    }
}

fn channel_to_module_instance(
    connections: &Vec<Connect>,
) -> HashMap<Channel, HashSet<ModuleInstance>> {
    let mut map: HashMap<Channel, HashSet<ModuleInstance>> = HashMap::new();
    for c in connections {
        let channel = &c.channel;
        let module_instance = &c.module_instance;
        if map.contains_key(channel) {
            map.get_mut(channel)
                .unwrap()
                .insert(module_instance.clone());
        } else {
            let mut set = HashSet::new();
            set.insert(module_instance.clone());
            map.insert(channel.clone(), set);
        }
    }
    map
}

fn all_independent(
    list: &Vec<Protocol>,
    single: &Protocol,
    current_instance: &ModuleInstance,
    channel_map: &HashMap<Channel, HashSet<ModuleInstance>>,
) -> bool {
    list.iter()
        .all(|s| is_independent(s, single, current_instance, channel_map))
}

// 1. sending actions on the different channels connected to different modules are independent
// 2. receiving actions onto different variables are independent
// 3. sending and receiving actions are independent
// independent checking only applies for communications
fn is_independent(
    a: &Protocol,
    b: &Protocol,
    current_instance: &ModuleInstance,
    channel_map: &HashMap<Channel, HashSet<ModuleInstance>>,
) -> bool {
    match a {
        Protocol::Communication(ac) => match b {
            Protocol::Communication(bc) => match ac {
                Communication::Send(a_sending) => match bc {
                    Communication::Send(b_sending) => {
                        check_two_sendings(a_sending, b_sending, current_instance, channel_map)
                    }
                    Communication::Receive(_) => true,
                },
                Communication::Receive(a_receiving) => match bc {
                    Communication::Send(_) => true,
                    Communication::Receive(b_receiving) => {
                        check_two_receivings(a_receiving, b_receiving)
                    }
                },
            },
            _ => false,
        },
        _ => false,
    }
}

fn check_two_sendings(
    a: &Sending,
    b: &Sending,
    current_instance: &ModuleInstance,
    channel_map: &HashMap<Channel, HashSet<ModuleInstance>>,
) -> bool {
    let a_channel = &a.channel;
    let b_channel = &b.channel;
    let empty_set = HashSet::new();
    let a_connected = channel_map
        .get(a_channel)
        .unwrap_or(&empty_set)
        .iter()
        .filter(|i| *i != current_instance)
        .collect::<Vec<&ModuleInstance>>();
    let b_connected = channel_map
        .get(b_channel)
        .unwrap_or(&empty_set)
        .iter()
        .filter(|i| *i != current_instance)
        .collect::<Vec<&ModuleInstance>>();
    a_connected.len() == 1 && b_connected.len() == 1 && a_connected[0] != b_connected[0]
}

fn check_two_receivings(a: &Receiving, b: &Receiving) -> bool {
    a.receiver != b.receiver
}
