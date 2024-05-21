use crate::models::Task;

/// ====== DATASTORE MODELS ======

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UserDocument {
    pub(crate) tasks: Vec<Task>,
}

/// Errors that can occur when interacting with the datastore
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum DatastoreErrors {
    /// Failed to parse request data
    InvalidWebhookRequestData(String),
    InvalidRequestData(String),
}

impl std::fmt::Display for DatastoreErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatastoreErrors::InvalidWebhookRequestData(e) => {
                write!(f, "Invalid webhook request data: {}", e)
            }
            DatastoreErrors::InvalidRequestData(e) => {
                write!(f, "Invalid request data: {}", e)
            }
        }
    }
}
