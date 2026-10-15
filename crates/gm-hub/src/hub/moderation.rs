//! A moderator's acts (MODELS.md 7, ANTICHEAT.md 7).

use super::*;

/// `HubRequest::Mod`.
pub(super) async fn moderate(
    hub: &Arc<Hub>,
    session: SessionId,
    op: ModOp,
) -> Result<HubResponse, HubError> {
    let moderator = hub.moderator(session).await?;
    match op {
        ModOp::Queue { limit } => Ok(HubResponse::ModQueue(hub.models.queue(limit).await?)),
        ModOp::Preview { .. } => Err(HubError::Internal),
        ModOp::Decide {
            model,
            approve,
            code,
            reason,
        } => {
            hub.models
                .decide(moderator, &model, approve, code, &reason)
                .await?;
            Ok(HubResponse::Ok)
        }
        ModOp::Takedown {
            model,
            code,
            reason,
            reference,
        } => {
            let revoked = hub
                .models
                .takedown(moderator, &model, code, &reason, &reference)
                .await?;
            // The database already says so; now the zones and their clients.
            for model in revoked {
                hub.notify_all(HubNotice::ModelRevoked { model }).await;
            }
            Ok(HubResponse::Ok)
        }
        ModOp::Reinstate { model, reason } => {
            hub.models.reinstate(moderator, &model, &reason).await?;
            Ok(HubResponse::Ok)
        }
        ModOp::SetUpload { email, allow } => {
            hub.models.set_upload(moderator, &email, allow).await?;
            Ok(HubResponse::Ok)
        }
        ModOp::SetTrust { email, tier } => {
            hub.models.set_trust(moderator, &email, tier).await?;
            Ok(HubResponse::Ok)
        }
        ModOp::ClearStrikes { email } => {
            hub.models.clear_strikes(moderator, &email).await?;
            Ok(HubResponse::Ok)
        }
        ModOp::AimReport { weeks, min_shots } => Ok(HubResponse::AimReport(
            hub.conduct.aim_report(moderator, weeks, min_shots).await?,
        )),
        ModOp::Replays {
            email,
            reported,
            flagged,
            limit,
        } => Ok(HubResponse::Replays(
            hub.conduct
                .replays(moderator, email.as_deref(), reported, flagged, limit)
                .await?,
        )),
        // Answered on the stream itself (`handle_stream`): a blob.
        ModOp::ReplayGet { .. } => Err(HubError::Internal),
        ModOp::Reports { open_only } => Ok(HubResponse::Reports(
            hub.conduct.reports(moderator, open_only).await?,
        )),
        ModOp::ReportVerdict { id, verdict, note } => {
            hub.conduct
                .report_verdict(moderator, id, verdict, &note)
                .await?;
            Ok(HubResponse::Ok)
        }
        ModOp::Ban {
            email,
            days,
            reason,
            cheat,
        } => ban(hub, moderator, email, days, reason, cheat).await,
        ModOp::Unban { email, note } => {
            hub.conduct.unban(moderator, &email, &note).await?;
            Ok(HubResponse::Ok)
        }
        ModOp::Reputation { email } => Ok(HubResponse::Standing(
            hub.conduct.standing(moderator, &email).await?,
        )),
        ModOp::Adjust { email, delta, note } => {
            hub.conduct.adjust(moderator, &email, delta, &note).await?;
            Ok(HubResponse::Ok)
        }
    }
}

/// `ModOp::Ban`: the account's sessions end here, its characters leave their zones, and
/// its stalls close (ANTICHEAT.md 6).
async fn ban(
    hub: &Arc<Hub>,
    moderator: AccountId,
    email: String,
    days: u32,
    reason: String,
    cheat: bool,
) -> Result<HubResponse, HubError> {
    let (account, kicked) = hub
        .conduct
        .ban(moderator, &email, days, &reason, cheat)
        .await?;
    // The account's sessions end here, its characters leave their zones,
    // and its stalls close (ANTICHEAT.md 6).
    hub.state
        .lock()
        .unwrap()
        .sessions
        .retain(|_, s| s.account != account);
    for k in kicked {
        hub.notify(
            &k.zone,
            HubNotice::Kick {
                character: k.character,
                reason: format!("banned: {reason}"),
            },
        )
        .await;
    }
    // The ban stands whatever happens to the tidying after it.
    match hub.db.characters_of(account).await {
        Ok(rows) => {
            for row in rows {
                if let Ok((stall, zone)) = hub.econ.stall_close(row.id).await {
                    hub.notify(&zone, HubNotice::StallClosed { stall }).await;
                }
            }
        }
        Err(e) => warn!(account, "closing a banned account's stalls: {e}"),
    }
    info!(account, days, "account banned");
    Ok(HubResponse::Ok)
}
