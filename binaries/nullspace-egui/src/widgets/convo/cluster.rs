use nullspace_client::internal::{ConvoMessage, MessageContent};
use nullspace_structs::timestamp::NanoTimestamp;

const CLUSTER_WINDOW_NANOS: u64 = 3 * 60 * 1_000_000_000;

pub fn cluster_convo(messages: &[ConvoMessage]) -> Vec<Vec<ConvoMessage>> {
    let mut clusters = Vec::new();
    let mut start = 0;

    while start < messages.len() {
        let first = &messages[start];
        let first_sender = &first.sender;
        let first_type = message_type(first);

        let Some(first_ts) = first.received_at else {
            clusters.push(messages[start..start + 1].to_vec());
            start += 1;
            continue;
        };

        let mut end = start + 1;
        while let Some(candidate) = messages.get(end) {
            let candidate_ts = if let Some(candidate_ts) = candidate.received_at {
                candidate_ts
            } else {
                NanoTimestamp::now()
            };

            if &candidate.sender != first_sender {
                break;
            }
            if message_type(candidate) != first_type {
                break;
            }
            if first_ts.0.abs_diff(candidate_ts.0) > CLUSTER_WINDOW_NANOS {
                break;
            }
            end += 1;
        }

        clusters.push(messages[start..end].to_vec());
        start = end;
    }

    clusters
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MessageType {
    PlainText,
    Markdown,
    Attachment,
    GroupInvite,
}

fn message_type(message: &ConvoMessage) -> MessageType {
    match &message.body {
        MessageContent::PlainText(_) => MessageType::PlainText,
        MessageContent::Markdown(_) => MessageType::Markdown,
        MessageContent::Attachment { .. } => MessageType::Attachment,
        MessageContent::GroupInvite { .. } => MessageType::GroupInvite,
    }
}
