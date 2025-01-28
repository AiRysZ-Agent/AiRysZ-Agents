use thiserror::Error;
use qdrant_client::{
    qdrant::{
        Distance, PointStruct, SearchPoints,
        VectorParams, Value,
        with_payload_selector::SelectorOptions, WithPayloadSelector,
        point_id::PointIdOptions,
        PointId, PointsSelector,
        CreateCollection, VectorsConfig,
        UpsertPoints, DeletePoints,
    },
    Qdrant,
    config::QdrantConfig,
    prelude::*,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use log;
use crate::database::qdrant_config::create_qdrant_client;

#[derive(Error, Debug)]
pub enum VectorDBError {
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Operation failed: {0}")]
    Operation(String),
    #[error("Collection exists: {0}")]
    CollectionExists(String),
}

#[derive(Clone)]
pub struct VectorDB {
    client: Arc<Qdrant>,
}

impl VectorDB {
    pub async fn new(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = create_qdrant_client(url).await?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    pub async fn create_collection(
        &self,
        name: &str,
        vector_size: u64,
    ) -> Result<(), VectorDBError> {
        let vectors_config = VectorParams {
            size: vector_size as u64,
            distance: Distance::Cosine.into(),
            ..Default::default()
        };

        let vectors_config = VectorsConfig {
            config: Some(qdrant_client::qdrant::vectors_config::Config::Params(vectors_config)),
        };

        let create_collection = CreateCollection {
            collection_name: name.to_string(),
            vectors_config: Some(vectors_config),
            ..Default::default()
        };

        match self.client.create_collection(create_collection).await {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("AlreadyExists") => {
                log::info!("Collection {} already exists, skipping creation", name);
                Ok(())
            }
            Err(e) => Err(VectorDBError::Operation(e.to_string())),
        }
    }

    pub async fn store_vector(
        &self,
        collection: &str,
        vector: Vec<f32>,
        payload: HashMap<String, serde_json::Value>,
    ) -> Result<String, VectorDBError> {
        let point_id = Uuid::new_v4().to_string();

        // Convert payload values to qdrant::Value
        let payload: HashMap<String, Value> = payload.into_iter()
            .map(|(k, v)| (k, Value::from(v)))
            .collect();

        let point = PointStruct {
            id: Some(PointId { 
                point_id_options: Some(PointIdOptions::Uuid(point_id.clone()))
            }),
            vectors: Some(vector.into()),
            payload: payload,
        };

        let upsert_points = UpsertPoints {
            collection_name: collection.to_string(),
            points: vec![point],
            ..Default::default()
        };

        self.client.upsert_points(upsert_points)
            .await
            .map_err(|e| VectorDBError::Operation(e.to_string()))?;

        Ok(point_id)
    }

    pub async fn search_vectors(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        limit: u64,
    ) -> Result<Vec<(String, f32, HashMap<String, serde_json::Value>)>, VectorDBError> {
        let request = SearchPoints {
            collection_name: collection.to_string(),
            vector: query_vector,
            limit: limit as u64,
            with_payload: Some(WithPayloadSelector {
                selector_options: Some(SelectorOptions::Enable(true)),
            }),
            ..Default::default()
        };

        let results = self.client.search_points(request)
            .await
            .map_err(|e| VectorDBError::Operation(e.to_string()))?;

        let points = results.result
            .into_iter()
            .map(|point| {
                let id = match point.id.and_then(|id| id.point_id_options) {
                    Some(PointIdOptions::Uuid(uuid)) => uuid,
                    _ => String::new(),
                };
                let score = point.score;
                let payload = point.payload
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::try_from(v).unwrap_or(serde_json::Value::Null)))
                    .collect();
                (id, score, payload)
            })
            .collect();

        Ok(points)
    }

    pub async fn delete_vectors(
        &self,
        collection: &str,
        ids: Vec<String>,
    ) -> Result<(), VectorDBError> {
        let points = ids.into_iter()
            .map(|id| PointId {
                point_id_options: Some(PointIdOptions::Uuid(id))
            })
            .collect::<Vec<_>>();

        let points_selector = PointsSelector {
            points_selector_one_of: Some(points.into()),
            ..Default::default()
        };

        let delete_points = DeletePoints {
            collection_name: collection.to_string(),
            points: Some(points_selector),
            ..Default::default()
        };

        self.client.delete_points(delete_points)
            .await
            .map_err(|e| VectorDBError::Operation(e.to_string()))?;

        Ok(())
    }
} 