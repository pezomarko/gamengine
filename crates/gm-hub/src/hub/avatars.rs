//! A session's custom models (MODELS.md 6): the list, a drop, what a character wears.

use super::*;
use gm_hub_proto::protocol::ModelId;

/// `HubRequest::ModelList`.
pub(super) async fn model_list(
    hub: &Arc<Hub>,
    session: SessionId,
) -> Result<HubResponse, HubError> {
    let account = hub.session_account(session)?;
    Ok(HubResponse::Models(hub.models.list(account).await?))
}

/// `HubRequest::ModelDrop`.
pub(super) async fn model_drop(
    hub: &Arc<Hub>,
    session: SessionId,
    model: ModelId,
) -> Result<HubResponse, HubError> {
    let account = hub.session_account(session)?;
    hub.models.drop_model(account, &model).await?;
    Ok(HubResponse::Ok)
}

/// `HubRequest::SetModel`.
pub(super) async fn set_model(
    hub: &Arc<Hub>,
    session: SessionId,
    character: CharacterId,
    model: Option<ModelId>,
) -> Result<HubResponse, HubError> {
    let account = hub.session_account(session)?;
    let row = hub
        .db
        .character(character)
        .await?
        .filter(|r| r.account_id == account)
        .ok_or(HubError::NotFound)?;
    let frame = gm_model::rig::frame_index(row.build.frame);
    hub.models
        .set_model(account, character, frame, model.as_ref())
        .await?;
    Ok(HubResponse::Ok)
}
