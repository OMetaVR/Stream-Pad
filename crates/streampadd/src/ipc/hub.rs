//! Connected clients and their outgoing queues. Every send is a `try_send`: a client whose queue is full is
//! dropped instead of ever stalling the daemon.

use crate::core::ClientId;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use streampad_core::ipc::{EventMsg, Response};
use tokio::sync::{Notify, mpsc};

/// Messages a client may have queued before it counts as too slow.
pub const CLIENT_QUEUE: usize = 1024;

struct Slot {
    tx: mpsc::Sender<Arc<str>>,
    subscribed: bool,
    trace: bool,
    kicked: Arc<Notify>,
}

/// A newly connected client: its id, its outgoing queue, and a signal that fires if the hub drops it.
pub struct Registration {
    pub id: ClientId,
    pub outbox: mpsc::Receiver<Arc<str>>,
    pub kicked: Arc<Notify>,
}

#[derive(Default)]
pub struct Hub {
    clients: Mutex<HashMap<ClientId, Slot>>,
    last_id: AtomicU64,
}

pub fn encode_line(msg: &impl Serialize) -> Arc<str> {
    let mut s = serde_json::to_string(msg).unwrap_or_else(|e| format!(r#"{{"event":"log","data":{{"level":"error","message":"encode: {e}"}}}}"#));
    s.push('\n');
    s.into()
}

impl Hub {
    pub fn register(&self) -> Registration {
        let id = self.last_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, outbox) = mpsc::channel(CLIENT_QUEUE);
        let kicked = Arc::new(Notify::new());
        let slot = Slot { tx, subscribed: false, trace: false, kicked: kicked.clone() };
        self.lock().insert(id, slot);
        Registration { id, outbox, kicked }
    }

    pub fn unregister(&self, id: ClientId) {
        self.lock().remove(&id);
    }

    pub fn subscribe(&self, id: ClientId, subscribed: bool, trace: bool) {
        if let Some(slot) = self.lock().get_mut(&id) {
            slot.subscribed = subscribed;
            slot.trace = subscribed && trace;
        }
    }

    pub fn respond(&self, id: ClientId, response: &Response) {
        let line = encode_line(response);
        let mut clients = self.lock();
        if let Some(slot) = clients.get(&id)
            && !deliver(slot, line)
        {
            kick(&mut clients, id);
        }
    }

    /// Send an event to every subscribed client (only `trace: true` ones when `trace_only`).
    pub fn broadcast(&self, event: &str, data: Value, trace_only: bool) {
        let mut clients = self.lock();
        let wanted = |s: &Slot| s.subscribed && (!trace_only || s.trace);
        if !clients.values().any(wanted) {
            return;
        }
        let line = encode_line(&EventMsg { event: event.to_string(), data });
        let slow: Vec<ClientId> =
            clients.iter().filter(|(_, s)| wanted(s)).filter(|(_, s)| !deliver(s, line.clone())).map(|(id, _)| *id).collect();
        for id in slow {
            kick(&mut clients, id);
        }
    }

    pub fn has_subscribers(&self) -> bool {
        self.lock().values().any(|s| s.subscribed)
    }

    /// Disconnect everyone (shutdown).
    pub fn close_all(&self) {
        let mut clients = self.lock();
        for slot in clients.values() {
            slot.kicked.notify_one();
        }
        clients.clear();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<ClientId, Slot>> {
        self.clients.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn deliver(slot: &Slot, line: Arc<str>) -> bool {
    match slot.tx.try_send(line) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(_)) => false,
        // The connection task is already gone; it unregisters itself.
        Err(mpsc::error::TrySendError::Closed(_)) => true,
    }
}

fn kick(clients: &mut HashMap<ClientId, Slot>, id: ClientId) {
    if let Some(slot) = clients.remove(&id) {
        log::warn!("dropping IPC client {id}: it is not reading its messages");
        slot.kicked.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn slow_client_is_dropped_others_keep_receiving() {
        let hub = Hub::default();
        let mut fast = hub.register();
        let slow = hub.register();
        hub.subscribe(fast.id, true, false);
        hub.subscribe(slow.id, true, false);
        for i in 0..CLIENT_QUEUE + 5 {
            hub.broadcast("state", json!({ "n": i }), false);
            while fast.outbox.try_recv().is_ok() {}
        }
        assert!(hub.lock().contains_key(&fast.id));
        assert!(!hub.lock().contains_key(&slow.id), "the slow client must be dropped");
        hub.broadcast("state", json!({}), false);
        assert!(fast.outbox.try_recv().is_ok());
    }

    #[test]
    fn trace_events_only_reach_trace_clients() {
        let hub = Hub::default();
        let mut plain = hub.register();
        let mut tracer = hub.register();
        hub.subscribe(plain.id, true, false);
        hub.subscribe(tracer.id, true, true);
        hub.broadcast("trace", json!({ "runId": 1, "nodeId": "a" }), true);
        assert!(plain.outbox.try_recv().is_err());
        assert!(tracer.outbox.try_recv().unwrap().contains("\"nodeId\":\"a\""));
    }

    #[test]
    fn unsubscribed_clients_get_responses_but_no_events() {
        let hub = Hub::default();
        let mut c = hub.register();
        hub.broadcast("state", json!({}), false);
        assert!(c.outbox.try_recv().is_err());
        hub.respond(c.id, &Response::ok(3, json!({})));
        assert_eq!(&*c.outbox.try_recv().unwrap(), "{\"id\":3,\"result\":{}}\n");
    }
}
