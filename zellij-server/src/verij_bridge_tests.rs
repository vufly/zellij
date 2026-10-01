use super::{SessionState, Size};
use zellij_utils::ipc::VerijPaneRequest;

#[test]
fn verij_generation_invalidates_reused_client_id_and_excludes_cli() {
    let mut state = SessionState::new();
    let id = state.new_client();
    assert!(
        state.verij_identity(id).is_none(),
        "CLI/unattached socket is not a display"
    );
    state.set_client_data(
        id,
        Size {
            rows: 50,
            cols: 160,
        },
        false,
    );
    let first = state.verij_identity(id).unwrap().clone();
    state.remove_client(id);
    assert!(state.verij_identity(id).is_none());
    let reused = state.new_client();
    assert_eq!(id, reused);
    state.set_client_data(
        reused,
        Size {
            rows: 50,
            cols: 160,
        },
        false,
    );
    assert_ne!(state.verij_identity(reused).unwrap(), &first);
    let web = state.new_client();
    state.set_client_data(
        web,
        Size {
            rows: 50,
            cols: 160,
        },
        true,
    );
    assert!(state.verij_identity(web).is_none());
}

#[test]
fn verij_queued_requests_supersede_and_do_not_cross_attachment_reuse() {
    let mut state = SessionState::new();
    let id = state.new_client();
    state.set_client_data(
        id,
        Size {
            rows: 50,
            cols: 160,
        },
        false,
    );
    let first = VerijPaneRequest {
        request_id: "first".into(),
        target_client_id: id,
        connection_id: state.verij_identity(id).unwrap().clone(),
        pane_id: 0,
        sequence: 1,
        query_only: false,
    };
    assert_eq!(state.verij_latest_sequence(&first), Some(0));
    state.verij_reserve(&first).unwrap();
    let mut latest = first.clone();
    latest.request_id = "latest".into();
    latest.pane_id = 1;
    latest.sequence = 2;
    state.verij_reserve(&latest).unwrap();
    assert_eq!(state.verij_latest_sequence(&first), Some(2));
    assert_eq!(state.verij_validate_execution(&first), Err("superseded"));
    assert_eq!(state.verij_reserve(&first), Err("superseded"));
    assert_eq!(state.verij_validate_execution(&latest), Ok(()));
    let mut query = first.clone();
    query.query_only = true;
    query.sequence = 0;
    state.verij_reserve(&query).unwrap();
    assert_eq!(state.verij_latest_sequence(&query), Some(2));
    assert_eq!(
        state.verij_validate_execution(&latest),
        Ok(()),
        "query must not cancel focus"
    );
    state.remove_client(id);
    assert_eq!(state.verij_latest_sequence(&latest), None);
    assert_eq!(
        state.verij_validate_execution(&latest),
        Err("stale_attachment")
    );
    let reused = state.new_client();
    assert_eq!(reused, id);
    state.set_client_data(
        id,
        Size {
            rows: 50,
            cols: 160,
        },
        false,
    );
    assert_eq!(
        state.verij_validate_execution(&latest),
        Err("stale_attachment")
    );
    let mut replacement = first;
    replacement.connection_id = state.verij_identity(id).unwrap().clone();
    assert_eq!(state.verij_latest_sequence(&replacement), Some(0));
    state.verij_reserve(&replacement).unwrap();
    assert_eq!(
        state.verij_validate_execution(&replacement),
        Ok(()),
        "new generation resets sequence"
    );
    replacement.sequence = 0;
    assert_eq!(state.verij_reserve(&replacement), Err("invalid_request"));
    replacement.query_only = true;
    replacement.request_id.clear();
    assert_eq!(state.verij_reserve(&replacement), Err("invalid_request"));
}

#[test]
fn verij_cancelled_control_connection_cannot_execute_or_receive_on_reuse() {
    let mut state = SessionState::new();
    let requester = state.new_client();
    let token = state.attachment_generations[&requester].clone();
    assert!(state.verij_validate_requester(requester, &token));
    state.remove_client(requester);
    assert!(!state.verij_validate_requester(requester, &token));
    let replacement = state.new_client();
    assert_eq!(replacement, requester);
    assert!(!state.verij_validate_requester(replacement, &token));
    let fresh = state.attachment_generations[&replacement].clone();
    assert!(state.verij_validate_requester(replacement, &fresh));
    state.set_client_data(
        replacement,
        Size {
            rows: 50,
            cols: 160,
        },
        false,
    );
    assert!(
        !state.verij_validate_requester(replacement, &fresh),
        "display socket is not a control connection"
    );
}
