use crate::abstraction::data_verification::verify_channel_implementation;
use crate::abstraction::protocol::Protocol::Unit;
use crate::abstraction::sv_info;
use crate::abstraction::sv_info::{
    BinRel, BoolExpression, Channel, ChannelInstance, InterfaceInfo, ModuleInfo, ModuleInstance,
    Port, PortDirection, Primary, Var,
};
use crate::error::VerilockError;
use crate::task::ChannelIdentifier;
use id_tree::InsertBehavior::{AsRoot, UnderNode};
use id_tree::{Node, NodeId, Tree, TreeBuilder};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use sv_parser::{
    unwrap_node, AnsiPortDeclaration, BinaryOperator, BlockItemDeclaration, CondPredicate,
    ConditionalStatement, DataDeclaration, ExpressionOrCondPattern, ListOfArguments,
    ListOfPortConnectionsOrdered, ListOfPortDeclarations, Locate, LoopStatement, NonPortModuleItem,
    ParBlock, Paren, PsOrHierarchicalTfIdentifier, RefNode, SeqBlock, StatementItem,
    StatementOrNull, SubroutineCall, SubroutineCallStatement, SyntaxTree, VariableDeclAssignment,
};

#[derive(Debug, PartialEq, Clone)]
pub enum Protocol {
    Unit,
    Always(Box<Always>),
    Block(Box<Block>),
    Communication(Communication),
    Extension(Extension),
    ForkJoin(Box<ForkJoin>),
    MultiArmsIfElse(Box<MultiArmedIfElse>),
    Update(Update),
    Loop(Box<Loop>),
}

impl Protocol {
    pub fn pretty_print(&self, padding: &str) {
        match self {
            Unit => print!("{}()", padding),
            Protocol::Always(a) => a.pretty_print(padding),
            Protocol::Block(b) => b.pretty_print(padding),
            Protocol::Communication(c) => c.pretty_print(padding),
            Protocol::Extension(e) => println!("{}", e.show()),
            Protocol::ForkJoin(fj) => fj.pretty_print(padding),
            Protocol::MultiArmsIfElse(maie) => maie.pretty_print(padding),
            Protocol::Update(u) => println!("{}{}", padding, u.show()),
            Protocol::Loop(l) => l.pretty_print(padding),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Always {
    pub block: Vec<Protocol>,
}

impl Always {
    fn pretty_print(&self, padding: &str) {
        println!("{}Always {{", padding);
        self.block
            .iter()
            .for_each(|s| s.pretty_print(&format!("{}  ", padding)));
        println!("{}}}", padding);
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Block {
    pub protocols: Vec<Protocol>,
}

impl Block {
    pub fn pretty_print(&self, padding: &str) {
        self.protocols.iter().for_each(|s| s.pretty_print(padding))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Conditional {
    pub condition: BoolExpression,
    pub protocol: Protocol,
}

impl Conditional {
    pub fn pretty_print(&self, index: usize, padding: &str) {
        if index == 0 {
            println!("{}If({}) {{", padding, self.condition.show());
        } else {
            println!("{}Else If({}) {{", padding, self.condition.show());
        }
        self.protocol.pretty_print(&format!("{}  ", padding));
        println!("{}}}", padding);
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Communication {
    Send(Sending),
    Receive(Receiving),
}
impl Communication {
    pub fn channel(&self) -> Channel {
        match self {
            Communication::Send(s) => s.channel.clone(),
            Communication::Receive(r) => r.channel.clone(),
        }
    }

    pub fn rebind_channel(&self, map: &HashMap<Var, Channel>) -> Communication {
        match self {
            Communication::Send(s) => Communication::Send(s.rebind_channel(map)),
            Communication::Receive(r) => Communication::Receive(r.rebind_channel(map)),
        }
    }

    pub fn is_external(&self) -> bool {
        match self {
            Communication::Send(s) => matches!(s.channel, Channel::Ref(_)),
            Communication::Receive(r) => matches!(r.channel, Channel::Ref(_)),
        }
    }

    pub fn is_internal(&self) -> bool {
        !self.is_external()
    }

    fn pretty_print(&self, padding: &str) {
        match self {
            Communication::Send(s) => {
                println!("{}{}<-{}", padding, s.channel.show(), s.info.show())
            }
            Communication::Receive(r) => {
                println!("{}{}->{}", padding, r.channel.show(), r.receiver.name)
            }
        }
    }
}

impl Display for Communication {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Communication::Send(s) => write!(f, "{}<-{}", s.channel.show(), s.info.show()),
            Communication::Receive(r) => write!(f, "{}->{}", r.channel.show(), r.receiver.name),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Sending {
    pub channel: Channel,
    pub info: Primary,
}

impl Sending {
    pub fn rebind_channel(&self, map: &HashMap<Var, Channel>) -> Sending {
        match &self.channel {
            Channel::Instance(_) => self.clone(),
            Channel::Ref(r) => {
                if map.contains_key(r) {
                    Sending {
                        channel: map.get(r).unwrap().clone(),
                        info: self.info.clone(),
                    }
                } else {
                    self.clone()
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Receiving {
    pub channel: Channel,
    pub receiver: Var,
}

impl Receiving {
    pub fn rebind_channel(&self, map: &HashMap<Var, Channel>) -> Receiving {
        match &self.channel {
            Channel::Instance(_) => self.clone(),
            Channel::Ref(r) => {
                if map.contains_key(r) {
                    Receiving {
                        channel: map.get(r).unwrap().clone(),
                        receiver: self.receiver.clone(),
                    }
                } else {
                    self.clone()
                }
            }
        }
    }
}

pub type Extension = BoolExpression;

#[derive(Debug, PartialEq, Clone)]
pub struct Update {
    pub var: Var,
    pub primary: Primary,
}

impl Update {
    pub fn show(&self) -> String {
        format!("{}:={}", self.var.name, self.primary.show())
    }
}

impl Display for Update {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.show())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ForkJoin {
    pub block: Vec<Protocol>,
}

impl ForkJoin {
    pub fn pretty_print(&self, padding: &str) {
        println!("{}Fork {{", padding);
        self.block
            .iter()
            .for_each(|s| s.pretty_print(&format!("{}  ", padding)));
        println!("{}}}", padding);
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct MultiArmedIfElse {
    pub conditionals: Vec<Conditional>,
    pub else_block: Option<Protocol>,
}

impl MultiArmedIfElse {
    pub fn pretty_print(&self, padding: &str) {
        self.conditionals
            .iter()
            .enumerate()
            .for_each(|(i, c)| c.pretty_print(i, padding));
        if self.else_block.is_some() {
            println!("{}Else {{", padding);
            self.else_block
                .iter()
                .for_each(|s| s.pretty_print(&format!("{}  ", padding)));
            println!("{}}}", padding);
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Loop {
    pub condition: BoolExpression,
    pub protocol: Protocol,
}

impl Loop {
    pub fn pretty_print(&self, padding: &str) {
        println!("{}Loop({}) {{", padding, self.condition.show());
        self.protocol.pretty_print(&format!("{}  ", padding));
        println!("{}}}", padding);
    }
}

// a module instantiate another sub module
#[derive(Debug, PartialEq, Clone)]
pub struct Instantiation {
    pub module: ModuleInfo,
    pub instance: ModuleInstance,
}

// connecting a channel instance to a module instance
#[derive(Debug, PartialEq, Clone)]
pub struct Connect {
    pub module_instance: ModuleInstance,
    pub channel: Channel,
    pub index: usize,
}

impl Connect {
    pub fn scope(&self) -> String {
        match &self.channel {
            Channel::Instance(i) => i.scope.clone(),
            Channel::Ref(r) => r.scope.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedModule {
    pub module: ModuleInfo,
    pub protocol: Protocol,
}

impl TypedModule {
    fn pretty_print(&self) {
        println!("{}:", self.module.module_name);
        self.protocol.pretty_print("");
    }
}

pub type DependencyTree = Tree<ModuleInfo>;

fn print_dependency_tree(tree: &DependencyTree) {
    let root = tree.root_node_id();
    if root.is_some() {
        print_helper(tree, root.unwrap());
    }
}

fn print_helper(tree: &DependencyTree, id: &NodeId) {
    let children = tree.children(id);
    let parent = tree.get(id);
    if parent.is_ok() && children.is_ok() {
        let parent = parent.unwrap();
        let children = children.unwrap();
        let children: Vec<&Node<ModuleInfo>> = children.collect();
        let child_ids = tree.children_ids(id);
        if children.len() > 0 {
            print!("{} depends on: ", parent.data().module_name);
            let dependencies = children
                .iter()
                .map(|m| m.data().module_name.clone())
                .collect::<Vec<String>>()
                .join(", ");
            println!("{}", dependencies);
            for c in child_ids.unwrap() {
                print_helper(tree, c);
            }
        }
    }
}

type DependencyForest = Vec<DependencyTree>;

#[derive(Debug, PartialEq, Clone)]
pub struct SessionComplex {
    pub dependency_forest: DependencyForest,
    pub modules: Vec<TypedModule>,
    pub module_instances: Vec<ModuleInstance>,
    pub channel_instances: Vec<ChannelInstance>,
    pub connections: Vec<Connect>,
}

impl SessionComplex {
    pub fn pretty_print(&self) {
        println!("Dependency Forest:");
        self.dependency_forest
            .iter()
            .for_each(print_dependency_tree);
        println!("\n\nSession Types:");
        self.modules.iter().for_each(TypedModule::pretty_print);
    }
}

pub fn extract_protocol(
    asts: &Vec<SyntaxTree>,
    id: &ChannelIdentifier,
) -> Result<SessionComplex, VerilockError> {
    // 1. extract static module and interface information
    let (modules, interfaces) = extract_module_and_interface_info(asts);
    // 2. verify whether channel implementations accord with the provided channel identifier
    if let Some(e) = verify_channel_implementation(&interfaces, id) {
        return Err(VerilockError::NoChannelImplementation(e));
    }
    // 3. identify module and interface instantiation & abstraction types
    let info_map: HashMap<&String, &ModuleInfo> = modules
        .iter()
        .map(|info| (&info.module_name, info))
        .collect();
    let (module_instances, channel_instances, instantiations, connections, typed) =
        extract_instantiation_and_infer_session_types(asts, id, &info_map);
    let forest = construct_dependency_forest(&instantiations, &modules, &info_map);
    Ok(SessionComplex {
        dependency_forest: forest,
        modules: typed,
        module_instances,
        channel_instances,
        connections,
    })
}

fn construct_dependency_forest(
    instantiations: &Vec<Instantiation>,
    module_infos: &Vec<ModuleInfo>,
    info_map: &HashMap<&String, &ModuleInfo>,
) -> DependencyForest {
    let instantiated: HashSet<String> = instantiations
        .iter()
        .map(|i| i.instance.type_name.clone())
        .collect();
    let tops: Vec<&ModuleInfo> = module_infos
        .iter()
        .filter(|m| !instantiated.contains(&m.module_name))
        .collect();
    tops.iter()
        .map(|&m| {
            let mut tree: DependencyTree = TreeBuilder::new().build();
            let id = tree.insert(Node::new(m.clone()), AsRoot).unwrap();
            construct_dependency_tree(&mut tree, instantiations, info_map, m, &id);
            tree
        })
        .collect()
}

fn construct_dependency_tree(
    tree: &mut DependencyTree,
    instantiations: &Vec<Instantiation>,
    module_map: &HashMap<&String, &ModuleInfo>,
    current: &ModuleInfo,
    current_id: &NodeId,
) {
    for i in instantiations {
        if i.module == *current {
            let sub = module_map.get(&i.instance.type_name).unwrap();
            let id = tree
                .insert(Node::new((*sub).clone()), UnderNode(current_id))
                .unwrap();
            construct_dependency_tree(tree, instantiations, module_map, sub, &id);
        }
    }
}

fn extract_module_and_interface_info(
    asts: &Vec<SyntaxTree>,
) -> (Vec<ModuleInfo>, Vec<InterfaceInfo>) {
    let mut modules = Vec::new();
    let mut interfaces = Vec::new();
    for ast in asts {
        for node in ast {
            match node {
                RefNode::ModuleDeclarationAnsi(x) => {
                    let id = unwrap_node!(x, ModuleIdentifier).unwrap();
                    let module_name = get_identifier(id, ast).unwrap();
                    let head = &x.nodes.0;
                    let ports = &head.nodes.6;
                    let ports = port_declaration_list_to_port_infos(ports, ast);
                    modules.push(ModuleInfo { module_name, ports })
                }
                RefNode::InterfaceDeclarationAnsi(x) => {
                    let id = unwrap_node!(x, InterfaceIdentifier).unwrap();
                    let interface_name = get_identifier(id, ast).unwrap();
                    let items = &x.nodes.2;
                    let tasks: HashSet<String> = items
                        .iter()
                        .filter_map(|item| {
                            let task_dec = unwrap_node!(item, TaskDeclaration)?;
                            let task_id = unwrap_node!(task_dec, TaskIdentifier)?;
                            get_identifier(task_id, ast)
                        })
                        .collect();
                    interfaces.push(InterfaceInfo {
                        interface_name,
                        tasks,
                    })
                }
                _ => (),
            }
        }
    }
    (modules, interfaces)
}

fn port_declaration_list_to_port_infos(
    list: &Option<ListOfPortDeclarations>,
    ast: &SyntaxTree,
) -> Vec<Port> {
    match list {
        Some(l) => {
            let list = &(&(&(&l.nodes).0).nodes).1;
            match list {
                Some(separation) => {
                    let mut ports = Vec::new();
                    let (head, tail) = &separation.nodes;
                    ports.push(ansi_port_declaration_to_port_info(0, &head.1, ast));
                    tail.iter()
                        .enumerate()
                        .map(|(index, (_, (_, dec)))| {
                            ansi_port_declaration_to_port_info(index + 1, dec, ast)
                        })
                        .for_each(|p| ports.push(p));
                    ports
                }
                None => Vec::new(),
            }
        }
        None => Vec::new(),
    }
}

// this function only process Interface port for simplicity
fn ansi_port_declaration_to_port_info(
    index: usize,
    item: &AnsiPortDeclaration,
    ast: &SyntaxTree,
) -> Port {
    let id = unwrap_node!(item, PortIdentifier).unwrap();
    let id = get_identifier(id, ast).unwrap();
    Port {
        direction: PortDirection::Interface,
        id,
        index,
    }
}

// merge instantiation extraction and type inference into one pass
fn extract_instantiation_and_infer_session_types(
    asts: &Vec<SyntaxTree>,
    channel_id: &ChannelIdentifier,
    module_info_map: &HashMap<&String, &ModuleInfo>,
) -> (
    Vec<ModuleInstance>,
    Vec<ChannelInstance>,
    Vec<Instantiation>,
    Vec<Connect>,
    Vec<TypedModule>,
) {
    let mut module_instances: Vec<ModuleInstance> = Vec::new();
    let mut channel_instances: Vec<ChannelInstance> = Vec::new();
    let mut instantiations: Vec<Instantiation> = Vec::new();
    let mut connections: Vec<Connect> = Vec::new();
    let mut typed: Vec<TypedModule> = Vec::new();
    for ast in asts {
        for node in ast {
            match node {
                RefNode::ModuleDeclarationAnsi(x) => {
                    let id = unwrap_node!(x, ModuleIdentifier).unwrap();
                    let module_name = get_identifier(id, ast).unwrap();
                    let mut local_channels: Vec<ChannelInstance> = Vec::new();
                    if let Some(scope) = module_info_map.get(&module_name) {
                        let items = &x.nodes.2;
                        for item in items {
                            if let Some(RefNode::ModuleInstantiation(i)) =
                                unwrap_node!(item, ModuleInstantiation)
                            {
                                let type_name_node = unwrap_node!(i, ModuleIdentifier).unwrap();
                                let type_name = get_identifier(type_name_node, ast).unwrap();
                                // do not support multiple module instantiations in a single line
                                let instance_node = unwrap_node!(i, HierarchicalInstance).unwrap();
                                let instance_name_node =
                                    unwrap_node!(instance_node.clone(), InstanceIdentifier)
                                        .unwrap();
                                let instance_name =
                                    get_identifier(instance_name_node, ast).unwrap();
                                if channel_id.channel_name == type_name {
                                    local_channels.push(ChannelInstance {
                                        scope: (*scope).module_name.clone(),
                                        type_name,
                                        instance_name,
                                    });
                                } else if module_info_map.contains_key(&type_name) {
                                    let instance = ModuleInstance {
                                        scope: (*scope).module_name.clone(),
                                        type_name,
                                        instance_name,
                                    };
                                    module_instances.push(instance.clone());
                                    instantiations.push(Instantiation {
                                        module: (*scope).clone(),
                                        instance: instance.clone(),
                                    });
                                    // deal with connection
                                    // do not support named binding
                                    if let Some(RefNode::ListOfPortConnectionsOrdered(
                                        port_connections,
                                    )) =
                                        unwrap_node!(instance_node, ListOfPortConnectionsOrdered)
                                    {
                                        connections.extend(
                                            convert_ordered_port_connection_to_verilock_connection(
                                                port_connections,
                                                scope,
                                                &instance,
                                                &local_channels,
                                                ast,
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                        let local_channel_maps = local_channels
                            .iter()
                            .map(|c| (c.instance_name.clone(), c))
                            .collect();
                        typed.push(infer_session_type_for_module(
                            ast,
                            scope,
                            channel_id,
                            &local_channel_maps,
                            items,
                        ));
                    }
                    channel_instances.extend(local_channels);
                }
                _ => {}
            }
        }
    }
    (
        module_instances,
        channel_instances,
        instantiations,
        connections,
        typed,
    )
}

fn convert_ordered_port_connection_to_verilock_connection(
    connections: &ListOfPortConnectionsOrdered,
    module_info: &ModuleInfo,
    module_instance: &ModuleInstance,
    local_channels: &Vec<ChannelInstance>,
    ast: &SyntaxTree,
) -> Vec<Connect> {
    let channel_map: HashMap<&String, &ChannelInstance> = local_channels
        .iter()
        .map(|i| (&i.instance_name, i))
        .collect();
    let connections = &connections.nodes.0.contents();
    connections
        .iter()
        .enumerate()
        .map(|(index, c)| {
            let node = unwrap_node!(*c, HierarchicalIdentifier).unwrap();
            let channel_ref = get_identifier(node, ast).unwrap();
            if let Some(&i) = channel_map.get(&channel_ref) {
                Connect {
                    module_instance: module_instance.clone(),
                    channel: Channel::Instance(i.clone()),
                    index,
                }
            } else {
                Connect {
                    module_instance: module_instance.clone(),
                    channel: Channel::Ref(Var {
                        scope: module_info.module_name.clone(),
                        name: channel_ref,
                    }),
                    index,
                }
            }
        })
        .collect()
}

fn infer_session_type_for_module(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    id: &ChannelIdentifier,
    local_channels: &HashMap<String, &ChannelInstance>,
    items: &Vec<NonPortModuleItem>,
) -> TypedModule {
    for item in items {
        if let Some(RefNode::AlwaysConstruct(ac)) = unwrap_node!(item, AlwaysConstruct) {
            let statement = &ac.nodes.1.nodes.2;
            let block = if let Some(s) = infer_statement(ast, scope, id, local_channels, statement)
            {
                vec![s]
            } else {
                Vec::new()
            };
            let session_type = Protocol::Always(Box::new(Always { block }));
            return TypedModule {
                module: scope.clone(),
                protocol: session_type,
            };
        }
    }
    TypedModule {
        module: scope.clone(),
        protocol: Protocol::Block(Box::new(Block {
            protocols: Vec::new(),
        })),
    }
}

fn infer_conditional_statement(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    id: &ChannelIdentifier,
    local_channels: &HashMap<String, &ChannelInstance>,
    statement: &ConditionalStatement,
) -> MultiArmedIfElse {
    let first_conditional = infer_single_branch_conditional(
        ast,
        scope,
        id,
        local_channels,
        &statement.nodes.2,
        &statement.nodes.3,
    );
    let mut arms: Vec<Conditional> = statement
        .nodes
        .4
        .iter()
        .map(|(_, _, c, s)| infer_single_branch_conditional(ast, scope, id, local_channels, c, s))
        .collect();
    let maybe_else = statement
        .nodes
        .5
        .as_ref()
        .map(|(_, s)| infer_statement_or_null(ast, scope, id, local_channels, s));
    arms.insert(0, first_conditional);
    MultiArmedIfElse {
        conditionals: arms,
        else_block: maybe_else,
    }
}

fn infer_single_branch_conditional(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    id: &ChannelIdentifier,
    local_channels: &HashMap<String, &ChannelInstance>,
    condition: &Paren<CondPredicate>,
    statement: &StatementOrNull,
) -> Conditional {
    let condition = cond_predicate_to_expression(&condition.nodes.1);
    let condition = extract_optional_bool_expression(ast, scope, &condition);
    let session = infer_statement_or_null(ast, scope, id, local_channels, statement);
    Conditional {
        condition,
        protocol: session,
    }
}

fn infer_communication(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    id: &ChannelIdentifier,
    local_channels: &HashMap<String, &ChannelInstance>,
    call: &SubroutineCallStatement,
) -> Option<Protocol> {
    if let SubroutineCallStatement::SubroutineCall(sc) = call {
        match &sc.0 {
            SubroutineCall::TfCall(call) => {
                if let PsOrHierarchicalTfIdentifier::HierarchicalTfIdentifier(h) = &call.nodes.0 {
                    let prefixs = &h.nodes.0.nodes.1;
                    if prefixs.len() == 1 {
                        let prefix = prefixs.first()?;
                        let callee = get_identifier(RefNode::Identifier(&prefix.0), ast)?;
                        let task = get_identifier(RefNode::Identifier(&h.nodes.0.nodes.2), ast)?;
                        let params = &(call.nodes.2.clone()?).nodes.1;
                        if let ListOfArguments::Ordered(ordered) = params {
                            let params = ordered.nodes.0.contents();
                            if params.len() == 1 {
                                if params[0].is_some() {
                                    let param = params[0].clone().unwrap();
                                    return construct_communication(
                                        id,
                                        scope,
                                        ast,
                                        local_channels,
                                        &callee,
                                        task,
                                        param,
                                    )
                                    .map(|c| Protocol::Communication(c));
                                }
                            }
                        }
                    }
                }
            }
            SubroutineCall::MethodCall(call) => {
                let callee = get_identifier(RefNode::MethodCallRoot(&call.nodes.0), ast)?;
                let task = unwrap_node!(&call.nodes.2, MethodIdentifier)?;
                let task_name = get_identifier(task, ast)?;
                if let RefNode::ListOfArgumentsOrdered(lao) =
                    unwrap_node!(&call.nodes.2, ListOfArgumentsOrdered)?
                {
                    let params = lao.nodes.0.contents();
                    if params.len() == 1 {
                        let param = params[0].clone()?;
                        return construct_communication(
                            id,
                            scope,
                            ast,
                            local_channels,
                            &callee,
                            task_name,
                            param,
                        )
                        .map(|c| Protocol::Communication(c));
                    }
                }
            }
            _ => return None,
        }
    }
    None
}

fn construct_communication(
    id: &ChannelIdentifier,
    scope: &ModuleInfo,
    ast: &SyntaxTree,
    local_channels: &HashMap<String, &ChannelInstance>,
    callee: &String,
    task_name: String,
    param: sv_parser::Expression,
) -> Option<Communication> {
    let channel = local_channels
        .get(callee)
        .map(|i| Channel::Instance((*i).clone()))
        .unwrap_or(Channel::Ref(Var {
            name: callee.clone(),
            scope: scope.module_name.clone(),
        }));
    if task_name == id.send_name {
        let param = to_primary(ast, scope, &param).unwrap_or(Primary::Unknown);
        Some(Communication::Send(Sending {
            channel,
            info: param,
        }))
    } else if task_name == id.receive_name {
        let receiver = to_var(ast, scope, &param)?;
        Some(Communication::Receive(Receiving { channel, receiver }))
    } else {
        None
    }
}

fn infer_loop_statement(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    id: &ChannelIdentifier,
    local_channels: &HashMap<String, &ChannelInstance>,
    statement: &LoopStatement,
) -> Option<Protocol> {
    match statement {
        LoopStatement::While(w) => {
            let condition = extract_bool_expression(ast, scope, &w.nodes.1.nodes.1);
            let session = infer_statement(
                ast,
                scope,
                id,
                local_channels,
                statement_or_null_to_statement_item(&w.nodes.2)?,
            )?;
            Some(Protocol::Loop(Box::new(Loop {
                condition,
                protocol: session,
            })))
        }
        _ => None,
    }
}

fn infer_par_block(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    id: &ChannelIdentifier,
    local_channels: &HashMap<String, &ChannelInstance>,
    block: &ParBlock,
) -> ForkJoin {
    let updates: Vec<Protocol> = extract_assignments_from_items(ast, scope, &block.nodes.2)
        .iter()
        .map(|u| Protocol::Update(u.clone()))
        .collect();
    let statements = block
        .nodes
        .3
        .iter()
        .filter_map(statement_or_null_to_statement_item)
        .collect();
    let mut block: Vec<Protocol> = infer_statements(ast, scope, id, local_channels, &statements);
    if !updates.is_empty() {
        block = block
            .iter()
            .map(|b| {
                let mut prefix = updates.clone();
                prefix.push(b.clone());
                Protocol::Block(Box::new(Block { protocols: prefix }))
            })
            .collect();
    }
    ForkJoin { block }
}

fn infer_seq_block(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    id: &ChannelIdentifier,
    local_channels: &HashMap<String, &ChannelInstance>,
    block: &SeqBlock,
) -> Block {
    let statements = block
        .nodes
        .3
        .iter()
        .filter_map(statement_or_null_to_statement_item)
        .collect();
    let types: Vec<Protocol> = extract_assignments_from_items(ast, scope, &block.nodes.2)
        .iter()
        .map(|u| Protocol::Update(u.clone()))
        .chain(infer_statements(
            ast,
            scope,
            id,
            local_channels,
            &statements,
        ))
        .collect();
    Block { protocols: types }
}

fn infer_statements(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    id: &ChannelIdentifier,
    local_channels: &HashMap<String, &ChannelInstance>,
    items: &Vec<&StatementItem>,
) -> Vec<Protocol> {
    items
        .iter()
        .filter_map(|i| infer_statement(ast, scope, id, local_channels, i))
        .collect()
}

fn infer_statement_or_null(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    id: &ChannelIdentifier,
    local_channels: &HashMap<String, &ChannelInstance>,
    item: &StatementOrNull,
) -> Protocol {
    match item {
        StatementOrNull::Statement(s) => {
            match infer_statement(ast, scope, id, local_channels, &s.nodes.2) {
                Some(session) => session,
                None => Unit,
            }
        }
        StatementOrNull::Attribute(_) => Unit,
    }
}

// statement-level inference
fn infer_statement(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    id: &ChannelIdentifier,
    local_channels: &HashMap<String, &ChannelInstance>,
    item: &StatementItem,
) -> Option<Protocol> {
    match item {
        StatementItem::ConditionalStatement(cs) => Some(Protocol::MultiArmsIfElse(Box::new(
            infer_conditional_statement(ast, scope, id, local_channels, cs),
        ))),
        StatementItem::SubroutineCallStatement(scs) => {
            infer_communication(ast, scope, id, local_channels, scs)
        }
        StatementItem::LoopStatement(ls) => {
            infer_loop_statement(ast, scope, id, local_channels, ls)
        }
        StatementItem::ParBlock(pb) => Some(Protocol::ForkJoin(Box::new(infer_par_block(
            ast,
            scope,
            id,
            local_channels,
            pb,
        )))),
        StatementItem::SeqBlock(sb) => Some(Protocol::Block(Box::new(infer_seq_block(
            ast,
            scope,
            id,
            local_channels,
            sb,
        )))),
        _ => None,
    }
}

fn extract_assignments_from_items(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    decs: &Vec<BlockItemDeclaration>,
) -> Vec<Update> {
    decs.iter()
        .flat_map(|dec| extract_assignment_from_item(ast, scope, dec))
        .collect()
}

fn extract_assignment_from_item(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    dec: &BlockItemDeclaration,
) -> Vec<Update> {
    if let BlockItemDeclaration::Data(d) = dec {
        if let DataDeclaration::Variable(v) = &d.nodes.1 {
            return v
                .nodes
                .4
                .nodes
                .0
                .contents()
                .iter()
                .filter_map(|a| match a {
                    VariableDeclAssignment::Variable(assignment) => Some(assignment),
                    _ => None,
                })
                .filter_map(|assignment| {
                    let v_str =
                        get_identifier(RefNode::VariableIdentifier(&assignment.nodes.0), ast)?;
                    let p: Primary = assignment
                        .nodes
                        .2
                        .clone()
                        .map(|(_, expr)| to_primary(ast, scope, &expr))
                        .unwrap_or(None)?;
                    Some(Update {
                        var: Var {
                            name: v_str,
                            scope: scope.module_name.clone(),
                        },
                        primary: p,
                    })
                })
                .collect();
        }
    }
    Vec::new()
}

fn extract_optional_bool_expression(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    expr: &Option<&sv_parser::Expression>,
) -> BoolExpression {
    match expr {
        Some(e) => extract_bool_expression(ast, scope, e),
        None => BoolExpression::Unknown,
    }
}

// it only extract binary boolean expression
fn extract_bool_expression(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    expr: &sv_parser::Expression,
) -> BoolExpression {
    match expr {
        sv_parser::Expression::Binary(b) => {
            let l = to_primary(ast, scope, &b.nodes.0);
            let op = to_op(ast, &b.nodes.1);
            let r = to_primary(ast, scope, &b.nodes.3);
            if l.is_some() && op.is_some() && r.is_some() {
                return BoolExpression::Binary(l.unwrap(), op.unwrap(), r.unwrap());
            }
            BoolExpression::Unknown
        }
        _ => BoolExpression::Unknown,
    }
}

fn statement_or_null_to_statement_item(son: &StatementOrNull) -> Option<&StatementItem> {
    match son {
        StatementOrNull::Attribute(_) => None,
        StatementOrNull::Statement(s) => Some(&(*s).nodes.2),
    }
}

fn cond_predicate_to_expression(cp: &CondPredicate) -> Option<&sv_parser::Expression> {
    let eoc = &cp.nodes.0.nodes.0;
    match eoc {
        ExpressionOrCondPattern::Expression(e) => Some(e),
        ExpressionOrCondPattern::CondPattern(_) => None,
    }
}

fn get_identifier(node: RefNode, ast: &SyntaxTree) -> Option<String> {
    let id = get_identifier_locate(node)?.embody(ast);
    Some(id.to_string())
}

fn get_number(node: RefNode, ast: &SyntaxTree) -> Option<i32> {
    match node {
        RefNode::UnsignedNumber(un) => {
            let n = un.nodes.0.embody(ast);
            let n: Result<i32, _> = n.parse();
            match n {
                Ok(i) => Some(i),
                _ => None,
            }
        }
        _ => None,
    }
}

fn get_identifier_locate(node: RefNode) -> Option<Locate> {
    match unwrap_node!(node, SimpleIdentifier, EscapedIdentifier) {
        Some(RefNode::SimpleIdentifier(x)) => Some(x.nodes.0),
        Some(RefNode::EscapedIdentifier(x)) => Some(x.nodes.0),
        _ => None,
    }
}

fn to_primary(
    ast: &SyntaxTree,
    scope: &ModuleInfo,
    expr: &sv_parser::Expression,
) -> Option<Primary> {
    if let sv_parser::Expression::Primary(p) = expr {
        return match p.deref() {
            sv_parser::Primary::PrimaryLiteral(pl) => {
                let n = unwrap_node!(RefNode::PrimaryLiteral(pl), UnsignedNumber)?;
                get_number(n, ast).map(|i| Primary::Int(i))
            }
            sv_parser::Primary::Hierarchical(h) => {
                let var_str = get_identifier(RefNode::PrimaryHierarchical(h), ast)?;
                Some(Primary::Variable(Var {
                    name: var_str,
                    scope: scope.module_name.clone(),
                }))
            }
            _ => None,
        };
    }
    None
}

fn to_var(ast: &SyntaxTree, scope: &ModuleInfo, expr: &sv_parser::Expression) -> Option<Var> {
    let var_str = get_identifier(RefNode::Expression(expr), ast)?;
    Some(Var {
        name: var_str,
        scope: scope.module_name.clone(),
    })
}

fn to_op(ast: &SyntaxTree, op: &BinaryOperator) -> Option<BinRel> {
    let op_str = op.nodes.0.nodes.0.embody(ast);
    sv_info::parse_bin_rel(op_str)
}

trait Embodiment {
    fn embody<'a>(&self, ast: &'a SyntaxTree) -> &'a str;
}

impl Embodiment for Locate {
    fn embody<'a>(&self, ast: &'a SyntaxTree) -> &'a str {
        ast.get_str(self).unwrap()
    }
}
