use jsonrpc_pubsub::typed::{Sink, Subscriber};
use jsonrpc_pubsub::SubscriptionId;
use std::collections::HashMap;
use std::ops;

pub struct Subscribers<T> {
    id: u64,
    subscriptions: HashMap<SubscriptionId, T>,
}

impl<T> Default for Subscribers<T> {
    fn default() -> Self {
        Self {
            id: 0,
            subscriptions: HashMap::new(),
        }
    }
}

impl<T> Subscribers<T> {
    fn next_id(&mut self) -> u64 {
        let id = self.id;
        self.id += 1;
        id
    }

    pub fn remove(&mut self, id: &SubscriptionId) -> Option<T> {
        self.subscriptions.remove(id)
    }
}

impl<T> Subscribers<Sink<T>> {
    pub fn add(&mut self, subscriber: Subscriber<T>) {
        let id = SubscriptionId::Number(self.next_id());
        if let Ok(sink) = subscriber.assign_id(id.clone()) {
            self.subscriptions.insert(id, sink);
        }
    }
}

impl<T> ops::Deref for Subscribers<T> {
    type Target = HashMap<SubscriptionId, T>;

    fn deref(&self) -> &Self::Target {
        &self.subscriptions
    }
}
