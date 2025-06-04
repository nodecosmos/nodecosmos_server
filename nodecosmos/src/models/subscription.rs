use crate::api::data::RequestData;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::description::Description;
use crate::models::node::{Node, UpdateEditorsNode};
use crate::models::traits::{Descendants, ElasticDocument};
use crate::models::workflow::Workflow;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Delete, Insert, Update};
use charybdis::types::{Set, Text, Timestamp, Uuid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum SubscriptionStatus {
    Active,
    Canceled,
    Deleted,
    Incomplete,
    IncompleteExpired,
    PastDue,
    Paused,
    Trialing,
    Unpaid,
    OpenSource,
}

#[charybdis_model(
    table_name = subscriptions,
    partition_keys = [root_id],
    clustering_keys = []
)]
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub root_id: Uuid,
    pub status: Text,
    pub customer_id: Option<Text>,
    pub sub_id: Option<Text>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub member_ids: Set<Uuid>,
}

partial_subscription!(ShowSubscription, root_id, status, created_at, updated_at, member_ids);

impl Subscription {
    pub async fn build_stripe_url(
        data: &RequestData,
        node: Node,
        existing_sub: Option<Self>,
    ) -> Result<String, NodecosmosError> {
        if let Some(stripe_cfg) = data.stripe_cfg() {
            let client_reference_id = node.root_id.to_string();
            let client = stripe::Client::new(stripe_cfg.secret_key.clone());
            let mut params = stripe::CreateCheckoutSession::new();
            let quantity = existing_sub.as_ref().map_or(1, |s| s.member_ids.len());
            let line_item = stripe::CreateCheckoutSessionLineItems {
                price: Some(stripe_cfg.price_id.clone()),
                quantity: Some(quantity as u64),
                ..Default::default()
            };
            let trial_period_days = if existing_sub.is_some() { None } else { Some(7) };
            let success_url = format!(
                "{}/nodes/{}/{}?subscribed=true",
                data.app.config.client_url, node.branch_id, node.id
            );
            let mut metadata = HashMap::new();
            metadata.insert("root_id".to_string(), node.root_id.to_string());

            params.metadata = Some(metadata.clone());
            params.line_items = Some(vec![line_item]);
            params.mode = Some(stripe::CheckoutSessionMode::Subscription);
            params.ui_mode = Some(stripe::CheckoutSessionUiMode::Hosted);
            params.success_url = Some(&success_url);
            params.subscription_data = Some(stripe::CreateCheckoutSessionSubscriptionData {
                trial_period_days,
                metadata: Some(metadata),
                ..Default::default()
            });
            params.client_reference_id = Some(&client_reference_id);

            let checkout_session = stripe::CheckoutSession::create(&client, params).await.map_err(|e| {
                NodecosmosError::InternalServerError(format!("Failed to create checkout session: {}", e))
            })?;

            Subscription {
                root_id: node.root_id,
                customer_id: existing_sub.as_ref().and_then(|s| s.customer_id.clone()),
                sub_id: existing_sub.as_ref().and_then(|s| s.sub_id.clone()),
                status: existing_sub
                    .as_ref()
                    .map_or(SubscriptionStatus::Incomplete.to_string(), |s| s.status.clone()),
                created_at: existing_sub.as_ref().map_or(chrono::Utc::now(), |s| s.created_at),
                updated_at: chrono::Utc::now(),
                member_ids: existing_sub
                    .as_ref()
                    .map_or([data.current_user.id].into_iter().collect(), |s| s.member_ids.clone()),
            }
            .insert()
            .execute(&data.app.db_session)
            .await?;

            if let Some(url) = checkout_session.url {
                Ok(url)
            } else {
                Err(NodecosmosError::InternalServerError(
                    "Failed to create checkout session".to_string(),
                ))
            }
        } else {
            Err(NodecosmosError::InternalServerError(
                "Stripe config not found".to_string(),
            ))
        }
    }

    pub async fn build_stripe_customer_portal_url(
        data: &RequestData,
        root_id: Uuid,
    ) -> Result<String, NodecosmosError> {
        if let Some(stripe_cfg) = data.stripe_cfg() {
            let client = stripe::Client::new(stripe_cfg.secret_key.clone());
            let sub = Subscription::find_by_root_id(root_id)
                .execute(&data.app.db_session)
                .await?;
            let customer_id = sub
                .customer_id
                .as_ref()
                .ok_or_else(|| NodecosmosError::InternalServerError("Customer ID not found".to_string()))?;
            let return_url = format!(
                "{}/nodes/{}/{}/subscriptions",
                data.app.config.client_url, root_id, root_id
            );
            let params = stripe::CreateBillingPortalSession {
                configuration: None,
                customer: stripe::CustomerId::from_str(customer_id)
                    .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to parse customer_id: {}", e)))?,
                expand: &[],
                flow_data: None,
                locale: None,
                on_behalf_of: None,
                return_url: Some(&return_url),
            };
            let portal_session = stripe::BillingPortalSession::create(&client, params)
                .await
                .map_err(|e| {
                    NodecosmosError::InternalServerError(format!("Failed to create customer portal session: {}", e))
                })?;

            Ok(portal_session.url)
        } else {
            Err(NodecosmosError::InternalServerError(
                "Stripe config not found".to_string(),
            ))
        }
    }

    pub async fn handle_checkout_session_expired_event(
        app: &App,
        object: stripe::CheckoutSession,
    ) -> Result<(), NodecosmosError> {
        if let Some(client_reference_id) = object.client_reference_id {
            let root_id = Uuid::parse_str(&client_reference_id)
                .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to parse root_id: {}", e)))?;

            let root_node = Node::find_by_branch_id_and_id(root_id, root_id)
                .execute(&app.db_session)
                .await?;

            let descendants = root_node.descendants(&app.db_session).await?.try_collect().await?;

            if !descendants.is_empty() {
                log::error!("Expired stripe session node {} should not have descendants", root_id);

                return Ok(());
            }

            if Description::maybe_find_first_by_branch_id_and_object_id(root_id, root_id)
                .execute(&app.db_session)
                .await?
                .is_some()
            {
                log::error!("Expired stripe session node {} should not have description", root_id);

                return Ok(());
            }

            if root_node.editor_ids.as_ref().is_some_and(|ids| !ids.is_empty()) {
                log::error!("Expired stripe session node {} should not have editors", root_id);

                return Ok(());
            }

            root_node.delete().execute(&app.db_session).await?;

            if let Some(wf) = Workflow::maybe_find_first_by_branch_id_and_node_id(root_id, root_id)
                .execute(&app.db_session)
                .await?
            {
                wf.delete().execute(&app.db_session).await?;
            }

            root_node.delete_elastic_document(&app.elastic_client).await?;

            log::info!("Expired stripe session node {} deleted", client_reference_id);
        }

        Ok(())
    }

    pub async fn handle_subscription_created_event(
        app: &App,
        object: stripe::Subscription,
    ) -> Result<(), NodecosmosError> {
        let root_id_string = object.metadata.get("root_id").ok_or_else(|| {
            NodecosmosError::InternalServerError("root_id not found in SubscriptionCreated event metadata".to_string())
        })?;
        let root_id = Uuid::parse_str(root_id_string)
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to parse root_id: {}", e)))?;

        Node::find_by_branch_id_and_id(root_id, root_id)
            .execute(&app.db_session)
            .await?
            .update_sub_active(app, true)
            .await?;

        let mut subscription = Subscription::find_by_root_id(root_id).execute(&app.db_session).await?;
        subscription.customer_id = Some(object.customer.id().to_string());
        subscription.sub_id = Some(object.id.to_string());
        subscription.status = SubscriptionStatus::Active.to_string();
        subscription.updated_at = chrono::Utc::now();
        subscription.update().execute(&app.db_session).await.map_err(|e| {
            log::error!("Failed to update subscription: {}", e);
            NodecosmosError::InternalServerError(format!("Failed to update subscription: {}", e))
        })?;

        Ok(())
    }

    pub async fn handle_subscription_cancelled_event(
        app: &App,
        object: stripe::Subscription,
        status: SubscriptionStatus,
    ) -> Result<(), NodecosmosError> {
        let root_id_string = object.metadata.get("root_id").ok_or_else(|| {
            NodecosmosError::InternalServerError(
                "root_id not found in SubscriptionCancelled event metadata".to_string(),
            )
        })?;
        let root_id = Uuid::parse_str(root_id_string)
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to parse root_id: {}", e)))?;

        if let Some(mut node) = Node::maybe_find_first_by_branch_id_and_id(root_id, root_id)
            .execute(&app.db_session)
            .await?
        {
            node.update_sub_active(app, false).await?
        }

        let mut subscription = Subscription::find_by_root_id(root_id).execute(&app.db_session).await?;
        subscription.status = status.to_string();
        subscription.updated_at = chrono::Utc::now();
        subscription.update().execute(&app.db_session).await.map_err(|e| {
            log::error!("Failed to update subscription: {}", e);
            NodecosmosError::InternalServerError(format!("Failed to update subscription: {}", e))
        })?;

        Ok(())
    }

    pub async fn handle_subscription_updated_event(
        app: &App,
        object: stripe::Subscription,
    ) -> Result<(), NodecosmosError> {
        let root_id_string = object.metadata.get("root_id").ok_or_else(|| {
            NodecosmosError::InternalServerError("root_id not found in SubscriptionCreated event metadata".to_string())
        })?;
        let root_id = Uuid::parse_str(root_id_string)
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to parse root_id: {}", e)))?;

        let mut subscription = Subscription::find_by_root_id(root_id).execute(&app.db_session).await?;

        // check if subscription is past_due
        if subscription.status != SubscriptionStatus::PastDue.to_string()
            && object.status == stripe::SubscriptionStatus::PastDue
        {
            subscription.status = SubscriptionStatus::PastDue.to_string();
            subscription.updated_at = chrono::Utc::now();
            subscription.update().execute(&app.db_session).await.map_err(|e| {
                log::error!("Failed to update subscription: {}", e);
                NodecosmosError::InternalServerError(format!("Failed to update subscription: {}", e))
            })?;
        } else if subscription.status != SubscriptionStatus::Unpaid.to_string()
            && object.status == stripe::SubscriptionStatus::Unpaid
        {
            subscription.status = SubscriptionStatus::Unpaid.to_string();
            subscription.updated_at = chrono::Utc::now();
            subscription.update().execute(&app.db_session).await.map_err(|e| {
                log::error!("Failed to update subscription: {}", e);
                NodecosmosError::InternalServerError(format!("Failed to update subscription: {}", e))
            })?;
            Node::find_by_branch_id_and_id(root_id, root_id)
                .execute(&app.db_session)
                .await?
                .update_sub_active(app, false)
                .await?;
        } else if subscription.status != SubscriptionStatus::Active.to_string()
            && object.status == stripe::SubscriptionStatus::Active
        {
            subscription.status = SubscriptionStatus::Active.to_string();
            subscription.updated_at = chrono::Utc::now();
            subscription.update().execute(&app.db_session).await.map_err(|e| {
                log::error!("Failed to update subscription: {}", e);
                NodecosmosError::InternalServerError(format!("Failed to update subscription: {}", e))
            })?;
            Node::find_by_branch_id_and_id(root_id, root_id)
                .execute(&app.db_session)
                .await?
                .update_sub_active(app, true)
                .await?;
        }

        Ok(())
    }

    pub async fn remove_members(&self, data: &RequestData, removed_ids: &[Uuid]) -> Result<(), NodecosmosError> {
        let node = Node::find_by_branch_id_and_id(self.root_id, self.root_id)
            .execute(data.db_session())
            .await?;

        UpdateEditorsNode::update_editor_ids(data, &node, &[], removed_ids).await?;

        Self::update_members(data, node.root_id, &[], removed_ids).await?;

        Ok(())
    }

    pub async fn update_members(
        data: &RequestData,
        root_id: Uuid,
        added_ids: &[Uuid],
        removed_ids: &[Uuid],
    ) -> Result<(), NodecosmosError> {
        let mut sub = Subscription::find_by_root_id(root_id)
            .execute(&data.app.db_session)
            .await?;
        let current_members_len = sub.member_ids.len();
        sub.member_ids.extend(added_ids.iter().cloned());
        sub.member_ids.retain(|id| !removed_ids.contains(id));
        sub.updated_at = chrono::Utc::now();
        let new_members_len = sub.member_ids.len();

        sub.update().execute(&data.app.db_session).await.map_err(|e| {
            log::error!("Failed to update subscription: {}", e);
            NodecosmosError::InternalServerError(format!("Failed to update subscription: {}", e))
        })?;

        if new_members_len == current_members_len {
            return Ok(());
        }

        // update subscription in stripe with new quantity
        if let (Some(stripe_cfg), Some(sub_id)) = (data.stripe_cfg(), sub.sub_id.as_ref()) {
            let client = stripe::Client::new(stripe_cfg.secret_key.clone());
            let sub_id = stripe::SubscriptionId::from_str(&sub_id.to_string())
                .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to parse sub_id: {}", e)))?;
            let subscription = stripe::Subscription::retrieve(&client, &sub_id, &[])
                .await
                .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to retrieve subscription: {}", e)))?;

            let mut params = stripe::UpdateSubscription::new();
            let item = subscription.items.data.into_iter().next();

            match item {
                Some(item) => {
                    let update_sub_item = stripe::UpdateSubscriptionItems {
                        id: Some(item.id.to_string()),
                        quantity: Some(new_members_len as u64),
                        ..Default::default()
                    };

                    params.items = Some(vec![update_sub_item]);

                    stripe::Subscription::update(&client, &sub_id, params)
                        .await
                        .map_err(|e| {
                            NodecosmosError::InternalServerError(format!("Failed to update subscription: {}", e))
                        })?;
                }
                None => {
                    return Err(NodecosmosError::InternalServerError(
                        "No subscription items found".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    pub async fn cancel_subscription(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let (Some(stripe_cfg), Some(sub_id)) = (data.stripe_cfg(), self.sub_id.as_ref()) {
            let client = stripe::Client::new(stripe_cfg.secret_key.clone());
            let sub_id = stripe::SubscriptionId::from_str(&sub_id.to_string())
                .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to parse sub_id: {}", e)))?;
            let params = stripe::CancelSubscription::new();
            stripe::Subscription::cancel(&client, &sub_id, params)
                .await
                .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to cancel subscription: {}", e)))?;
        }

        Ok(())
    }
}
