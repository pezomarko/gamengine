//! A session's characters (HUB.md 3): the list, a new one, its build, the content, the
//! zones, entering one, its trials.

use super::*;

/// `HubRequest::Characters`.
pub(super) async fn characters(
    hub: &Arc<Hub>,
    session: SessionId,
) -> Result<HubResponse, HubError> {
    let account = hub.session_account(session)?;
    let rows = hub.db.characters_of(account).await?;
    Ok(HubResponse::Characters(
        rows.iter().map(|r| r.summary()).collect(),
    ))
}

/// `HubRequest::CreateCharacter`.
pub(super) async fn create_character(
    hub: &Arc<Hub>,
    session: SessionId,
    name: String,
    build: BuildChoice,
) -> Result<HubResponse, HubError> {
    let account = hub.session_account(session)?;
    // What every client can draw, and no name another could be taken for.
    let name =
        gm_hub_proto::names::character_name(&name).map_err(|why| HubError::Invalid(why.into()))?;
    let key = gm_hub_proto::names::skeleton(&name);
    let build = hub.resolve_build(build)?;
    let row = hub
        .db
        .create_character(account, &name, &key, &build)
        .await?;
    Ok(HubResponse::Character(row.summary()))
}

/// `HubRequest::Content`.
pub(super) async fn content(hub: &Arc<Hub>, session: SessionId) -> Result<HubResponse, HubError> {
    hub.session_account(session)?;
    // A blurb for every preset, whatever the configuration gave.
    let mut blurbs = hub.cfg.blurbs.clone();
    blurbs.resize(hub.cfg.content.builds.len(), String::new());
    Ok(HubResponse::Content {
        pack: hub.cfg.content.clone(),
        blurbs,
    })
}

/// `HubRequest::ListZones`.
pub(super) async fn list_zones(
    hub: &Arc<Hub>,
    session: SessionId,
) -> Result<HubResponse, HubError> {
    hub.session_account(session)?;
    let st = hub.state.lock().unwrap();
    let mut zones: Vec<ZoneSummary> = st
        .zones
        .iter()
        .filter(|(_, z)| z.last_heartbeat.elapsed() < ZONE_STALE)
        .map(|(id, z)| ZoneSummary {
            id: id.clone(),
            map: z.map.clone(),
            players: z.players,
            max_players: z.max_players,
            web: z.web.is_some(),
            min_trust: z.min_trust,
            requires: z.requires.clone(),
            addr: z.addr,
            cert_hash: fnv1a64(&z.cert_der),
            up_secs: z.since.elapsed().as_secs(),
        })
        .collect();
    zones.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(HubResponse::Zones(zones))
}

/// `HubRequest::Enter`.
pub(super) async fn enter(
    hub: &Arc<Hub>,
    conn: &Link,
    session: SessionId,
    character: CharacterId,
    zone: ZoneId,
) -> Result<HubResponse, HubError> {
    let account = hub.session_account(session)?;
    let row = hub
        .db
        .character(character)
        .await?
        .filter(|r| r.account_id == account)
        .ok_or(HubError::NotFound)?;
    // A build the content no longer takes is repaired here (MATRIX.md 9.1) and
    // stored, so that a character outlives a change of the rules; one nothing can
    // be made of is refused in words, and not at a zone's door after the claim.
    let row = match row.build.repaired(&hub.cfg.content) {
        None => row,
        Some(build) => {
            info!(character = row.id, "build repaired to the current rules");
            hub.db.set_build(account, row.id, &build).await?;
            crate::db::CharacterRow { build, ..row }
        }
    };
    row.build.validate(&hub.cfg.content).map_err(|e| {
        HubError::Invalid(format!("this character's build is no longer valid: {e}"))
    })?;
    let web = conn.is_web();
    let zone = if zone.is_empty() {
        // Where it was, else the start zone, else whatever will have it: the
        // first of them whose gate the character passes. With none, the refusal
        // of the first is the one to tell.
        let mut refusal = HubError::NotFound;
        let mut found = None;
        for (i, candidate) in hub
            .default_zones(row.pos_zone.as_deref(), web)
            .into_iter()
            .enumerate()
        {
            let open = match hub.zone_open(&candidate, web, character).await {
                Ok(()) => hub.gate(&candidate, character).await,
                Err(e) => Err(e),
            };
            match open {
                Ok(()) => {
                    found = Some(candidate);
                    break;
                }
                Err(e) if i == 0 => refusal = e,
                Err(_) => {}
            }
        }
        found.ok_or(refusal)?
    } else {
        hub.zone_open(&zone, web, character).await?;
        hub.gate(&zone, character).await?;
        zone
    };
    hub.db.begin_enter(account, character, &zone).await?;
    Ok(HubResponse::Ticket(
        hub.ticket_for(&zone, account, character)?,
    ))
}

/// `HubRequest::Trials`.
pub(super) async fn trials(
    hub: &Arc<Hub>,
    session: SessionId,
    character: CharacterId,
) -> Result<HubResponse, HubError> {
    let account = hub.session_account(session)?;
    hub.db
        .character(character)
        .await?
        .filter(|r| r.account_id == account)
        .ok_or(HubError::NotFound)?;
    Ok(HubResponse::Trials(hub.db.trials_of(character).await?))
}
