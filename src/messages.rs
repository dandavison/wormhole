use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use tokio::sync::watch;

type ConsumerId = u64;

lazy_static! {
    static ref STORE: Mutex<MessageStore> = Mutex::new(MessageStore {
        consumers: HashMap::new(),
        next_id: 1,
    });
    static ref VERSION: (watch::Sender<u64>, watch::Receiver<u64>) = watch::channel(0);
}

fn notify_change() {
    VERSION.0.send_modify(|v| *v = v.wrapping_add(1));
}

pub fn subscribe() -> watch::Receiver<u64> {
    VERSION.1.clone()
}

struct Consumer {
    project: String,
    role: String,
    queue: Vec<Notification>,
}

struct MessageStore {
    consumers: HashMap<ConsumerId, Consumer>,
    next_id: ConsumerId,
}

pub struct Store<'a>(MutexGuard<'a, MessageStore>);

pub fn lock() -> Store<'static> {
    Store(STORE.lock().unwrap())
}

impl<'a> Store<'a> {
    pub fn find_or_register(&mut self, project: &str, role: &str) -> ConsumerId {
        for (&id, c) in &self.0.consumers {
            if c.project == project && c.role == role {
                return id;
            }
        }
        let id = self.0.next_id;
        self.0.next_id += 1;
        self.0.consumers.insert(
            id,
            Consumer {
                project: project.to_string(),
                role: role.to_string(),
                queue: Vec::new(),
            },
        );
        id
    }

    pub fn drain(&mut self, id: ConsumerId) -> Vec<Notification> {
        self.0
            .consumers
            .get_mut(&id)
            .map(|c| std::mem::take(&mut c.queue))
            .unwrap_or_default()
    }

    pub fn publish(&mut self, project: &str, target: &Target, notification: Notification) {
        for consumer in self.0.consumers.values_mut() {
            if consumer.project != project {
                continue;
            }
            if let Target::Role(role) = target {
                if consumer.role != *role {
                    continue;
                }
            }
            consumer.queue.push(notification.clone());
        }
        notify_change();
    }

    pub fn has_messages(&self, id: ConsumerId) -> bool {
        self.0
            .consumers
            .get(&id)
            .map_or(false, |c| !c.queue.is_empty())
    }

    #[allow(dead_code)]
    pub fn remove_project(&mut self, project: &str) {
        self.0.consumers.retain(|_, c| c.project != project);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl Notification {
    pub fn new(method: &str) -> Self {
        Notification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: None,
        }
    }
}

pub enum Target {
    Role(String),
    Broadcast,
}

#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub target: String,
    pub message: Notification,
}

impl PublishRequest {
    pub fn into_parts(self) -> (Target, Notification) {
        let target = if self.target == "*" {
            Target::Broadcast
        } else {
            Target::Role(self.target)
        };
        (target, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_drain() {
        let mut store = lock();
        let id = store.find_or_register("msg-test-1", "editor");
        store.publish(
            "msg-test-1",
            &Target::Role("editor".to_string()),
            Notification::new("editor/close"),
        );
        let msgs = store.drain(id);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].method, "editor/close");
        assert!(store.drain(id).is_empty());
    }

    #[test]
    fn test_publish_wrong_role() {
        let mut store = lock();
        let id = store.find_or_register("msg-test-2", "editor");
        store.publish(
            "msg-test-2",
            &Target::Role("cli".to_string()),
            Notification::new("cli/something"),
        );
        assert!(store.drain(id).is_empty());
    }

    #[test]
    fn test_publish_wrong_project() {
        let mut store = lock();
        let id = store.find_or_register("msg-test-3a", "editor");
        store.publish(
            "msg-test-3b",
            &Target::Role("editor".to_string()),
            Notification::new("editor/close"),
        );
        assert!(store.drain(id).is_empty());
    }

    #[test]
    fn test_message_survives_between_polls() {
        let mut store = lock();
        let id = store.find_or_register("msg-test-gap", "editor");
        assert!(store.drain(id).is_empty());
        store.publish(
            "msg-test-gap",
            &Target::Role("editor".to_string()),
            Notification::new("editor/close"),
        );
        let id2 = store.find_or_register("msg-test-gap", "editor");
        assert_eq!(id, id2);
        let msgs = store.drain(id2);
        assert_eq!(
            msgs.len(),
            1,
            "message published between polls must not be lost"
        );
    }

    #[test]
    fn test_message_delivered_during_long_poll() {
        let mut store = lock();
        let id = store.find_or_register("msg-test-longpoll", "editor");
        store.publish(
            "msg-test-longpoll",
            &Target::Role("editor".to_string()),
            Notification::new("editor/close"),
        );
        let msgs = store.drain(id);
        assert_eq!(msgs.len(), 1, "message must be delivered during long-poll");
    }

    #[test]
    fn test_broadcast() {
        let mut store = lock();
        let id1 = store.find_or_register("msg-test-5", "editor");
        let id2 = store.find_or_register("msg-test-5", "cli");
        store.publish(
            "msg-test-5",
            &Target::Broadcast,
            Notification::new("test/broadcast"),
        );
        assert_eq!(store.drain(id1).len(), 1);
        assert_eq!(store.drain(id2).len(), 1);
    }

    #[test]
    fn test_remove_project() {
        let mut store = lock();
        let id = store.find_or_register("msg-test-rm", "editor");
        store.publish(
            "msg-test-rm",
            &Target::Role("editor".to_string()),
            Notification::new("editor/close"),
        );
        store.remove_project("msg-test-rm");
        assert!(store.drain(id).is_empty());
        assert_eq!(store.find_or_register("msg-test-rm", "editor"), id + 1);
    }
}
