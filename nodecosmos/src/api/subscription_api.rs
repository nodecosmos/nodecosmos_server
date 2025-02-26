use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::node::{AuthNode, Node};
use crate::models::subscription::{ShowSubscription, Subscription, SubscriptionStatus};
use crate::models::user::ShowUser;
use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use charybdis::operations::InsertWithCallbacks;
use charybdis::types::Uuid;
use futures::StreamExt;
use serde_json::json;

#[post("/webhook")]
pub async fn subscription_webhook(req: HttpRequest, mut payload: web::Payload, app: web::Data<App>) -> Response {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.map_err(|e| {
            log::error!("Failed to read payload: {:?}", e);
            NodecosmosError::InternalServerError("Failed to read payload".to_string())
        })?;

        body.extend_from_slice(&chunk);
    }
    let payload_str = std::str::from_utf8(&body).map_err(|e| {
        log::error!("Failed to convert payload to string: {:?}", e);
        actix_web::error::ErrorBadRequest("Invalid UTF-8 payload")
    })?;

    let stripe_signature = req
        .headers()
        .get("Stripe-Signature")
        .ok_or_else(|| {
            log::error!("Stripe-Signature header not found");
            actix_web::error::ErrorBadRequest("Stripe-Signature header not found")
        })?
        .to_str()
        .map_err(|e| {
            log::error!("Failed to parse Stripe-Signature header: {:?}", e);
            actix_web::error::ErrorBadRequest("Failed to parse Stripe-Signature header")
        })?;

    let secret = if let Some(secret) = app.stripe_cfg.as_ref().map(|stripe_cfg| &stripe_cfg.webhook_secret_key) {
        secret
    } else {
        return Err(NodecosmosError::InternalServerError(
            "Stripe secret key not found".to_string(),
        ));
    };

    let event = stripe::Webhook::construct_event(payload_str, stripe_signature, secret).map_err(|e| {
        log::error!("Failed to construct event: {}", e);
        NodecosmosError::BadRequest("Failed to construct event".to_string())
    })?;

    match event.type_ {
        stripe::EventType::CheckoutSessionExpired => {
            if let stripe::EventObject::CheckoutSession(session) = event.data.object {
                Subscription::handle_checkout_session_expired_event(&app, session).await?;
            } else {
                log::error!(
                    "Failed to parse CheckoutSessionExpired event object {:?}",
                    event.data.object
                );
            }
        }
        stripe::EventType::CustomerSubscriptionCreated => {
            if let stripe::EventObject::Subscription(subscription) = event.data.object {
                Subscription::handle_subscription_created_event(&app, subscription).await?;
            } else {
                log::error!(
                    "Failed to parse CheckoutSessionExpired event object {:?}",
                    event.data.object
                );
            }
        }
        stripe::EventType::CustomerSubscriptionDeleted => {
            if let stripe::EventObject::Subscription(subscription) = event.data.object {
                Subscription::handle_subscription_cancelled_event(&app, subscription, SubscriptionStatus::Deleted)
                    .await?;
            } else {
                log::error!(
                    "Failed to parse CheckoutSessionExpired event object {:?}",
                    event.data.object
                );
            }
        }
        stripe::EventType::CustomerSubscriptionPaused => {
            if let stripe::EventObject::Subscription(subscription) = event.data.object {
                Subscription::handle_subscription_cancelled_event(&app, subscription, SubscriptionStatus::Paused)
                    .await?;
            } else {
                log::error!(
                    "Failed to parse CheckoutSessionExpired event object {:?}",
                    event.data.object
                );
            }
        }
        stripe::EventType::CustomerSubscriptionResumed => {
            if let stripe::EventObject::Subscription(subscription) = event.data.object {
                Subscription::handle_subscription_created_event(&app, subscription).await?;
            } else {
                log::error!(
                    "Failed to parse CheckoutSessionExpired event object {:?}",
                    event.data.object
                );
            }
        }
        stripe::EventType::CustomerSubscriptionUpdated => {
            if let stripe::EventObject::Subscription(subscription) = event.data.object {
                Subscription::handle_subscription_updated_event(&app, subscription).await?;
            } else {
                log::error!(
                    "Failed to parse CheckoutSessionExpired event object {:?}",
                    event.data.object
                );
            }
        }
        _ => {
            log::warn!("Unhandled event type: {:?}", event.type_);
        }
    }

    Ok(HttpResponse::Ok().finish())
}

#[post("/build_url")]
pub async fn build_subscription_url(node: web::Json<Node>, data: RequestData) -> Response {
    let mut node = node.into_inner();

    if node.is_public {
        Err(NodecosmosError::Forbidden(
            "Subscription is not required for public nodes!".to_string(),
        ))?
    }

    if !node.is_root {
        Err(NodecosmosError::Forbidden(
            "Subscription is only allowed on organization root nodes!".to_string(),
        ))?
    }

    if !data.current_user.is_confirmed {
        Err(NodecosmosError::Forbidden(
            "User must confirm email before creating subscription!".to_string(),
        ))?
    }

    if data.current_user.is_blocked {
        Err(NodecosmosError::Forbidden("User is blocked!".to_string()))?
    }

    let insert = node.insert_cb(&data).execute(data.db_session()).await;

    if let Err(e) = insert {
        return Err(e);
    }

    let url = Subscription::build_stripe_url(&data, node, None).await?;

    Ok(HttpResponse::Ok().json(json!({
        "url": url
    })))
}

#[get("/build_new_url_for_cancelled_sub/{root_id}")]
pub async fn build_new_url_for_cancelled_sub(data: RequestData, root_id: web::Path<Uuid>) -> Response {
    let root_id = root_id.into_inner();
    let node = Node::find_by_branch_id_and_id(root_id, root_id)
        .execute(data.db_session())
        .await?;

    if node.owner_id != data.current_user.id {
        Err(NodecosmosError::Forbidden(
            "Only the owner can build new subscription!".to_string(),
        ))?
    }

    let existing_subscription = Subscription::find_by_root_id(root_id)
        .execute(data.db_session())
        .await?;

    if existing_subscription.status == SubscriptionStatus::Active.to_string() {
        Err(NodecosmosError::Forbidden(
            "Subscription is already active!".to_string(),
        ))?
    }

    let url = Subscription::build_stripe_url(&data, node, Some(existing_subscription)).await?;

    Ok(HttpResponse::Ok().json(json!({
        "url": url
    })))
}

#[get("/customer_portal_url/{root_id}")]
pub async fn get_customer_portal_url(data: RequestData, root_id: web::Path<Uuid>) -> Response {
    let root_id = root_id.into_inner();
    let node = AuthNode::find_by_branch_id_and_id(root_id, root_id)
        .execute(data.db_session())
        .await?;

    if node.owner_id != data.current_user.id {
        Err(NodecosmosError::Forbidden(
            "Only the owner can view members of the organization!".to_string(),
        ))?
    }

    let url = Subscription::build_stripe_customer_portal_url(&data, root_id).await?;

    Ok(HttpResponse::Ok().json(json!({
        "url": url
    })))
}

#[get("/{root_id}")]
pub async fn get_subscription(data: RequestData, root_id: web::Path<Uuid>) -> Response {
    let root_id = root_id.into_inner();
    let node = AuthNode::find_by_branch_id_and_id(root_id, root_id)
        .execute(data.db_session())
        .await?;

    if node.owner_id != data.current_user.id {
        Err(NodecosmosError::Forbidden(
            "Only the owner can view members of the organization!".to_string(),
        ))?
    }

    let subscription = ShowSubscription::find_by_root_id(root_id)
        .execute(data.db_session())
        .await?;

    let users = ShowUser::find_by_ids(data.db_session(), &subscription.member_ids).await?;

    Ok(HttpResponse::Ok().json(json!({
        "subscription": subscription,
        "users": users
    })))
}

#[delete("/{root_id}/member/{member_id}")]
pub async fn delete_org_member(data: RequestData, params: web::Path<(Uuid, Uuid)>) -> Response {
    let (root_id, member_id) = params.into_inner();

    let node = Node::find_by_branch_id_and_id(root_id, root_id)
        .execute(data.db_session())
        .await?;

    if node.owner_id == member_id {
        Err(NodecosmosError::Forbidden(
            "Owner cannot be removed from the organization!".to_string(),
        ))?
    }

    if node.owner_id != data.current_user.id {
        Err(NodecosmosError::Forbidden(
            "Only the owner can remove members from the organization!".to_string(),
        ))?
    }

    let subscription = Subscription::find_by_root_id(root_id)
        .execute(data.db_session())
        .await?;

    subscription.remove_members(&data, &[member_id]).await?;

    Ok(HttpResponse::Ok().finish())
}
