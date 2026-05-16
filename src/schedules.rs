//! Recurring ingestion schedules.

use crate::client::Client;
use crate::datasets::push_pagination;
use crate::errors::Result;
use crate::transport::Request;
use crate::types::{
    CreateScheduleRequest, Schedule, ScheduleList, TriggerScheduleResponse, UpdateScheduleRequest,
};

/// Service for managing recurring ingestion schedules.
///
/// A schedule pairs a source with a target dataset and a cron expression. The
/// server's ingestion scheduler daemon polls for due schedules and creates jobs
/// as they fire.
#[derive(Clone)]
pub struct ScheduleService {
    client: Client,
}

impl ScheduleService {
    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }

    /// List schedules with optional `limit` and `offset` pagination.
    pub async fn list(&self, limit: u32, offset: u32) -> Result<ScheduleList> {
        let mut req = Request {
            method: "GET".into(),
            path: "/ingestion/schedules".into(),
            ..Default::default()
        };
        push_pagination(&mut req.query, limit, offset);
        self.client.dispatcher().json(req).await
    }

    /// Get one schedule by id.
    pub async fn get(&self, schedule_id: &str) -> Result<Schedule> {
        self.client
            .dispatcher()
            .json(Request {
                method: "GET".into(),
                path: format!("/ingestion/schedules/{schedule_id}"),
                ..Default::default()
            })
            .await
    }

    /// Create a recurring schedule.
    pub async fn create(&self, request: CreateScheduleRequest) -> Result<Schedule> {
        let body = serde_json::to_value(&request)?;
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: "/ingestion/schedules".into(),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// Apply a partial update to a schedule. Only `Some` fields in `request`
    /// are sent.
    pub async fn update(
        &self,
        schedule_id: &str,
        request: UpdateScheduleRequest,
    ) -> Result<Schedule> {
        let body = serde_json::to_value(&request)?;
        self.client
            .dispatcher()
            .json(Request {
                method: "PATCH".into(),
                path: format!("/ingestion/schedules/{schedule_id}"),
                body: Some(body),
                ..Default::default()
            })
            .await
    }

    /// Delete a schedule.
    pub async fn delete(&self, schedule_id: &str) -> Result<()> {
        self.client
            .dispatcher()
            .empty(Request {
                method: "DELETE".into(),
                path: format!("/ingestion/schedules/{schedule_id}"),
                ..Default::default()
            })
            .await
    }

    /// Trigger an immediate run for a schedule, outside its cron cadence.
    pub async fn trigger(&self, schedule_id: &str) -> Result<TriggerScheduleResponse> {
        self.client
            .dispatcher()
            .json(Request {
                method: "POST".into(),
                path: format!("/ingestion/schedules/{schedule_id}/trigger"),
                ..Default::default()
            })
            .await
    }
}
