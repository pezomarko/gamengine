//! What a registered zone asks (HUB.md 3.2, 3.3): its hello and heartbeat, claims, saves,
//! handoffs, trials and conduct.

use super::*;
use gm_hub_proto::protocol::{AimStats, CharacterState, ReportReason, SessionToken};

/// A zone's `HubRequest::ZoneHello`, as one value.
pub(super) struct Hello {
    pub secret: String,
    pub zone: ZoneId,
    pub map: String,
    pub map_hash: u64,
    pub addr: SocketAddr,
    pub cert_der: Vec<u8>,
    pub web: Option<WebAddr>,
    pub min_trust: i16,
    pub requires: Vec<String>,
    pub max_players: u32,
}

/// The trial whose pass wakes the Essence.
const RITE: &str = "rite";

/// `HubRequest::Trial`.
pub(super) async fn trial(
    hub: &Arc<Hub>,
    auth: &ConnAuth,
    character: CharacterId,
    trial: String,
    secs: u32,
) -> Result<HubResponse, HubError> {
    // A zone speaks only for characters playing in it, and only about the trials
    // of its own map.
    let zone = hub.zone_of_conn(auth)?;
    if hub.db.zone_of(character).await?.as_ref() != Some(&zone) {
        return Err(HubError::Unauthorized);
    }
    let map = hub
        .state
        .lock()
        .unwrap()
        .zones
        .get(&zone)
        .map(|z| z.map.clone())
        .ok_or(HubError::Unauthorized)?;
    if !hub
        .cfg
        .content
        .trials
        .iter()
        .any(|t| t.key == trial && t.map == map)
    {
        return Err(HubError::Invalid(format!(
            "{trial:?} is not a trial of the map {map:?}"
        )));
    }
    hub.db.trial_pass(character, &trial, &zone, secs).await?;
    hub.db
        .log(&zone, "trial", &format!("{character} {trial} {secs}s"))
        .await;
    // A step of the ladder opens a cell of the kit at once (SKILLS.md 5).
    if gm_hub_proto::protocol::LADDER
        .iter()
        .any(|(step, _)| *step == trial)
    {
        super::tell_zone_of_items(hub, character).await;
    }
    // The rite (SKILLS.md 4): the Essence wakes in the weapon in hand.
    if trial == RITE
        && let Some(form) = hub
            .econ
            .awaken(character, |t| super::form_of(hub, t))
            .await
            .map_err(super::econ_err)?
    {
        hub.db
            .log(&zone, "essence", &format!("{character} {form}"))
            .await;
        super::tell_zone_of_items(hub, character).await;
    }
    Ok(HubResponse::Ok)
}

/// `HubRequest::ZoneHello`.
pub(super) async fn zone_hello(
    hub: &Arc<Hub>,
    auth: &ConnAuth,
    conn: &Link,
    remote: SocketAddr,
    hello: Hello,
) -> Result<HubResponse, HubError> {
    let Hello {
        secret,
        zone,
        map,
        map_hash,
        addr,
        cert_der,
        web,
        min_trust,
        requires,
        max_players,
    } = hello;
    if secret != hub.cfg.zone_secret || hub.cfg.zone_secret.is_empty() {
        warn!(%remote, %zone, "zone hello with a wrong secret");
        return Err(HubError::Unauthorized);
    }
    if valid_name(&zone).is_none() {
        return Err(HubError::Invalid("zone id".into()));
    }
    // (A zone that takes nobody would be full for good.)
    if max_players == 0 {
        return Err(HubError::Invalid("a zone takes one client at least".into()));
    }
    if requires.len() > MAX_ZONE_REQUIRES
        || requires
            .iter()
            .any(|r| !hub.cfg.content.trials.iter().any(|t| &t.key == r))
    {
        return Err(HubError::Invalid(
            "the zone requires an unknown trial".into(),
        ));
    }
    // A restarted zone has lost its players: they go offline and re-enter.
    let orphaned = hub.db.offline_zone(&zone).await?;
    let previous = {
        let mut st = hub.state.lock().unwrap();
        let previous = st.zones.remove(&zone).map(|z| z.conn);
        st.zones.insert(
            zone.clone(),
            ZoneEntry {
                addr,
                cert_der,
                web,
                min_trust,
                max_players,
                map: map.clone(),
                map_hash,
                players: 0,
                tick_mean_us: 0.0,
                since: Instant::now(),
                last_heartbeat: Instant::now(),
                conn: conn.clone(),
                requires,
            },
        );
        previous
    };
    if let Some(old) = previous
        && old.stable_id() != conn.stable_id()
    {
        old.close(1, b"replaced by a new zone process");
    }
    hub.verifier.lock().unwrap().insert(
        zone.clone(),
        TokenVerifier::new(hub.cfg.key.public_key(), &zone).map_err(|_| HubError::Internal)?,
    );
    *auth.zone.lock().unwrap() = Some(zone.clone());
    hub.db
        .log(
            &zone,
            "hello",
            &format!("{map} {map_hash:016x} {addr} orphaned={orphaned}"),
        )
        .await;
    info!(%zone, %map, %addr, orphaned, "zone registered");
    Ok(HubResponse::Registered {
        public_key: hub.cfg.key.public_key(),
    })
}

/// `HubRequest::Heartbeat`.
pub(super) async fn heartbeat(
    hub: &Arc<Hub>,
    auth: &ConnAuth,
    players: u32,
    tick_mean_us: f32,
) -> Result<HubResponse, HubError> {
    let zone = hub.zone_of_conn(auth)?;
    let mut st = hub.state.lock().unwrap();
    if let Some(z) = st.zones.get_mut(&zone) {
        z.players = players;
        z.tick_mean_us = tick_mean_us;
        z.last_heartbeat = Instant::now();
    }
    Ok(HubResponse::Ok)
}

/// `HubRequest::Claim`.
pub(super) async fn claim(
    hub: &Arc<Hub>,
    auth: &ConnAuth,
    token: SessionToken,
) -> Result<HubResponse, HubError> {
    let zone = hub.zone_of_conn(auth)?;
    let payload = {
        let mut v = hub.verifier.lock().unwrap();
        let verifier = v.get_mut(&zone).ok_or(HubError::Unauthorized)?;
        verifier
            .accept(&token, now_secs())
            .map_err(|e| HubError::Invalid(e.to_string()))?
    };
    // A ticket issued before a ban does not outlive it.
    hub.conduct.check(payload.account).await?;
    let row = hub.db.claim(payload.character, &zone).await?;
    // A handoff: the origin zone drops its ghost.
    if let Some(from) = &row.location_zone
        && from != &zone
    {
        hub.notify(from, HubNotice::Claimed { character: row.id })
            .await;
    }
    let mut state = row.state();
    if state.zone.as_deref() != Some(zone.as_str()) {
        // The saved position is on another zone's map, or there is none: spawn here.
        state.zone = None;
    }
    // What follows can fail after the character has been moved here; the zone would
    // hear "claim failed" and have nothing to give back, so the hub does.
    let rest = async {
        // It is somewhere else now: whatever trade it had open is called off
        // (PARTY.md 6): a trade is between two that stand together.
        hub.econ.trades_end_of(row.id).await.map_err(econ_err)?;
        // The owner is playing this character now: every hire of it as an avatar
        // ends (ECONOMY.md 11), and the zones its hirers play in are told.
        for (hire, hirer) in hub.econ.end_hires_of(row.id).await.map_err(econ_err)? {
            if let Some(z) = hub.db.zone_of(hirer).await? {
                hub.notify(&z, HubNotice::HireEnded { hirer, hire }).await;
            }
        }
        let squad = hub
            .hired(row.id, row.build.squad_capacity(&hub.cfg.content))
            .await?;
        // Read after the character became this zone's (ITEMS.md 3.3): whatever it
        // put on through the zone it came from is in it.
        let gear = hub
            .econ
            .gear(row.id, &hub.cfg.items)
            .await
            .map(reading)
            .map_err(econ_err)?;
        Ok::<_, HubError>((
            squad,
            hub.models.worn(row.id).await?,
            gear,
            // As gear: read after the character became this zone's. A change
            // that comes later is told to this zone, with a larger number.
            hub.parties.reading(row.id).await?,
            // A moderator's character is a game master there (GM.md 1).
            hub.models.is_moderator(row.account_id).await?,
        ))
    }
    .await;
    match rest {
        Ok((squad, model, gear, party, gm)) => Ok(HubResponse::Claimed {
            character: row.id,
            name: row.name.clone(),
            state,
            team: 0,
            model,
            squad,
            gear,
            party,
            gm,
        }),
        Err(e) => {
            let _ = hub.db.release(row.id, &zone).await;
            Err(e)
        }
    }
}

/// `HubRequest::Release`.
pub(super) async fn release(
    hub: &Arc<Hub>,
    auth: &ConnAuth,
    character: CharacterId,
) -> Result<HubResponse, HubError> {
    let zone = hub.zone_of_conn(auth)?;
    if hub.db.release(character, &zone).await? {
        info!(%zone, character, "a character the zone had no body for is offline again");
    }
    Ok(HubResponse::Ok)
}

/// `HubRequest::Save`.
pub(super) async fn save(
    hub: &Arc<Hub>,
    auth: &ConnAuth,
    character: CharacterId,
    state: CharacterState,
    leaving: bool,
) -> Result<HubResponse, HubError> {
    let zone = hub.zone_of_conn(auth)?;
    state
        .build
        .validate(&hub.cfg.content)
        .map_err(|e| HubError::Invalid(e.to_string()))?;
    hub.db.save(character, &zone, &state, leaving).await?;
    // Play time is what makes an account established (ANTICHEAT.md 6): looked at
    // when a character leaves, whether or not it ever shot at anybody.
    if leaving {
        if let Some(row) = hub.db.character(character).await? {
            hub.conduct.promote(row.account_id).await?;
        }
        return Ok(HubResponse::Ok);
    }
    // The number of what the hub holds of its party: a zone that missed a notice
    // sees that it is behind, and asks (PARTY.md 3.2).
    Ok(HubResponse::Saved {
        party: hub.parties.seq_of(character).await?,
    })
}

/// `HubRequest::Handoff`.
pub(super) async fn handoff(
    hub: &Arc<Hub>,
    auth: &ConnAuth,
    character: CharacterId,
    state: CharacterState,
    to_zone: ZoneId,
    web: bool,
) -> Result<HubResponse, HubError> {
    let zone = hub.zone_of_conn(auth)?;
    if to_zone == zone {
        return Err(HubError::Invalid("already there".into()));
    }
    // (The zone says whether its traveller is a browser.)
    hub.zone_open(&to_zone, web, character).await?;
    state
        .build
        .validate(&hub.cfg.content)
        .map_err(|e| HubError::Invalid(e.to_string()))?;
    hub.gate(&to_zone, character).await?;
    hub.db
        .begin_handoff(character, &zone, &to_zone, &state)
        .await?;
    let row = hub
        .db
        .character(character)
        .await?
        .ok_or(HubError::NotFound)?;
    Ok(HubResponse::Ticket(hub.ticket_for(
        &to_zone,
        row.account_id,
        character,
    )?))
}

/// Zones: conduct (ANTICHEAT.md 8).
/// `HubRequest::ZoneAim`.
pub(super) async fn zone_aim(
    hub: &Arc<Hub>,
    auth: &ConnAuth,
    nonce: u64,
    character: CharacterId,
    stats: AimStats,
) -> Result<HubResponse, HubError> {
    let zone = hub.zone_of_conn(auth)?;
    // A zone speaks for the characters that play or played in it (a leaver's
    // numbers arrive after it went): the character must exist, no more.
    if hub.db.character(character).await?.is_none() {
        return Err(HubError::NotFound);
    }
    hub.conduct.aim(&zone, nonce, character, &stats).await?;
    Ok(HubResponse::Ok)
}

/// `HubRequest::ZoneReport`.
pub(super) async fn zone_report(
    hub: &Arc<Hub>,
    auth: &ConnAuth,
    reporter: CharacterId,
    target: CharacterId,
    reason: ReportReason,
) -> Result<HubResponse, HubError> {
    let zone = hub.zone_of_conn(auth)?;
    if hub.db.zone_of(reporter).await?.as_deref() != Some(zone.as_str()) {
        return Err(HubError::Unauthorized);
    }
    let id = hub
        .conduct
        .report_open(&zone, reporter, target, reason)
        .await?;
    Ok(HubResponse::ReportOpened { id })
}
