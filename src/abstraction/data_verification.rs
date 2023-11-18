use crate::abstraction::sv_info::InterfaceInfo;
use crate::error::NoChannelImplementation;
use crate::task::ChannelIdentifier;

pub fn verify_channel_implementation(
    implementation: &Vec<InterfaceInfo>,
    provided: &ChannelIdentifier,
) -> Option<NoChannelImplementation> {
    for i in implementation {
        if i.interface_name == provided.channel_name
            && i.tasks.contains(&provided.receive_name)
            && i.tasks.contains(&provided.send_name)
        {
            return None;
        }
    }
    Some(NoChannelImplementation {
        id: provided.channel_name.to_string(),
    })
}
