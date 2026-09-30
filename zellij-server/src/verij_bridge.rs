//! Generation and sequence policy shared by dispatch, execution and delivery.
use crate::SessionState;
use zellij_utils::ipc::VerijPaneRequest;

impl SessionState {
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
