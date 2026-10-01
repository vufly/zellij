//! Generation and sequence policy shared by dispatch, execution and delivery.
use crate::SessionState;
use zellij_utils::ipc::VerijPaneRequest;

impl SessionState {
    pub(crate) fn verij_latest_sequence(&self, request: &VerijPaneRequest) -> Option<u64> {
        (self.verij_identity(request.target_client_id) == Some(&request.connection_id)).then(|| {
            self.verij_navigation_sequences
                .get(&request.target_client_id)
                .copied()
                .unwrap_or(0)
        })
    }
    pub(crate) fn verij_validate_requester(&self, requester: u16, generation: &str) -> bool {
        self.clients.get(&requester) == Some(&None)
            && self
                .attachment_generations
                .get(&requester)
                .map(String::as_str)
                == Some(generation)
    }

    pub(crate) fn verij_reserve(&mut self, request: &VerijPaneRequest) -> Result<(), &'static str> {
        if request.request_id.is_empty()
            || request.request_id.len() > 128
            || (request.query_only && request.sequence != 0)
            || (!request.query_only && request.sequence == 0)
        {
            return Err("invalid_request");
        }
        if self.verij_identity(request.target_client_id) != Some(&request.connection_id) {
            return Err("stale_attachment");
        }
        if !request.query_only {
            let latest = self
                .verij_navigation_sequences
                .entry(request.target_client_id)
                .or_default();
            if request.sequence <= *latest {
                return Err("superseded");
            }
            *latest = request.sequence;
        }
        Ok(())
    }

    pub(crate) fn verij_validate_execution(
        &self,
        request: &VerijPaneRequest,
    ) -> Result<(), &'static str> {
        if self.verij_identity(request.target_client_id) != Some(&request.connection_id) {
            return Err("stale_attachment");
        }
        if !request.query_only
            && self
                .verij_navigation_sequences
                .get(&request.target_client_id)
                != Some(&request.sequence)
        {
            return Err("superseded");
        }
        Ok(())
    }
}

/// Opt-in, debug-build-only instrumentation for disposable live race probes.
/// No registry lock may be held across a barrier. Release builds ignore it.
pub(crate) fn verij_probe_event(name: &str, mut event: serde_json::Value) {
    #[cfg(debug_assertions)]
    if let Some(directory) = std::env::var_os("VERIJ_ZELLIJ_PROBE_DIR") {
        if !name.is_empty()
            && name.len() <= 200
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        {
            if let Some(event) = event.as_object_mut() {
                event.insert(
                    "time_ns".into(),
                    serde_json::json!(std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                        .to_string()),
                );
            }
            let path = std::path::PathBuf::from(directory).join(format!("{name}.json"));
            let temporary = path.with_extension("tmp");
            if let Ok(bytes) = serde_json::to_vec(&event) {
                if std::fs::write(&temporary, bytes).is_ok() {
                    let _ = std::fs::rename(temporary, path);
                }
            }
        }
    }
    let _ = (name, event);
}

pub(crate) fn verij_probe_barrier(request: &VerijPaneRequest, phase: &str) -> bool {
    #[cfg(debug_assertions)]
    if let Some(directory) = std::env::var_os("VERIJ_ZELLIJ_PROBE_DIR") {
        if !request.request_id.is_empty()
            && request.request_id.len() <= 128
            && request
                .request_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
        {
            let name = format!("{}.{phase}", request.request_id);
            let hold = std::path::PathBuf::from(directory).join(format!("{name}.hold"));
            if hold.exists() {
                verij_probe_event(
                    &name,
                    serde_json::json!({"request": request, "phase": phase}),
                );
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
                while hold.exists() {
                    if std::time::Instant::now() >= deadline {
                        verij_probe_event(
                            &format!("{name}.expired"),
                            serde_json::json!({"expired": true}),
                        );
                        return false;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }
    }
    let _ = (request, phase);
    true
}

pub(crate) fn verij_probe_route_end(generation: &str) {
    #[cfg(debug_assertions)]
    {
        let request = VerijPaneRequest {
            request_id: generation.into(),
            target_client_id: 0,
            connection_id: String::new(),
            pane_id: 0,
            sequence: 0,
            query_only: true,
        };
        let _ = verij_probe_barrier(&request, "route-end");
    }
    let _ = generation;
}
