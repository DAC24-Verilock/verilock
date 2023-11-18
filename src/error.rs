use crate::abstraction::sv_info::ModuleInstance;
use std::error::Error;
use std::fmt::{Display, Formatter, Result};

#[derive(Debug, PartialEq, Clone)]
pub struct Action {
    pub subject: ModuleInstance,
    pub action: String,
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{} fires {}", self.subject, self.action)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum VerilockError {
    NoChannelImplementation(NoChannelImplementation),
    UnsolvableConstraints(UnsolvableConstraints),
    DanglingSending(DanglingSending),
    DanglingReceiving(DanglingReceiving),
    LiveLock(LiveLock),
}

impl VerilockError {
    pub fn report(&self) {
        match self {
            VerilockError::NoChannelImplementation(e) => println!("{}", e),
            VerilockError::UnsolvableConstraints(e) => println!("{}", e),
            VerilockError::DanglingSending(e) => println!("{}", e),
            VerilockError::DanglingReceiving(e) => println!("{}", e),
            VerilockError::LiveLock(e) => println!("{}", e),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct NoChannelImplementation {
    pub id: String,
}

impl Display for NoChannelImplementation {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "there is no channel implementation for the provided channel identifier: {}",
            self.id
        )
    }
}

impl Error for NoChannelImplementation {}

#[derive(Debug, PartialEq, Clone)]
pub struct UnsolvableConstraints {
    pub constraints: Vec<String>,
}

impl Display for UnsolvableConstraints {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let constraint_lines = self.constraints.join("\n");
        write!(
            f,
            "the following constraints cannot be solved:\n{}",
            constraint_lines
        )
    }
}

impl Error for UnsolvableConstraints {}

#[derive(Debug, PartialEq, Clone)]
pub struct DanglingSending {
    pub trace: Vec<Action>,
    pub dangling: Action,
}

impl Display for DanglingSending {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let traces = self
            .trace
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<String>>()
            .join("\n");
        write!(
            f,
            "the trace of actions: \n{}\nwill cause the dangling sending: {}",
            traces, self.dangling
        )
    }
}

impl Error for DanglingSending {}

#[derive(Debug, PartialEq, Clone)]
pub struct DanglingReceiving {
    pub trace: Vec<Action>,
    pub dangling: Action,
}

impl Display for DanglingReceiving {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let traces = self
            .trace
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<String>>()
            .join("\n");
        write!(
            f,
            "the trace of actions: \n{}\nwill cause the dangling receiving: {}",
            traces, self.dangling
        )
    }
}

impl Error for DanglingReceiving {}

#[derive(Debug, PartialEq, Clone)]
pub struct LiveLock {
    pub module: ModuleInstance,
}

impl Display for LiveLock {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "the module {} is live-locked", self.module.to_string())
    }
}

impl Error for LiveLock {}
