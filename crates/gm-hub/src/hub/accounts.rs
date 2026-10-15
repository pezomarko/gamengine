//! Accounts and sessions (HUB.md 2): registering, logging in and out.

use super::*;

/// `HubRequest::Register`.
pub(super) async fn register(
    hub: &Arc<Hub>,
    remote: SocketAddr,
    email: String,
    password: String,
) -> Result<HubResponse, HubError> {
    if !hub.auth_allowed(remote.ip()) {
        return Err(HubError::Busy);
    }
    let email = normalize_email(&email)?;
    if password.chars().count() < 8 || password.len() > 256 {
        return Err(HubError::Invalid(
            "a password is 8 characters or more, and 256 bytes at most".into(),
        ));
    }
    let _permit = hub.hashing.try_acquire().map_err(|_| HubError::Busy)?;
    let hash = tokio::task::spawn_blocking(move || {
        let mut salt = [0u8; 16];
        rand::fill(&mut salt);
        Argon2::default()
            .hash_password_with_salt(password.as_bytes(), &salt)
            .map(|h: PasswordHash| h.to_string())
    })
    .await
    .map_err(|_| HubError::Internal)?
    .map_err(|_| HubError::Internal)?;
    let account = hub.db.create_account(&email, &hash).await?;
    let session = hub.new_session(account);
    info!(%email, account, "registered");
    Ok(HubResponse::Session { session, account })
}

/// `HubRequest::Login`.
pub(super) async fn login(
    hub: &Arc<Hub>,
    remote: SocketAddr,
    email: String,
    password: String,
) -> Result<HubResponse, HubError> {
    if !hub.auth_allowed(remote.ip()) {
        return Err(HubError::Busy);
    }
    let email = normalize_email(&email)?;
    // An email nobody registered costs what a wrong password costs: the answer's
    // time does not say which accounts exist.
    let found = hub.db.account_by_email(&email).await?;
    let (account, hash) = match found {
        Some((account, hash)) => (Some(account), hash),
        None => (None, hub.decoy_hash.clone()),
    };
    let _permit = hub.hashing.try_acquire().map_err(|_| HubError::Busy)?;
    let ok = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash)
            .map(|parsed| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok()
            })
            .unwrap_or(false)
    })
    .await
    .map_err(|_| HubError::Internal)?;
    let Some(account) = account.filter(|_| ok) else {
        return Err(HubError::Credentials);
    };
    // A banned account is told why and until when (ANTICHEAT.md 6), after the
    // password: the reason is the account's own business.
    hub.conduct.check(account).await?;
    let session = hub.new_session(account);
    // A ban that landed between the check and the session ends the session.
    if let Err(e) = hub.conduct.check(account).await {
        hub.state
            .lock()
            .unwrap()
            .sessions
            .retain(|_, s| s.account != account);
        return Err(e);
    }
    Ok(HubResponse::Session { session, account })
}

/// `HubRequest::Logout`.
pub(super) async fn logout(hub: &Arc<Hub>, session: SessionId) -> Result<HubResponse, HubError> {
    let account = hub.session_account(session)?;
    let others = {
        let mut st = hub.state.lock().unwrap();
        st.sessions.remove(&session);
        st.sessions.values().any(|s| s.account == account)
    };
    // The account is logged in somewhere else too: that client's play goes on.
    if others {
        return Ok(HubResponse::Ok);
    }
    // Characters still in a zone are kicked there and go offline here.
    let rows = hub.db.characters_of(account).await?;
    let in_zone: Vec<(CharacterId, ZoneId)> = rows
        .iter()
        .filter(|r| r.location_kind == "zone")
        .filter_map(|r| r.location_zone.clone().map(|z| (r.id, z)))
        .collect();
    // Characters in a live zone are saved and taken offline by that zone when it
    // handles the kick; everything else (transits, orphans) goes offline here.
    hub.db.offline_not_in_zone(account).await?;
    for (character, zone) in in_zone {
        hub.notify(
            &zone,
            HubNotice::Kick {
                character,
                reason: "logged out".into(),
            },
        )
        .await;
    }
    Ok(HubResponse::Ok)
}
