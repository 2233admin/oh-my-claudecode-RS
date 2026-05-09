use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InboxMessage {
    Message { content: String, timestamp: String },
    Context { content: String, timestamp: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboxMessage {
    Ready {
        timestamp: String,
    },
    TaskComplete {
        task_id: String,
        summary: String,
        timestamp: String,
    },
    TaskFailed {
        task_id: String,
        error: String,
        timestamp: String,
    },
    Idle {
        timestamp: String,
    },
    ShutdownAck {
        request_id: String,
        timestamp: String,
    },
    DrainAck {
        request_id: String,
        timestamp: String,
    },
    Heartbeat {
        timestamp: String,
    },
    Error {
        error: String,
        timestamp: String,
    },
    AllTasksComplete {
        timestamp: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShutdownSignal {
    pub request_id: String,
    pub reason: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DrainSignal {
    pub request_id: String,
    pub reason: String,
    pub timestamp: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inbox_message_serde_roundtrip() {
        let msg = InboxMessage::Message {
            content: "hello".into(),
            timestamp: "2026-05-10T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: InboxMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn outbox_message_serde_roundtrip() {
        let msg = OutboxMessage::TaskComplete {
            task_id: "t1".into(),
            summary: "done".into(),
            timestamp: "2026-05-10T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: OutboxMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn shutdown_signal_serde_roundtrip() {
        let sig = ShutdownSignal {
            request_id: "r1".into(),
            reason: "timeout".into(),
            timestamp: "2026-05-10T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&sig).unwrap();
        let back: ShutdownSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(sig, back);
    }

    #[test]
    fn drain_signal_serde_roundtrip() {
        let sig = DrainSignal {
            request_id: "d1".into(),
            reason: "scaling down".into(),
            timestamp: "2026-05-10T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&sig).unwrap();
        let back: DrainSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(sig, back);
    }
}
