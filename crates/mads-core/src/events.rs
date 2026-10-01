use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::google::Issue;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Totals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: Option<f64>,
    pub missions: usize,
    pub failed_missions: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub seq: u64,
    /// RFC 3339, UTC.
    pub ts: String,
    pub event: Event,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    RunStarted {
        run_id: String,
        run_dir: String,
        provider: String,
        model: Option<String>,
    },
    MissionStarted {
        mission: String,
        attempt: u32,
    },
    AgentText {
        mission: String,
        text: String,
    },
    ToolCalled {
        mission: String,
        tool: String,
        summary: String,
    },
    ToolFinished {
        mission: String,
        tool: String,
        ok: bool,
        summary: String,
    },
    Usage {
        mission: String,
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: Option<f64>,
    },
    MissionFinished {
        mission: String,
        ok: bool,
        reason: Option<String>,
    },
    Step {
        name: String,
        detail: String,
    },
    Validation {
        errors: Vec<Issue>,
        warnings: Vec<Issue>,
    },
    UrlChecked {
        url: String,
        status: Option<u16>,
        ok: bool,
    },
    ArtifactWritten {
        path: String,
    },
    RunFinished {
        ok: bool,
        exit_code: i32,
        totals: Totals,
    },
}

/// Cloneable handle that stamps events with a sequence number and a timestamp.
#[derive(Clone)]
pub struct EventSink {
    tx: UnboundedSender<EventEnvelope>,
    seq: Arc<AtomicU64>,
}

impl EventSink {
    pub fn channel() -> (EventSink, UnboundedReceiver<EventEnvelope>) {
        let (tx, rx) = unbounded_channel();
        (
            EventSink {
                tx,
                seq: Arc::new(AtomicU64::new(0)),
            },
            rx,
        )
    }

    /// Events are best effort: a closed receiver must never fail the run.
    pub fn emit(&self, event: Event) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let ts = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default();
        let _ = self.tx.send(EventEnvelope { seq, ts, event });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_is_monotonic_and_ts_is_rfc3339() {
        let (sink, mut rx) = EventSink::channel();
        sink.emit(Event::Step {
            name: "a".into(),
            detail: String::new(),
        });
        sink.emit(Event::Step {
            name: "b".into(),
            detail: String::new(),
        });
        let (e1, e2) = (rx.try_recv().unwrap(), rx.try_recv().unwrap());
        assert_eq!((e1.seq, e2.seq), (1, 2));
        assert!(
            time::OffsetDateTime::parse(&e1.ts, &time::format_description::well_known::Rfc3339)
                .is_ok(),
            "{}",
            e1.ts
        );
    }

    #[test]
    fn clones_share_the_sequence() {
        let (sink, mut rx) = EventSink::channel();
        let other = sink.clone();
        sink.emit(Event::Step {
            name: "a".into(),
            detail: String::new(),
        });
        other.emit(Event::Step {
            name: "b".into(),
            detail: String::new(),
        });
        assert_eq!(rx.try_recv().unwrap().seq, 1);
        assert_eq!(rx.try_recv().unwrap().seq, 2);
    }

    #[test]
    fn json_is_tagged_with_snake_case_type() {
        let (sink, mut rx) = EventSink::channel();
        sink.emit(Event::ToolCalled {
            mission: "plan".into(),
            tool: "get_business".into(),
            summary: String::new(),
        });
        let json = serde_json::to_value(rx.try_recv().unwrap()).unwrap();
        assert_eq!(json["event"]["type"], "tool_called");
        assert_eq!(json["event"]["tool"], "get_business");
        assert!(json["seq"].is_u64());
    }

    #[test]
    fn emitting_after_the_receiver_is_dropped_does_not_panic() {
        let (sink, rx) = EventSink::channel();
        drop(rx);
        sink.emit(Event::Step {
            name: "a".into(),
            detail: String::new(),
        });
    }

    #[test]
    fn envelope_roundtrips() {
        let (sink, mut rx) = EventSink::channel();
        sink.emit(Event::RunFinished {
            ok: true,
            exit_code: 0,
            totals: Totals::default(),
        });
        let e = rx.try_recv().unwrap();
        let back: EventEnvelope =
            serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }
}
