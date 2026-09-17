//! One render per flight at a time. Drawing a preview costs a track decode, a
//! satellite fetch and a rasterization, so a burst of crawlers hitting a cold
//! flight should produce one picture, not one each.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::broadcast;

/// Handle to the in-flight renders. Cheap to clone; lives on `AppState`.
#[derive(Clone, Default)]
pub struct RenderGate {
    inflight: Arc<Mutex<HashMap<String, broadcast::Sender<()>>>>,
}

pub enum Turn {
    /// Nobody else is drawing this flight. Hold the lease until the image is
    /// stored; dropping it releases the followers.
    Render(RenderLease),
    /// Someone else got there first. Await this, then read the stored row.
    Wait(broadcast::Receiver<()>),
}

impl RenderGate {
    pub fn enter(&self, flight_id: &str) -> Turn {
        let mut inflight = self.lock();
        match inflight.get(flight_id) {
            Some(leader) => Turn::Wait(leader.subscribe()),
            None => {
                let (tx, _) = broadcast::channel(1);
                inflight.insert(flight_id.to_owned(), tx.clone());
                Turn::Render(RenderLease {
                    gate: self.clone(),
                    flight_id: flight_id.to_owned(),
                    _tx: tx,
                })
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, broadcast::Sender<()>>> {
        self.inflight.lock().expect("render gate mutex poisoned")
    }
}

pub struct RenderLease {
    gate: RenderGate,
    flight_id: String,
    /// The only other sender. Followers wake on the channel closing, so they're
    /// released even when the render panics or bails out early with `?`.
    _tx: broadcast::Sender<()>,
}

impl Drop for RenderLease {
    fn drop(&mut self) {
        self.gate.lock().remove(&self.flight_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_render(turn: &Turn) -> bool {
        matches!(turn, Turn::Render(_))
    }

    #[tokio::test]
    async fn the_second_caller_waits_and_the_first_releases_it() {
        let gate = RenderGate::default();

        let leader = gate.enter("LEO-11");
        let Turn::Wait(mut follower) = gate.enter("LEO-11") else {
            panic!("the second caller should wait");
        };

        drop(leader);

        // Closed, not a value: the lease signals by going away.
        assert!(follower.recv().await.is_err());
        assert!(gate.lock().is_empty(), "the lease cleans up after itself");
    }

    #[tokio::test]
    async fn another_flight_renders_in_parallel() {
        let gate = RenderGate::default();

        let _leader = gate.enter("LEO-11");

        assert!(is_render(&gate.enter("LEO-12")));
    }

    #[tokio::test]
    async fn a_flight_can_be_rendered_again_once_the_lease_is_gone() {
        let gate = RenderGate::default();

        drop(gate.enter("LEO-11"));

        assert!(is_render(&gate.enter("LEO-11")));
    }
}
