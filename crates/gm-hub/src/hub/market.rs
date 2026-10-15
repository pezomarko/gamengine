//! The economy over the wire (ECONOMY.md): a session's operations on its character, and
//! a zone's.

use super::*;

/// `HubRequest::Econ`.
pub(super) async fn econ(
    hub: &Arc<Hub>,
    session: SessionId,
    character: CharacterId,
    op: EconOp,
) -> Result<HubResponse, HubError> {
    let account = hub.session_account(session)?;
    if !hub.econ_allowed(account) {
        return Err(HubError::Busy);
    }
    let row = hub
        .db
        .character(character)
        .await?
        .filter(|r| r.account_id == account)
        .ok_or(HubError::NotFound)?;
    let zone = match row.summary().location {
        LocationSummary::Zone(z) => Some(z),
        _ => None,
    };
    // What moves items without the zone's hand (MODES.md 11.2) is told to the
    // zone after: the storage, a stall of one's own, a craft, a trade (both sides).
    let moves_items = matches!(
        op,
        EconOp::StorageDeposit { .. }
            | EconOp::StorageWithdraw { .. }
            | EconOp::StallList { .. }
            | EconOp::StallUnlist { .. }
            | EconOp::StallClose
            | EconOp::Craft { .. }
            | EconOp::Decompose { .. }
            | EconOp::TradeAccept { .. }
            | EconOp::SetBar { .. }
    );
    let trade = match &op {
        EconOp::TradeAccept { trade, .. } => Some(*trade),
        _ => None,
    };
    let reply = econ_op(hub, character, zone, op).await?;
    if moves_items {
        if let (Some(trade), EconReply::Trade { committed: true }) = (trade, &reply)
            && let Ok((a, b)) = hub.econ.trade_characters(trade).await
        {
            tell_zone_of_items(hub, a).await;
            tell_zone_of_items(hub, b).await;
        } else if trade.is_none() {
            tell_zone_of_items(hub, character).await;
        }
    }
    Ok(HubResponse::Econ(reply))
}

/// `HubRequest::ZoneEcon`.
pub(super) async fn zone_econ(
    hub: &Arc<Hub>,
    auth: &ConnAuth,
    op: ZoneEconOp,
) -> Result<HubResponse, HubError> {
    let zone = hub.zone_of_conn(auth)?;
    Ok(HubResponse::Econ(zone_econ_op(hub, &zone, op).await?))
}

/// `HubRequest::ZoneParty`.
pub(super) async fn zone_party(
    hub: &Arc<Hub>,
    auth: &ConnAuth,
    op: ZonePartyOp,
) -> Result<HubResponse, HubError> {
    let zone = hub.zone_of_conn(auth)?;
    Ok(HubResponse::Party(zone_party_op(hub, &zone, op).await?))
}
