use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;

use crate::live_data::{LiveDataEnvelope, LiveDataKind};

const RETIRED_SESSION_LIMIT: usize = 4;

pub(super) fn preferred_remote(
    remote_ids: &HashSet<String>,
    priorities: &HashMap<String, i32>,
) -> Option<String> {
    remote_ids
        .iter()
        .filter_map(|remote_id| {
            priorities
                .get(remote_id)
                .map(|priority| (*priority, remote_id))
        })
        .min()
        .map(|(_, remote_id)| remote_id.clone())
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct StreamKey {
    friend_id: String,
    kind: LiveDataKind,
}

#[derive(Debug)]
struct StreamSequence {
    session_id: String,
    sequence: u64,
    retired_sessions: VecDeque<String>,
}

#[derive(Default)]
pub(super) struct SequenceTracker(Mutex<HashMap<StreamKey, StreamSequence>>);

impl SequenceTracker {
    pub(super) fn accept(
        &self,
        friend_id: &str,
        envelope: &LiveDataEnvelope,
    ) -> Result<bool, String> {
        if envelope.sequence == 0 || uuid::Uuid::parse_str(&envelope.session_id).is_err() {
            return Ok(false);
        }

        let key = StreamKey {
            friend_id: friend_id.to_owned(),
            kind: envelope.kind(),
        };
        let mut streams = self.0.lock().map_err(|error| error.to_string())?;
        let Some(current) = streams.get_mut(&key) else {
            streams.insert(
                key,
                StreamSequence {
                    session_id: envelope.session_id.clone(),
                    sequence: envelope.sequence,
                    retired_sessions: VecDeque::new(),
                },
            );
            return Ok(true);
        };

        if current.session_id == envelope.session_id {
            if envelope.sequence <= current.sequence {
                return Ok(false);
            }
            current.sequence = envelope.sequence;
            return Ok(true);
        }
        if current.retired_sessions.contains(&envelope.session_id) {
            return Ok(false);
        }

        current
            .retired_sessions
            .push_back(current.session_id.clone());
        if current.retired_sessions.len() > RETIRED_SESSION_LIMIT {
            current.retired_sessions.pop_front();
        }
        current.session_id = envelope.session_id.clone();
        current.sequence = envelope.sequence;
        Ok(true)
    }

    pub(super) fn retain(&self, friend_ids: &[String]) -> Result<(), String> {
        let mut streams = self.0.lock().map_err(|error| error.to_string())?;
        streams.retain(|key, _| friend_ids.contains(&key.friend_id));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::{SequenceTracker, preferred_remote};
    use crate::cursor::{CursorPosition, CursorPositions};
    use crate::live_data::{LiveData, LiveDataEnvelope};

    fn envelope(session_id: &str, sequence: u64) -> LiveDataEnvelope {
        LiveDataEnvelope {
            session_id: session_id.to_owned(),
            sequence,
            data: LiveData::Cursor {
                positions: CursorPositions {
                    raw: CursorPosition::default(),
                    mapped: CursorPosition::default(),
                },
            },
        }
    }

    #[test]
    fn rejects_duplicate_out_of_order_and_retired_session_packets() {
        let tracker = SequenceTracker::default();
        let first = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();

        assert!(tracker.accept("friend", &envelope(&first, 2)).unwrap());
        assert!(!tracker.accept("friend", &envelope(&first, 2)).unwrap());
        assert!(!tracker.accept("friend", &envelope(&first, 1)).unwrap());
        assert!(tracker.accept("friend", &envelope(&second, 1)).unwrap());
        assert!(!tracker.accept("friend", &envelope(&first, 3)).unwrap());
        assert!(tracker.accept("friend", &envelope(&second, 2)).unwrap());
    }

    #[test]
    fn relay_duplicates_do_not_block_a_newer_stream_packet() {
        let tracker = SequenceTracker::default();
        let session = uuid::Uuid::new_v4().to_string();

        assert!(tracker.accept("friend", &envelope(&session, 7)).unwrap());
        assert!(!tracker.accept("friend", &envelope(&session, 7)).unwrap());
        assert!(tracker.accept("friend", &envelope(&session, 8)).unwrap());
    }

    #[test]
    fn preferred_remote_uses_priority_then_stable_id() {
        let sources = HashSet::from([
            "lower".to_owned(),
            "higher-b".to_owned(),
            "higher-a".to_owned(),
        ]);
        let priorities = HashMap::from([
            ("lower".to_owned(), 3),
            ("higher-b".to_owned(), 1),
            ("higher-a".to_owned(), 1),
        ]);

        assert_eq!(
            preferred_remote(&sources, &priorities).as_deref(),
            Some("higher-a")
        );
    }
}
